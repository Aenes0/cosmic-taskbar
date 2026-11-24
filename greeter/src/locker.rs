// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

use color_eyre::eyre::WrapErr;
use cosmic::app::{Core, Settings, Task};
use cosmic::cctk::wayland_protocols::xdg::shell::client::xdg_positioner::Gravity;
use cosmic::iced::core::text::{Ellipsize, EllipsizeHeightLimit};
use cosmic::iced::event::wayland::{OutputEvent, SessionLockEvent};
use cosmic::iced::futures::{self, SinkExt};
use cosmic::iced::platform_specific::shell::wayland::commands::session_lock::{
    destroy_lock_surface, lock, unlock,
};
use cosmic::iced::runtime::core::window::Id as SurfaceId;
use cosmic::iced::runtime::platform_specific::wayland::subsurface::SctkSubsurfaceSettings;
use cosmic::iced::{
    self, Alignment, Background, Border, Length, Point, Size, Subscription,
};
use cosmic::surface::action::LiveSettings;
use cosmic::{Element, executor, surface, theme, widget};
use cosmic_config::CosmicConfigEntry;
use cosmic_greeter_daemon::{TimeAppletConfig, UserData};
use std::any::TypeId;
use std::ffi::{CStr, CString};
use std::os::fd::OwnedFd;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use std::{env, fs, process};
use tokio::sync::mpsc;
use tokio::task;
use tracing::level_filters::LevelFilter;
use tracing::warn;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};
use wayland_client::Proxy;
use wayland_client::protocol::wl_output::WlOutput;

use crate::backdrop::{self, BackdropSet};
use crate::common::{self, Common, DEFAULT_MENU_ITEM_HEIGHT, MAX_WIDTH};
use crate::fl;
use crate::login_ui;
use crate::styles;
use crate::motion::{self, HoverTarget, Motion, Part};
use crate::power::{self, PowerAction};
use crate::style_config::{self, StyleConfig};

fn lockfile_opt() -> Option<PathBuf> {
    let runtime_dir = dirs::runtime_dir()?;
    let session_id = env::var("XDG_SESSION_ID").ok()?;
    Some(runtime_dir.join(format!("cosmic-greeter-{}.lock", session_id)))
}

pub fn main(user: pwd::Passwd) -> Result<(), Box<dyn std::error::Error>> {
    color_eyre::install().wrap_err("failed to install color_eyre error handler")?;

    let trace = tracing_subscriber::registry();
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::WARN.into())
        .from_env_lossy();

    #[cfg(feature = "systemd")]
    if let Ok(journald) = tracing_journald::layer() {
        trace
            .with(journald)
            .with(env_filter)
            .try_init()
            .wrap_err("failed to initialize logger")?;
    } else {
        trace
            .with(fmt::layer())
            .with(env_filter)
            .try_init()
            .wrap_err("failed to initialize logger")?;
        warn!("failed to connect to journald")
    }

    #[cfg(not(feature = "systemd"))]
    trace
        .with(fmt::layer())
        .with(env_filter)
        .try_init()
        .wrap_err("failed to initialize logger")?;

    crate::localize::localize();

    let mut user_data = UserData::from(user);
    // We are already the user at this point
    user_data.load_config_as_user();

    let nested = crate::power::nested_from_env();

    let flags = Flags {
        user_icon: user_data
            .icon_opt
            .take()
            .map(widget::image::Handle::from_bytes),
        user_data,
        lockfile_opt: lockfile_opt(),
        #[cfg(feature = "logind")]
        logind_available: crate::logind::is_available() && !nested,
        #[cfg(not(feature = "logind"))]
        logind_available: false,
        nested,
        demo: motion::demo_mode_from(std::env::var_os(motion::DEMO_ENV).as_deref(), nested),
    };

    let settings = Settings::default().no_main_window(true);

    crate::perf::init_from_env();
    cosmic::app::run::<App>(settings, flags)?;

    Ok(())
}

/// Linha de status de hoje: campo visível = prompt com valor (Caps Lock só conta aí);
/// prompt sem valor = info. `menu` e `current_status` usam a mesma escolha.
fn status_for<'a>(
    prompt_opt: &'a Option<(String, bool, Option<String>)>,
    authenticating: bool,
    caps_lock: bool,
    error_opt: Option<&'a str>,
) -> login_ui::StatusLine<'a> {
    let (field, info) = match prompt_opt {
        Some((_, _, Some(_))) => (true, None),
        Some((prompt, _, None)) => (false, Some(prompt.as_str())),
        None => (false, None),
    };
    login_ui::status_line(authenticating, caps_lock && field, error_opt, info)
}

/// Mensagens de entrar/sair do hover de um alvo.
fn hovers(surface_id: SurfaceId, target: HoverTarget) -> login_ui::Hovers<Message> {
    login_ui::Hovers {
        enter: Message::Hover(surface_id, target, true),
        exit: Message::Hover(surface_id, target, false),
    }
}

/// Aplica um passo do demo ao movimento; devolve `Some(aberto)` quando o passo muda o menu ⏻.
pub(crate) fn apply_demo<K: Copy + PartialEq>(
    motion: &mut Motion<K>,
    event: motion::DemoEvent,
    now: Instant,
) -> Option<bool> {
    match event {
        motion::DemoEvent::RestartEntry => motion.restart_entry(now),
        motion::DemoEvent::Shake => motion.on_auth_error(true, now),
        motion::DemoEvent::Menu(open) => {
            if open {
                motion.open_menu(now);
            }
            return Some(open);
        }
        motion::DemoEvent::Hover(on) => motion.demo_hover(on, now),
    }
    None
}

/// O campo aceita texto e envio só fora da autenticação e fora do fade de saída.
pub(crate) fn accepts_input(authenticating: bool, exiting: bool) -> bool {
    !authenticating && !exiting
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UnlockStep {
    /// Bloqueado: começa o fade; `unlock()` só quando o timer `EXIT_FADE` disparar.
    Fade,
    /// Já saindo (PAM + logind quase juntos): nada, um único `unlock()`.
    Ignore,
    /// Fora de `Locked`: segue direto para `finish_unlock` (mesmos logs de antes).
    Finish,
}

fn unlock_step<K: Copy + PartialEq>(locked: bool, motion: &mut Motion<K>, now: Instant) -> UnlockStep {
    if !locked {
        UnlockStep::Finish
    } else if motion.start_exit(now) {
        UnlockStep::Fade
    } else {
        UnlockStep::Ignore
    }
}

/// Estado do bloqueio sem o `task_handle` (para decisões puras e testes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StateKind {
    Locking,
    Locked,
    Unlocking,
    Unlocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LockStep {
    /// Destravado: bloqueia já.
    Lock,
    /// Destravando (fade de saída ou `Unlocking`): bloqueia de novo assim que o
    /// compositor confirmar o `Unlocked`, em vez de perder o pedido.
    Defer,
    /// Já bloqueando ou bloqueado de vez: nada.
    Ignore,
}

fn lock_decision(kind: StateKind, exiting: bool) -> LockStep {
    match kind {
        StateKind::Unlocked => LockStep::Lock,
        StateKind::Locked if exiting => LockStep::Defer,
        StateKind::Unlocking => LockStep::Defer,
        StateKind::Locking | StateKind::Locked => LockStep::Ignore,
    }
}

/// O inibidor de suspensão (modo `delay` do logind) só é solto quando a tela já está
/// bloqueada de vez; destravando, ele fica até o próximo `Locked` (ou até o logind
/// vencer o `InhibitDelayMaxSec`).
fn keep_inhibit(kind: StateKind, exiting: bool) -> bool {
    !(kind == StateKind::Locked && !exiting)
}

/// Convert PAM errors to user-friendly localized messages
fn pam_error_to_message(error: &pam_client::Error) -> String {
    use pam_client::ErrorCode;

    // Use the structured error code instead of string matching for reliability
    match error.code() {
        ErrorCode::AUTH_ERR | ErrorCode::CRED_INSUFFICIENT => {
            fl!("auth-error-credentials")
        }
        ErrorCode::PERM_DENIED => {
            fl!("auth-error-denied")
        }
        ErrorCode::MAXTRIES => {
            fl!("auth-error-maxtries")
        }
        ErrorCode::ACCT_EXPIRED | ErrorCode::USER_UNKNOWN => {
            fl!("auth-error-account")
        }
        _ => {
            // For any other error, show a generic message
            fl!("auth-error-default")
        }
    }
}

/// Variável que escolhe o serviço PAM (`/etc/pam.d/<nome>`).
pub(crate) const PAM_SERVICE_ENV: &str = "COSMIC_GREETER_PAM_SERVICE";
const DEFAULT_PAM_SERVICE: &str = "cosmic-greeter";

/// Nome do serviço PAM: o valor se for um nome simples (`[A-Za-z0-9._-]`, sem começar por `.`,
/// para nunca virar caminho); senão o padrão compilado.
pub(crate) fn pam_service_from(value: Option<&std::ffi::OsStr>) -> String {
    match value.and_then(|v| v.to_str()) {
        Some(name)
            if !name.is_empty()
                && !name.starts_with('.')
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')) =>
        {
            name.to_string()
        }
        _ => DEFAULT_PAM_SERVICE.to_string(),
    }
}

/// Serviço usado quando o escolhido não tem arquivo PAM (existe em toda distro).
const FALLBACK_PAM_SERVICE: &str = "login";
/// Onde o Linux-PAM procura os serviços (o vendor dir é o fallback de /etc).
const PAM_DIRS: [&str; 2] = ["/etc/pam.d", "/usr/lib/pam.d"];

/// Serviço PAM que realmente existe. Sem `/etc/pam.d/cosmic-greeter` (Arch/CachyOS) o PAM cairia
/// em `other` (`pam_deny`) e recusaria toda senha; então, se o escolhido não existe, usa `login`.
pub(crate) fn resolve_pam_service(
    value: Option<&std::ffi::OsStr>,
    exists: impl Fn(&str) -> bool,
) -> String {
    let candidate = pam_service_from(value);
    if exists(&candidate) || !exists(FALLBACK_PAM_SERVICE) {
        return candidate;
    }
    tracing::warn!(
        "serviço PAM {candidate:?} não existe; usando {FALLBACK_PAM_SERVICE:?}"
    );
    FALLBACK_PAM_SERVICE.to_string()
}

fn pam_service_exists(name: &str) -> bool {
    PAM_DIRS
        .iter()
        .any(|dir| std::path::Path::new(dir).join(name).is_file())
}

pub fn pam_thread(username: String, conversation: Conversation) -> Result<(), pam_client::Error> {
    //TODO: send errors to GUI, restart process

    // Create PAM context
    let mut context = pam_client::Context::new(
        &resolve_pam_service(
            std::env::var_os(PAM_SERVICE_ENV).as_deref(),
            pam_service_exists,
        ),
        Some(&username),
        conversation,
    )?;

    // Authenticate the user (ask for password, 2nd-factor token, fingerprint, etc.)
    tracing::info!("authenticate");
    context.authenticate(pam_client::Flag::NONE)?;

    // Validate the account (is not locked, expired, etc.)
    tracing::info!("acct_mgmt");
    context.acct_mgmt(pam_client::Flag::NONE)?;

    Ok(())
}

pub struct Conversation {
    msg_tx: futures::channel::mpsc::Sender<cosmic::Action<Message>>,
    value_rx: mpsc::Receiver<String>,
}

impl Conversation {
    fn prompt_value(
        &mut self,
        prompt_c: &CStr,
        secret: bool,
    ) -> Result<CString, pam_client::ErrorCode> {
        let prompt = prompt_c.to_str().map_err(|err| {
            tracing::error!("failed to convert prompt to UTF-8: {:?}", err);
            pam_client::ErrorCode::CONV_ERR
        })?;

        futures::executor::block_on(async {
            self.msg_tx
                .send(cosmic::Action::App(
                    common::Message::Prompt(prompt.to_string(), secret, Some(String::new())).into(),
                ))
                .await
        })
        .map_err(|err| {
            tracing::error!("failed to send prompt: {:?}", err);
            pam_client::ErrorCode::CONV_ERR
        })?;

        let value = self.value_rx.blocking_recv().ok_or_else(|| {
            tracing::error!("failed to receive value: channel closed");
            pam_client::ErrorCode::CONV_ERR
        })?;

        CString::new(value).map_err(|err| {
            tracing::error!("failed to convert value to C string: {:?}", err);
            pam_client::ErrorCode::CONV_ERR
        })
    }

    fn message(&mut self, prompt_c: &CStr) -> Result<(), pam_client::ErrorCode> {
        let prompt = prompt_c.to_str().map_err(|err| {
            tracing::error!("failed to convert prompt to UTF-8: {:?}", err);
            pam_client::ErrorCode::CONV_ERR
        })?;

        futures::executor::block_on(async {
            self.msg_tx
                .send(cosmic::Action::App(
                    common::Message::Prompt(prompt.to_string(), false, None).into(),
                ))
                .await
        })
        .map_err(|err| {
            tracing::error!("failed to send prompt: {:?}", err);
            pam_client::ErrorCode::CONV_ERR
        })
    }

    fn error_message(&mut self, prompt_c: &CStr) -> Result<(), pam_client::ErrorCode> {
        let prompt = prompt_c.to_str().map_err(|err| {
            tracing::error!("failed to convert prompt to UTF-8: {:?}", err);
            pam_client::ErrorCode::CONV_ERR
        })?;

        futures::executor::block_on(async {
            self.msg_tx
                .send(cosmic::Action::App(Message::Error(prompt.to_string())))
                .await
        })
        .map_err(|err| {
            tracing::error!("failed to send error message: {:?}", err);
            pam_client::ErrorCode::CONV_ERR
        })
    }
}

impl pam_client::ConversationHandler for Conversation {
    fn prompt_echo_on(&mut self, prompt_c: &CStr) -> Result<CString, pam_client::ErrorCode> {
        tracing::info!("prompt_echo_on {:?}", prompt_c);
        self.prompt_value(prompt_c, false)
    }
    fn prompt_echo_off(&mut self, prompt_c: &CStr) -> Result<CString, pam_client::ErrorCode> {
        tracing::info!("prompt_echo_off {:?}", prompt_c);
        self.prompt_value(prompt_c, true)
    }
    fn text_info(&mut self, prompt_c: &CStr) {
        tracing::info!("text_info {:?}", prompt_c);
        match self.message(prompt_c) {
            Ok(()) => (),
            Err(err) => {
                tracing::warn!("failed to send text_info: {:?}", err);
            }
        }
    }
    fn error_msg(&mut self, prompt_c: &CStr) {
        tracing::info!("error_msg {:?}", prompt_c);
        match self.error_message(prompt_c) {
            Ok(()) => (),
            Err(err) => {
                tracing::warn!("failed to send error_msg: {:?}", err);
            }
        }
    }
}

#[derive(Clone)]
pub struct Flags {
    user_data: UserData,
    user_icon: Option<widget::image::Handle>,
    lockfile_opt: Option<PathBuf>,
    logind_available: bool,
    nested: bool,
    /// Demo de movimento (só aninhado): dispara entrada/balanço/menu/hover sem entrada do usuário.
    demo: Option<motion::DemoMode>,
}

///TODO: this is custom code that should be better handled by libcosmic
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dropdown {
    Keyboard,
    Power,
}

/// Messages that are used specifically by our [`App`].
#[derive(Clone, Debug)]
pub enum Message {
    None,
    Common(common::Message),
    OutputEvent(OutputEvent, WlOutput),
    SessionLockEvent(SessionLockEvent),
    Channel(mpsc::Sender<String>),
    BackgroundState(cosmic_bg_config::state::State),
    DropdownToggle(Dropdown),
    KeyboardLayout(usize),
    Inhibit(Arc<OwnedFd>),
    Submit(String),
    Surface(surface::Action<Message>),
    Suspend,
    Power(PowerAction),
    TimeAppletConfig(TimeAppletConfig),
    Error(String),
    Lock,
    Unlock,
    /// Tick de animação (só enquanto `motion.is_animating()`).
    Frame(Instant),
    /// Fim do fade de saída: agora sim `unlock()`.
    UnlockReady,
    /// Hover de um alvo (campo, seta, ⏻) numa superfície.
    Hover(SurfaceId, motion::HoverTarget, bool),
    /// O cursor saiu da superfície: apaga os hovers dela.
    CursorLeft(SurfaceId),
    /// Foco do campo de senha (clique / Tab / Esc).
    FieldFocus(bool),
    /// Passo do roteiro de demo.
    DemoTick,
    /// Release systemd-logind delay sleep inhibitor after lock surfaces have committed a frame.
    ReleaseInhibit,
    /// Resultado do job de desfoque/análise do papel de parede da superfície principal.
    Backdrop(SurfaceId, Result<backdrop::Prepared, String>),
    /// Resultado da rotação por horário lida fora da UI: fatia de `rotate_interval` que o pediu e o
    /// papel de parede (`None` = nada a trocar).
    Wallpaper(u64, Option<LoadedWallpaper>),
}

/// Papel de parede escolhido e lido pela rotação, já fora da thread de UI.
#[derive(Clone, Debug)]
pub struct LoadedWallpaper {
    /// Origem (caminho + mtime); `None` quando a escolha ficou sem papel de parede custom.
    source: Option<WallpaperSource>,
    /// Handle lido; `None` se a leitura falhou ou não há papel de parede custom.
    handle: Option<widget::image::Handle>,
}

impl From<common::Message> for Message {
    fn from(message: common::Message) -> Self {
        Self::Common(message)
    }
}

#[derive(Clone, Debug)]
enum State {
    Locking,
    Locked {
        task_handle: cosmic::iced::task::Handle,
    },
    Unlocking,
    Unlocked,
}

impl State {
    fn kind(&self) -> StateKind {
        match self {
            State::Locking => StateKind::Locking,
            State::Locked { .. } => StateKind::Locked,
            State::Unlocking => StateKind::Unlocking,
            State::Unlocked => StateKind::Unlocked,
        }
    }
}

impl Drop for State {
    fn drop(&mut self) {
        // Abort the locked task when the state is changed.
        if let Self::Locked { task_handle } = self {
            tracing::info!("dropping lockscreen tasks");
            task_handle.abort();
        }
    }
}

/// The [`App`] stores application-specific state.
pub struct App {
    common: Common<Message>,
    flags: Flags,
    state: State,
    dropdown_opt: Option<Dropdown>,
    inhibit_opt: Option<Arc<OwnedFd>>,
    value_tx_opt: Option<mpsc::Sender<String>>,
    authenticating: bool,
    motion: Motion<SurfaceId>,
    /// Um `Lock` chegou durante a saída/`Unlocking`: bloquear de novo no `Unlocked`.
    relock_pending: bool,
    /// Próximo passo do roteiro de demo.
    demo_slot: u64,
    /// Visual style & wallpaper rotation config (`~/.config/cosmic-taskbar/greeter.toml`).
    style_config: StyleConfig,
    /// Loaded custom/rotating wallpaper handle, if configured.
    custom_wallpaper: Option<widget::image::Handle>,
    /// Number of lock cycles (used for `rotate_on_lock`).
    lock_count: u64,
    /// Arquivo (e mtime) de onde veio `custom_wallpaper`: papel de parede igual não é recarregado,
    /// então o handle (e o desfoque já pronto dele) sobrevive entre bloqueios.
    wallpaper_src: Option<WallpaperSource>,
    /// Fatia de `rotate_interval` da última escolha; `Tick` recarrega quando ela muda.
    last_rotation_slot: Option<u64>,
    /// Desfoque e análise do fundo por superfície principal (preenchido por jobs assíncronos).
    backdrops: BackdropSet<SurfaceId>,
}

/// De onde veio o papel de parede custom: caminho e data de modificação.
type WallpaperSource = (PathBuf, Option<SystemTime>);

/// Segundos desde a época Unix (0 se o relógio estiver antes dela).
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Lê a origem (caminho + mtime) do papel de parede escolhido.
fn wallpaper_source(path: PathBuf) -> WallpaperSource {
    let modified = fs::metadata(&path).and_then(|m| m.modified()).ok();
    (path, modified)
}

/// O papel de parede já carregado continua valendo: mesma origem e, se há origem, handle presente.
fn wallpaper_unchanged(
    current: Option<&WallpaperSource>,
    next: Option<&WallpaperSource>,
    has_handle: bool,
) -> bool {
    current == next && has_handle == next.is_some()
}

/// O resultado da rotação ainda vale: a fatia que o pediu é a última escolhida (resultado de fatia
/// velha é descartado).
fn wallpaper_slice_is_current(current: Option<u64>, result_slot: u64) -> bool {
    current == Some(result_slot)
}

/// Lê `picked` (I/O: metadados e arquivo; só chamar fora da thread de UI). `None` quando a origem é a
/// já carregada (`current` + `has_handle`), para o handle e o desfoque prontos continuarem valendo.
fn load_rotated_wallpaper(
    picked: Option<PathBuf>,
    current: Option<&WallpaperSource>,
    has_handle: bool,
) -> Option<LoadedWallpaper> {
    let next = picked.map(wallpaper_source);
    if wallpaper_unchanged(current, next.as_ref(), has_handle) {
        return None;
    }
    let handle = next
        .as_ref()
        .and_then(|(path, _)| style_config::load_wallpaper_handle(path));
    Some(LoadedWallpaper { source: next, handle })
}

impl App {
    /// A linha de status que `menu` mostra (mesma escolha, ver `status_for`).
    fn current_status(&self) -> login_ui::StatusLine<'_> {
        status_for(
            &self.common.prompt_opt,
            self.authenticating,
            self.common.caps_lock,
            self.common.error_opt.as_deref(),
        )
    }

    /// Reload style configuration and pick/load wallpaper for the current lock cycle.
    fn reload_style_and_wallpaper(&mut self) {
        self.style_config = StyleConfig::load();
        let now = unix_now();
        self.last_rotation_slot = self.style_config.rotation_slot(now);
        let picked = self.style_config.pick_wallpaper_at(self.lock_count, now);
        self.apply_wallpaper(picked);
    }

    /// Carrega `picked` como papel de parede custom, a menos que seja o mesmo arquivo já carregado
    /// (mesmo handle = mesmo desfoque, sem refazer o job nem piscar a imagem nítida).
    fn apply_wallpaper(&mut self, picked: Option<PathBuf>) {
        let next = picked.map(wallpaper_source);
        if wallpaper_unchanged(self.wallpaper_src.as_ref(), next.as_ref(), self.custom_wallpaper.is_some()) {
            return;
        }
        self.custom_wallpaper = next
            .as_ref()
            .and_then(|(path, _)| style_config::load_wallpaper_handle(path));
        self.wallpaper_src = next;
    }

    /// Rotação por horário: quando a fatia de `rotate_interval` muda, escolhe e lê o próximo papel de
    /// parede fora da thread de UI (`Message::Wallpaper`); prompt, foco e texto digitado não são
    /// tocados. Só enquanto bloqueado; ao bloquear de novo `reload_style_and_wallpaper` já escolhe.
    fn rotate_wallpaper_if_due(&mut self) -> Task<Message> {
        if matches!(self.state, State::Unlocked) {
            return Task::none();
        }
        let now = unix_now();
        let slot = self.style_config.rotation_slot(now);
        if !style_config::rotation_due(self.last_rotation_slot, slot) {
            return Task::none();
        }
        let Some(slot_value) = slot else {
            return Task::none();
        };
        self.last_rotation_slot = slot;
        let cfg = self.style_config.clone();
        let lock_count = self.lock_count;
        let current = self.wallpaper_src.clone();
        let has_handle = self.custom_wallpaper.is_some();
        cosmic::task::future(async move {
            let loaded = task::spawn_blocking(move || {
                let picked = cfg.pick_wallpaper_at(lock_count, now);
                load_rotated_wallpaper(picked, current.as_ref(), has_handle)
            })
            .await
            .unwrap_or(None);
            Message::Wallpaper(slot_value, loaded)
        })
    }

    /// Aplica o resultado da rotação (já lido fora da UI), a menos que a origem não tenha mudado
    /// entre o pedido e a resposta.
    fn apply_loaded_wallpaper(&mut self, loaded: LoadedWallpaper) {
        if wallpaper_unchanged(
            self.wallpaper_src.as_ref(),
            loaded.source.as_ref(),
            self.custom_wallpaper.is_some(),
        ) {
            return;
        }
        self.custom_wallpaper = loaded.handle;
        self.wallpaper_src = loaded.source;
    }

    /// Imagem de fundo da superfície principal `id`: a custom da config ou a do cosmic-bg.
    fn original_wallpaper(&self, id: SurfaceId) -> Option<&widget::image::Handle> {
        self.custom_wallpaper
            .as_ref()
            .or_else(|| self.common.surface_images.get(&id))
    }

    /// Identidade do que a superfície principal `id` mostra (`None` sem imagem ou sem tamanho).
    fn backdrop_tag(&self, id: SurfaceId) -> Option<backdrop::Tag> {
        backdrop::surface_tag(
            self.original_wallpaper(id)?,
            self.style_config.blur_radius,
            self.common.output_size(id),
        )
    }

    /// Dispara o desfoque/análise de cada superfície que ainda não tem o resultado do seu `tag`.
    /// Só decodifica/desfoca/hasheia dentro do job (`prepare_async`); aqui só clona `Bytes`.
    fn spawn_backdrops(&mut self) -> Task<Message> {
        if matches!(self.state, State::Unlocked) {
            return Task::none();
        }
        let ids: Vec<SurfaceId> = self.common.surface_ids.values().copied().collect();
        let mut tasks = Vec::new();
        for id in ids {
            let Some(tag) = self.backdrop_tag(id) else {
                continue;
            };
            if !self.backdrops.wants(id, tag) {
                continue;
            }
            let Some(bytes) = self.original_wallpaper(id).and_then(backdrop::handle_bytes) else {
                continue;
            };
            self.backdrops.mark_pending(id, tag);
            tasks.push(cosmic::task::future(async move {
                // `cache_dir` cria a pasta (I/O): também fora da thread de UI.
                let cache = task::spawn_blocking(|| backdrop::cache_dir(false))
                    .await
                    .ok()
                    .flatten();
                Message::Backdrop(id, backdrop::prepare_async(bytes, tag, cache).await)
            }));
        }
        Task::batch(tasks)
    }

    /// O antigo ramo `State::Unlocked` de `Message::Lock`, sem mudança (mais `cancel_exit`).
    fn start_lock(&mut self) -> Task<Message> {
        tracing::info!("session locking");
        self.lock_count = self.lock_count.saturating_add(1);
        self.reload_style_and_wallpaper();
        self.relock_pending = false;
        self.state = State::Locking;
        self.motion.cancel_exit();
        // Clear errors
        self.common.error_opt = None;
        // Clear value_tx
        self.value_tx_opt = None;
        // Reset authenticating state
        self.authenticating = false;
        // Try to create lockfile when locking
        if let Some(ref lockfile) = self.flags.lockfile_opt
            && let Err(err) = fs::File::create(lockfile)
        {
            tracing::warn!("failed to create lockfile {:?}: {}", lockfile, err);
        }
        // Tell compositor to lock
        lock()
    }

    /// O antigo corpo de `Message::Unlock`, sem mudança: destrói as superfícies e chama `unlock()`.
    fn finish_unlock(&mut self) -> Task<Message> {
        match self.state {
            State::Locked { .. } => {
                tracing::info!("sessing unlocking");
                self.state = State::Unlocking;
                // Clear errors
                self.common.error_opt = None;
                // Clear value_tx
                self.value_tx_opt = None;
                // Stop authenticating
                self.authenticating = false;

                // Try to delete lockfile when unlocking
                if let Some(ref lockfile) = self.flags.lockfile_opt
                    && let Err(err) = fs::remove_file(lockfile)
                {
                    tracing::warn!("failed to remove lockfile {:?}: {}", lockfile, err);
                }

                // Destroy lock surfaces
                let mut commands = Vec::with_capacity(self.common.surface_ids.len() + 1);

                for (_output, surface_id) in self.common.surface_ids.iter() {
                    self.common.surface_names.remove(surface_id);
                    self.common.window_size.remove(surface_id);
                    commands.push(destroy_lock_surface(*surface_id));
                }

                // Tell compositor to unlock
                commands.push(unlock());

                // Wait to exit until `Unlocked` event, when server has processed unlock
                return Task::batch(commands);
            }
            State::Locking => {
                tracing::info!("session still locking");
            }
            State::Unlocking | State::Unlocked => {
                tracing::info!("session already unlocking or unlocked");
            }
        }
        Task::none()
    }

    /// Toda ação de energia do locker passa por aqui (com `nested`, nada chama o logind).
    fn power_task(action: PowerAction, nested: bool) -> Task<Message> {
        cosmic::Task::future(async move { power::perform(action, nested).await.err() }).and_then(
            move |err| {
                tracing::error!("failed to {:?}: {}", action, err);
                cosmic::task::message(cosmic::Action::App(Message::Error(err)))
            },
        )
    }

    /// Tela estilizada (5 layouts + tipografia configurável).
    fn menu(&self, surface_id: SurfaceId) -> Element<'_, Message> {
        crate::perf::count(crate::perf::Counter::View);
        crate::perf::mark_first_view();
        let cfg = &self.style_config;
        let military_time = cfg.clock_24h;
        let metrics = login_ui::screen_metrics(self.common.screen_height(surface_id));
        let (hour, minute) = self.common.time.format_hour_minute(military_time);
        let clock_data = login_ui::ClockData {
            time: self
                .common
                .time
                .format_time_opts(military_time, cfg.show_seconds),
            hour,
            minute,
            date: self.common.time.format_date(),
            short_date: self.common.time.format_date_short(),
            look: self.motion.look(Part::Clock),
        };

        let accepts = accepts_input(self.authenticating, self.motion.exiting());
        let field_look = self.motion.look(Part::Field);
        let mut field = None;
        if let Some((prompt, secret, value_opt)) = &self.common.prompt_opt {
            match value_opt {
                Some(value) => {
                    let text_input_id = self
                        .common
                        .surface_names
                        .get(&surface_id)
                        .and_then(|id| self.common.text_input_ids.get(id))
                        .cloned()
                        .unwrap_or_else(|| cosmic::widget::Id::new("text_input"));

                    let mut text_input = login_ui::password_input(
                        login_ui::field_placeholder(prompt, *secret),
                        value.as_str(),
                        *secret,
                        common::Message::Prompt(prompt.clone(), !*secret, Some(value.clone()))
                            .into(),
                        field_look.alpha,
                    )
                    .id(text_input_id);

                    // Don't allow input when authenticating (nor during the exit fade)
                    if accepts {
                        text_input = text_input
                            .on_input(|input| {
                                common::Message::Prompt(prompt.clone(), *secret, Some(input)).into()
                            })
                            .on_submit(Message::Submit);
                    }
                    text_input = text_input
                        .on_focus(Message::FieldFocus(true))
                        .on_unfocus(Message::FieldFocus(false));

                    let submit = (accepts && !value.is_empty())
                        .then(|| Message::Submit(value.clone()));
                    field = Some(styles::FieldParts {
                        input: text_input,
                        submit,
                        fx: login_ui::FieldFx {
                            look: field_look,
                            hover: self.motion.hover_t(surface_id, HoverTarget::Field),
                            focus: self.motion.focus_t(),
                            submit_hover: self.motion.hover_t(surface_id, HoverTarget::Submit),
                            field_hovers: Some(hovers(surface_id, HoverTarget::Field)),
                            submit_hovers: Some(hovers(surface_id, HoverTarget::Submit)),
                        },
                    });
                }
                None => {}
            }
        }

        let power = power::menu_visible(self.flags.logind_available, self.flags.nested).then(|| {
            styles::PowerParts {
                open: self.dropdown_opt == Some(Dropdown::Power),
                on_toggle: Message::DropdownToggle(Dropdown::Power),
                on_action: Message::Power,
                fx: login_ui::PowerFx {
                    look: self.motion.look(Part::Power),
                    hover: self.motion.hover_t(surface_id, HoverTarget::Power),
                    menu: self.motion.menu_look(),
                    hovers: Some(hovers(surface_id, HoverTarget::Power)),
                },
            }
        });

        let keyboard = style_config::wants_keyboard_menu(self.common.active_layouts.len())
            .then(|| styles::ChoiceMenu {
                open: self.dropdown_opt == Some(Dropdown::Keyboard),
                on_toggle: Message::DropdownToggle(Dropdown::Keyboard),
                label: self
                    .common
                    .active_layouts
                    .get(self.common.current_keyboard_layout)
                    .map(|layout| layout.name())
                    .unwrap_or_default(),
                items: self
                    .common
                    .active_layouts
                    .iter()
                    .enumerate()
                    .map(|(i, layout)| {
                        (
                            layout.description.clone(),
                            i == self.common.current_keyboard_layout,
                            Message::KeyboardLayout(i),
                        )
                    })
                    .collect(),
            });

        // Análise do fundo do monitor desta subsuperfície; só vale se for do que está na tela.
        let main = self.common.main_surface_of(surface_id).unwrap_or(surface_id);
        let backdrop = self
            .backdrop_tag(main)
            .and_then(|tag| self.backdrops.get(main).filter(|p| p.tag == tag))
            .map(|p| &p.analysis);

        styles::view(styles::ScreenInput {
            cfg,
            metrics,
            clock: clock_data,
            user: styles::UserParts {
                icon: self.flags.user_icon.as_ref(),
                full_name: &self.flags.user_data.full_name,
            },
            field,
            status: self.current_status(),
            looks: styles::Looks {
                clock: self.motion.look(Part::Clock),
                avatar: self.motion.look(Part::Avatar),
                name: self.motion.look(Part::Name),
                field: field_look,
                status: self.motion.look(Part::Status),
                power: self.motion.look(Part::Power),
            },
            power,
            session: None,
            keyboard,
            system: &self.common.system_info,
            backdrop,
        })
    }

    /// Layout original do upstream (cartão com relógio grande); fica escondido, não apagado.
    #[allow(dead_code)]
    fn menu_upstream(&self, surface_id: SurfaceId) -> Element<'_, Message> {
        let window_width = self
            .common
            .window_size
            .get(&surface_id)
            .map(|s| s.width)
            .unwrap_or(MAX_WIDTH);
        let menu_width = if window_width > MAX_WIDTH {
            MAX_WIDTH
        } else {
            window_width
        };
        let left_element = {
            let military_time = self.flags.user_data.time_applet_config.military_time;
            let date_time_column = self.common.time.date_time_widget(military_time);

            let mut status_row = widget::row::with_capacity(2).padding(16.0).spacing(12.0);

            if let Some(network_icon) = self.common.network_icon_opt.as_ref() {
                status_row = status_row.push(network_icon.clone());
            }

            if let Some((power_icon, power_percent)) = &self.common.power_info_opt {
                status_row = status_row.push(
                    iced::widget::row![
                        power_icon.clone(),
                        widget::text(format!("{:.0}%", power_percent)),
                    ]
                    .align_y(Alignment::Center),
                );
            }

            //TODO: move code for custom dropdowns to libcosmic
            let menu_checklist = |label, value, message| {
                Element::from(
                    widget::menu::menu_button(vec![
                        if value {
                            widget::icon::from_name("object-select-symbolic")
                                .size(16)
                                .icon()
                                .width(Length::Fixed(16.0))
                                .into()
                        } else {
                            widget::space::horizontal()
                                .width(Length::Fixed(17.0))
                                .into()
                        },
                        widget::space::horizontal().width(Length::Fixed(8.0)).into(),
                        widget::text(label)
                            .align_x(iced::alignment::Horizontal::Left)
                            .into(),
                    ])
                    .on_press(message),
                )
            };
            let dropdown_menu = |items: Vec<_>| {
                let item_cnt = items.len();

                let items = widget::menu::menu_column::MenuColumn::with_children(items);
                let items = if item_cnt > 7 {
                    Element::from(
                        widget::scrollable(items)
                            .height(Length::Fixed(DEFAULT_MENU_ITEM_HEIGHT * 7.)),
                    )
                } else {
                    Element::from(items)
                };

                let menu: widget::Container<'_, Message, cosmic::prelude::Theme> =
                    widget::container(items)
                        .padding(1)
                        //TODO: move style to libcosmic
                        .class(theme::Container::custom(|theme| {
                            let cosmic = theme.cosmic();
                            let component = &cosmic.background(theme.transparent).component;
                            widget::container::Style {
                                icon_color: Some(component.on.into()),
                                text_color: Some(component.on.into()),
                                background: Some(Background::Color(component.base.into())),
                                border: Border {
                                    radius: 8.0.into(),
                                    width: 1.0,
                                    color: component.divider.into(),
                                },
                                ..Default::default()
                            }
                        }))
                        .width(Length::Fixed(240.0));

                if let Some(t) = self.common.rectangle_tracker.as_ref() {
                    Element::from(t.container((surface_id, true), menu))
                } else {
                    menu.into()
                }
            };

            let mut input_button = widget::popover(
                widget::container(
                    widget::button::custom(
                        widget::text(
                            self.common
                                .active_layouts
                                .get(self.common.current_keyboard_layout)
                                .map(|x| x.name())
                                .unwrap_or_default(),
                        )
                        .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
                        .height(16)
                        .center(),
                    )
                    .padding(12.0)
                    .on_press(Message::DropdownToggle(Dropdown::Keyboard)),
                )
                .max_width(80),
            )
            .position(widget::popover::Position::Bottom);
            if matches!(self.dropdown_opt, Some(Dropdown::Keyboard)) {
                let mut items = Vec::with_capacity(self.common.active_layouts.len());
                for (i, layout) in self.common.active_layouts.iter().enumerate() {
                    items.push(menu_checklist(
                        &layout.description,
                        i == self.common.current_keyboard_layout,
                        Message::KeyboardLayout(i),
                    ));
                }
                input_button = input_button.popup(dropdown_menu(items));
            }

            //TODO: implement these buttons
            let mut button_row = iced::widget::row![
                /*TODO: greeter accessibility options
                widget::button::custom(widget::icon::from_name(
                    "applications-accessibility-symbolic"
                ))
                .padding(12.0)
                .on_press(Message::None),
                */
                widget::tooltip(
                    widget::button::custom(widget::icon::from_name("input-keyboard-symbolic"))
                        .padding(12.0)
                        .on_press(Message::Common(common::Message::OnScreenKeyboard)),
                    widget::text(fl!("on-screen-keyboard")),
                    widget::tooltip::Position::Top,
                ),
                widget::tooltip(
                    input_button,
                    widget::text(fl!("keyboard-layout")),
                    widget::tooltip::Position::Top
                ),
            ];
            if cfg!(feature = "logind") && self.flags.logind_available {
                button_row = button_row.push(widget::tooltip(
                    widget::button::custom(widget::icon::from_name("system-suspend-symbolic"))
                        .padding(12.0)
                        .on_press(Message::Suspend),
                    widget::text(fl!("suspend")),
                    widget::tooltip::Position::Top,
                ));
            }
            let button_row = button_row.padding([16.0, 0.0, 0.0, 0.0]).spacing(8.0);

            widget::container(iced::widget::column![
                date_time_column,
                widget::divider::horizontal::default().width(Length::Fixed(menu_width / 2. - 16.)),
                status_row,
                widget::divider::horizontal::default().width(Length::Fixed(menu_width / 2. - 16.)),
                button_row,
            ])
            .align_x(Alignment::Start)
        };

        let right_element = {
            let mut column = widget::column::with_capacity(5)
                .spacing(12.0)
                .max_width(280.0);

            let military_time = self.flags.user_data.time_applet_config.military_time;
            let space_height = match military_time {
                true => 63.0,
                false => 10.0,
            };

            // Add top spacing for better visual appearance
            // Bottom of the password text input field should align with bottom of time widget
            column = column.push(widget::space::vertical().height(Length::Fixed(space_height)));

            // Display user icon or empty transparent box
            if let Some(icon_handle) = &self.flags.user_icon {
                column = column.push(
                    widget::container(
                        widget::image(icon_handle)
                            .width(Length::Fixed(78.0))
                            .height(Length::Fixed(78.0))
                            .content_fit(iced::ContentFit::Fill),
                    )
                    .padding(0.0)
                    .width(Length::Fill)
                    .height(Length::Fixed(78.0))
                    .align_x(Alignment::Center),
                );
            } else {
                // Empty transparent box for users without icons
                column = column.push(
                    widget::container(widget::space::horizontal().width(Length::Fixed(78.0)))
                        .padding(0.0)
                        .width(Length::Fill)
                        .height(Length::Fixed(78.0))
                        .align_x(Alignment::Center),
                );
            }

            column = column.push(
                widget::container(widget::text::title4(&self.flags.user_data.full_name))
                    .width(Length::Fill)
                    .align_x(Alignment::Center),
            );

            if let Some((prompt, secret, value_opt)) = &self.common.prompt_opt {
                match value_opt {
                    Some(value) => {
                        let text_input_id = self
                            .common
                            .surface_names
                            .get(&surface_id)
                            .and_then(|id| self.common.text_input_ids.get(id))
                            .cloned()
                            .unwrap_or_else(|| cosmic::widget::Id::new("text_input"));

                        let mut text_input = widget::secure_input(
                            prompt.clone(),
                            value.as_str(),
                            Some(
                                common::Message::Prompt(
                                    prompt.clone(),
                                    !*secret,
                                    Some(value.clone()),
                                )
                                .into(),
                            ),
                            *secret,
                        )
                        .id(text_input_id);

                        // Don't allow input when authenticating
                        if !self.authenticating {
                            text_input = text_input
                                .on_input(|input| {
                                    common::Message::Prompt(prompt.clone(), *secret, Some(input))
                                        .into()
                                })
                                .on_submit(Message::Submit);
                        }

                        if *secret {
                            text_input = text_input.password()
                        }

                        column = column.push(text_input);

                        if self.common.caps_lock && !self.authenticating {
                            column = column.push(widget::text(fl!("caps-lock")));
                        } else if self.common.error_opt.is_none() {
                            column = column.push(widget::text(""));
                        }
                    }
                    None => {
                        column = column.push(widget::text(prompt));
                    }
                }
            }

            // Show either authenticating message or error message in the same location
            if self.authenticating {
                column = column.push(
                    widget::container(
                        widget::row::with_capacity(2)
                            .spacing(8.0)
                            .align_y(Alignment::Center)
                            .push(widget::indeterminate_circular().size(16.0).bar_height(2.0))
                            .push(widget::text(fl!("authenticating"))),
                    )
                    .width(Length::Fill)
                    .align_x(Alignment::Center),
                );
            } else if let Some(error) = &self.common.error_opt {
                column = column.push(
                    widget::text(error)
                        .class(theme::Text::Color(iced::Color::from_rgb(1.0, 0.0, 0.0))),
                );
                if !self.common.caps_lock {
                    column = column.push(widget::text(""));
                }
            } else {
                column = column.push(widget::text(""));
            }

            widget::container(column)
                .align_x(Alignment::Center)
                .width(Length::Fill)
        };
        let menu = widget::layer_container(
            iced::widget::row![left_element, right_element].align_y(Alignment::Start),
        )
        .layer(cosmic::cosmic_theme::Layer::Background)
        .padding(16)
        .class(cosmic::theme::Container::Custom(Box::new(
            |theme: &cosmic::Theme| {
                // Use background appearance as the base
                let mut appearance =
                    widget::container::Catalog::style(theme, &cosmic::theme::Container::Background);
                let c: iced::Color = theme.cosmic().background(theme.transparent).base.into();
                appearance.background = Some(iced::Background::Color(c));
                appearance.border = iced::Border::default().rounded(16.0);
                appearance
            },
        )))
        .width(Length::Fill)
        .height(Length::Shrink);
        let menu = if let Some(t) = self.common.rectangle_tracker.as_ref() {
            Element::from(t.container((surface_id, false), menu))
        } else {
            menu.into()
        };
        widget::container(widget::column::with_children(vec![
            widget::space::vertical()
                .height(Length::FillPortion(1))
                .into(),
            menu,
            widget::space::vertical()
                .height(Length::FillPortion(4))
                .into(),
        ]))
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .class(cosmic::theme::Container::Transparent)
        .into()
    }
}

/// Implement [`cosmic::Application`] to integrate with COSMIC.
impl cosmic::Application for App {
    /// Default async executor to use with the app.
    type Executor = executor::Default;

    /// Argument received [`cosmic::Application::new`].
    type Flags = Flags;

    /// Message type specific to our [`App`].
    type Message = Message;

    /// The unique application ID to supply to the window manager.
    const APP_ID: &'static str = "com.system76.CosmicGreeter";

    fn core(&self) -> &Core {
        &self.common.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.common.core
    }

    /// Creates the application, and optionally emits command on initialize.
    fn init(mut core: Core, flags: Self::Flags) -> (Self, Task<Self::Message>) {
        core.set_app_type(cosmic::core::AppType::System);

        let (mut common, common_task) = Common::init(core);
        common.on_output_event = Some(Box::new(|output_event, output| {
            Message::OutputEvent(output_event, output)
        }));
        common.on_session_lock_event = Some(Box::new(Message::SessionLockEvent));
        common.update_user_data(&flags.user_data);

        let already_locked = match flags.lockfile_opt {
            Some(ref lockfile) => lockfile.exists(),
            None => false,
        };

        let style_config = StyleConfig::load();
        let now_secs = unix_now();
        let wallpaper_src = style_config.pick_wallpaper_at(0, now_secs).map(wallpaper_source);
        let custom_wallpaper = wallpaper_src
            .as_ref()
            .and_then(|(path, _)| style_config::load_wallpaper_handle(path));
        let last_rotation_slot = style_config.rotation_slot(now_secs);

        let mut app = App {
            common,
            flags,
            state: State::Unlocked,
            dropdown_opt: None,
            inhibit_opt: None,
            value_tx_opt: None,
            authenticating: false,
            motion: Motion::new(Instant::now()),
            relock_pending: false,
            demo_slot: 0,
            style_config,
            custom_wallpaper,
            lock_count: 0,
            wallpaper_src,
            last_rotation_slot,
            backdrops: BackdropSet::default(),
        };
        // Grava a chave do status sem fade (nada de fade de 150 ms na partida).
        let key = login_ui::status_key(&app.current_status());
        app.motion.set_status_key(key);

        let task = if cfg!(feature = "logind") && app.flags.logind_available {
            if already_locked {
                // Recover previously locked state
                tracing::info!("recovering previous locked state");
                app.state = State::Locking;
                lock()
            } else {
                // When logind is available, wait for lock signal
                Task::none()
            }
        } else {
            // When logind is not available, lock immediately
            tracing::info!("locking immediately");
            app.state = State::Locking;
            lock()
        };

        (app, Task::batch([task, common_task]))
    }

    /// Handle application events here.
    /// tick → `update_inner` → observa o status (fade só quando a linha muda).
    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        crate::perf::count(crate::perf::Counter::Update);
        let now = Instant::now();
        self.motion.tick(now);
        // Quadros de animação e hover não mudam imagem nem tamanho: nem olham os fundos.
        let touches_wallpaper = !matches!(
            message,
            Message::Frame(_) | Message::Hover(..) | Message::CursorLeft(_)
        );
        let task = self.update_inner(message);
        let key = login_ui::status_key(&self.current_status());
        self.motion.observe_status(key, now);
        if touches_wallpaper {
            // Bloqueio, imagem/tamanho novos e troca de papel de parede: um job por tag novo.
            return Task::batch([task, self.spawn_backdrops()]);
        }
        task
    }

    // Not used for layer surface window
    fn view(&self) -> Element<'_, Self::Message> {
        unimplemented!()
    }

    /// Creates a view after each update.
    fn view_window(&self, surface_id: SurfaceId) -> Element<'_, Self::Message> {
        crate::perf::count(crate::perf::Counter::WallpaperView);
        // A primeira pintura nunca espera o job: sem resultado do `tag` atual, vai a imagem original.
        let img = match self.original_wallpaper(surface_id) {
            Some(original) => match self.backdrop_tag(surface_id) {
                // Rotação com tag novo pendente: segue o desfocado anterior, sem piscar a original.
                Some(tag) => self.backdrops.view_handle(
                    surface_id,
                    self.style_config.blur_radius,
                    tag,
                    original,
                ),
                None => original,
            },
            None => &self.common.fallback_background,
        };
        login_ui::wallpaper_view(img, self.style_config.dim_opacity)
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let mut subscriptions = Vec::with_capacity(7);

        subscriptions.push(self.common.subscription().map(Message::from));

        struct BackgroundSubscription;
        subscriptions.push(
            cosmic_config::config_state_subscription(
                TypeId::of::<BackgroundSubscription>(),
                cosmic_bg_config::NAME.into(),
                cosmic_bg_config::state::State::version(),
            )
            .map(|res| {
                if !res.errors.is_empty() {
                    tracing::info!("errors loading background state: {:?}", res.errors);
                }
                Message::BackgroundState(res.config)
            }),
        );

        struct TimeAppletSubscription;
        subscriptions.push(
            cosmic_config::config_subscription(
                TypeId::of::<TimeAppletSubscription>(),
                "com.system76.CosmicAppletTime".into(),
                TimeAppletConfig::VERSION,
            )
            .map(|res| {
                if !res.errors.is_empty() {
                    tracing::info!("errors loading background state: {:?}", res.errors);
                }
                Message::TimeAppletConfig(res.config)
            }),
        );

        #[cfg(feature = "logind")]
        if self.flags.logind_available {
            subscriptions.push(crate::logind::subscription());
        }

        // Ticks de animação: a assinatura só existe enquanto algo se mexe.
        subscriptions.push(self.motion.frames().map(Message::Frame));

        // Mouse saindo da superfície (o `mouse_area` não avisa isso).
        subscriptions.push(cosmic::iced::event::listen_with(|event, _status, id| match event {
            iced::Event::Mouse(iced::mouse::Event::CursorLeft) => Some(Message::CursorLeft(id)),
            _ => None,
        }));

        // Roteiro de demo (só aninhado com a variável de ambiente).
        subscriptions.push(motion::demo_ticks(self.flags.demo).map(|_| Message::DemoTick));

        if self.style_config.needs_second_tick() {
            subscriptions.push(
                cosmic::iced::time::every(std::time::Duration::from_secs(1))
                    .map(|_| Message::Common(common::Message::Tick)),
            );
        }

        Subscription::batch(subscriptions)
    }
}

impl App {
    /// O antigo `update`: os braços ficam como estavam.
    fn update_inner(&mut self, message: Message) -> Task<Message> {
        let now = self.motion.now();
        match message {
            Message::None => {}
            Message::Common(common_message) => {
                if matches!(&common_message, common::Message::Prompt(_, _, Some(_)))
                    && self.authenticating
                {
                    self.authenticating = false;
                    self.common.prompt_opt = None;
                }
                // A superfície apareceu: a cascata de entrada começa (se armada).
                if matches!(&common_message, common::Message::SubsurfaceOpened(_)) {
                    self.motion.start_entry(now);
                    self.motion.set_focus(true, now);
                }
                // Rotação por horário no `Tick` que já existe (sem timer novo).
                let is_tick = matches!(&common_message, common::Message::Tick);
                let task = self.common.update(common_message);
                if is_tick {
                    return Task::batch([task, self.rotate_wallpaper_if_due()]);
                }
                return task;
            }
            Message::Backdrop(surface_id, result) => match result {
                Ok(prepared) => {
                    // Resultado de um pedido já superado (outra imagem/tamanho) é descartado.
                    if !self.backdrops.accept(surface_id, prepared) {
                        tracing::debug!("fundo desatualizado descartado para {surface_id:?}");
                    }
                }
                Err(err) => {
                    // O pedido segue marcado como pendente: não há nova tentativa para o mesmo
                    // tag, então este aviso sai uma vez só; a tela segue com a imagem original.
                    tracing::warn!("fundo da superfície {surface_id:?} sem desfoque/análise: {err}");
                    // O desfocado anterior era de outra imagem: volta à original deste tag.
                    self.backdrops.drop_ready(surface_id);
                }
            },
            Message::Wallpaper(slot, loaded) => {
                if !wallpaper_slice_is_current(self.last_rotation_slot, slot) {
                    tracing::debug!("papel de parede da fatia {slot} descartado (fatia velha)");
                } else if let Some(loaded) = loaded {
                    self.apply_loaded_wallpaper(loaded);
                }
            }
            Message::OutputEvent(output_event, output) => {
                match output_event {
                    OutputEvent::Created(output_info_opt) => {
                        tracing::info!("output {}: created", output.id());

                        let surface_id = SurfaceId::unique();
                        let subsurface_id = SurfaceId::unique();

                        if let Some(old_surface_id) =
                            self.common.surface_ids.insert(output.clone(), surface_id)
                        {
                            //TODO: remove old surface?
                            tracing::warn!(
                                "output {}: already had surface ID {:?}",
                                output.id(),
                                old_surface_id
                            );
                            return Task::none();
                        }

                        self.common
                            .subsurface_outputs
                            .insert(subsurface_id, output.clone());

                        let rect = common::menu_rect(
                            output_info_opt.as_ref().and_then(|info| info.logical_size),
                        );
                        match output_info_opt {
                            Some(output_info) => match output_info.name {
                                Some(output_name) => {
                                    self.common
                                        .output_names
                                        .insert(output.clone(), output_name.clone());
                                    self.common
                                        .surface_names
                                        .insert(surface_id, output_name.clone());
                                    self.common
                                        .surface_names
                                        .insert(subsurface_id, output_name.clone());
                                    self.common.surface_images.remove(&surface_id);
                                    self.common.update_wallpapers(&self.flags.user_data);
                                    let text_input_id =
                                        widget::Id::new(format!("input-{output_name}",));
                                    self.common
                                        .text_input_ids
                                        .insert(output_name.clone(), text_input_id.clone());
                                }
                                None => {
                                    tracing::warn!("output {}: no output name", output.id());
                                }
                            },
                            None => {
                                tracing::warn!("output {}: no output info", output.id());
                            }
                        }

                        let (loc, sub_size) = (rect.position(), rect.size());
                        self.common.window_size.insert(surface_id, rect.size());

                        self.common.subsurface_rects.insert(output.clone(), rect);

                        let msg = cosmic::surface::action::subsurface(
                            move |_: &mut App| SctkSubsurfaceSettings {
                                parent: surface_id,
                                id: subsurface_id,
                                loc,
                                size: Some(sub_size),
                                z: 10,
                                steal_keyboard_focus: true,
                                gravity: Gravity::BottomRight,
                                offset: (0, 0),
                                input_zone: None,
                            },
                            Some(Box::new(move |app: &App| {
                                app.menu(subsurface_id).map(cosmic::Action::App)
                            })),
                        );

                        if matches!(self.state, State::Locked { .. }) {
                            return cosmic::task::message(cosmic::Action::Surface(
                                cosmic::surface::action::lock(
                                    || LiveSettings {
                                        padding: None,
                                        corners: None,
                                        blur: Some(false),
                                    },
                                    surface_id,
                                    output,
                                    None::<fn() -> cosmic::Element<'static, cosmic::Action<Message>>>,
                                ),
                            ))
                            .chain(cosmic::task::message(cosmic::Action::Surface(msg)));
                        }
                    }
                    OutputEvent::Removed => {
                        tracing::info!("output {}: removed", output.id());
                        match self.common.surface_ids.remove(&output) {
                            Some(surface_id) => {
                                self.backdrops.forget(surface_id);
                                self.common.surface_images.remove(&surface_id);
                                self.common.surface_names.remove(&surface_id);
                                self.common.window_size.remove(&surface_id);
                                if let Some(n) = self.common.surface_names.remove(&surface_id) {
                                    self.common.text_input_ids.remove(&n);
                                }
                                if matches!(self.state, State::Locked { .. }) {
                                    return destroy_lock_surface(surface_id);
                                }
                            }
                            None => {
                                tracing::warn!("output {}: no surface found", output.id());
                            }
                        }
                    }
                    OutputEvent::InfoUpdate(info) => {
                        self.common
                            .subsurface_rects
                            .insert(output.clone(), common::menu_rect(info.logical_size));

                        tracing::info!("output {}: info update", output.id());
                    }
                }
            }
            Message::SessionLockEvent(session_lock_event) => match session_lock_event {
                SessionLockEvent::Focused(..) => {}
                SessionLockEvent::Locked => {
                    tracing::info!("session locked");
                    if matches!(self.state, State::Locked { .. }) {
                        return Task::none();
                    }

                    // Novo ciclo: nada da saída anterior vaza; conteúdo escondido até a
                    // subsuperfície abrir (ou `ARM_TIMEOUT`).
                    self.relock_pending = false;
                    self.motion.cancel_exit();
                    self.motion.arm_entry(now);

                    let username = self.flags.user_data.name.clone();
                    let (locked_task, locked_handle) =
                        cosmic::task::stream(cosmic::iced::stream::channel(
                            16,
                            |mut msg_tx: futures::channel::mpsc::Sender<_>| async move {
                                // Sem heartbeat: o relógio vem de `crate::time::tick()` (alinhado
                                // ao minuto, ligado em `Common::init`); parado = sem mensagens.
                                let heartbeat_future = futures::future::pending::<()>();

                                let pam_future = async {
                                    loop {
                                        let (value_tx, value_rx) = mpsc::channel(16);
                                        msg_tx
                                            .send(cosmic::Action::App(Message::Channel(value_tx)))
                                            .await
                                            .unwrap();

                                        let pam_res = {
                                            let username = username.clone();
                                            let msg_tx = msg_tx.clone();
                                            task::spawn_blocking(move || {
                                                pam_thread(
                                                    username,
                                                    Conversation { msg_tx, value_rx },
                                                )
                                            })
                                            .await
                                            .unwrap()
                                        };

                                        match pam_res {
                                            Ok(()) => {
                                                tracing::info!("successfully authenticated");
                                                msg_tx
                                                    .send(cosmic::Action::App(Message::Unlock))
                                                    .await
                                                    .unwrap();
                                                break;
                                            }
                                            Err(err) => {
                                                tracing::warn!("authentication error: {}", err);
                                                msg_tx
                                                    .send(cosmic::Action::App(Message::Error(
                                                        pam_error_to_message(&err),
                                                    )))
                                                    .await
                                                    .unwrap();
                                            }
                                        }
                                    }
                                };

                                futures::pin_mut!(heartbeat_future);
                                futures::pin_mut!(pam_future);
                                futures::future::select(heartbeat_future, pam_future).await;
                            },
                        ))
                        .abortable();

                    let mut commands = Vec::with_capacity(self.common.surface_ids.len() + 1);
                    commands.push(locked_task);

                    self.state = State::Locked {
                        task_handle: locked_handle,
                    };

                    // Create lock surfaces first; keep sleep inhibitor alive until after lock
                    // surfaces have been created and had time to commit an opaque frame.
                    for (output, surface_id) in self.common.surface_ids.iter() {
                        commands.push(cosmic::task::message(cosmic::Action::Surface(
                            cosmic::surface::action::lock(
                                || LiveSettings {
                                    padding: None,
                                    corners: None,
                                    blur: Some(false),
                                },
                                *surface_id,
                                output.clone(),
                                None::<fn() -> cosmic::Element<'static, cosmic::Action<Message>>>,
                            ),
                        )));

                        if let Some((rect, name)) = self
                            .common
                            .subsurface_rects
                            .get(output)
                            .copied()
                            .zip(self.common.output_names.get(output))
                        {
                            let subsurface_id = SurfaceId::unique();
                            self.common
                                .subsurface_outputs
                                .insert(subsurface_id, output.clone());
                            let surface_id = *surface_id;
                            self.common.surface_names.insert(surface_id, name.clone());
                            self.common
                                .surface_names
                                .insert(subsurface_id, name.clone());

                            let msg = cosmic::surface::action::subsurface(
                                move |_: &mut App| SctkSubsurfaceSettings {
                                    parent: surface_id,
                                    id: subsurface_id,
                                    loc: Point::new(rect.x, rect.y),
                                    size: Some(Size::new(rect.width, rect.height)),
                                    z: 10,
                                    steal_keyboard_focus: true,
                                    gravity: Gravity::BottomRight,
                                    offset: (0, 0),
                                    input_zone: None,
                                },
                                Some(Box::new(move |app: &App| {
                                    app.menu(subsurface_id).map(cosmic::Action::App)
                                })),
                            );
                            commands.push(cosmic::task::message(cosmic::Action::Surface(msg)));
                        } else {
                            tracing::error!("no rectangle for subsurface creation...");
                        }
                    }

                    // Commit barrier: allow the compositor to commit at least one opaque lock
                    // frame before releasing the systemd-logind delay sleep inhibitor.
                    if self.inhibit_opt.is_some() {
                        commands.push(cosmic::task::future(async {
                            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                            Message::ReleaseInhibit
                        }));
                    }
                    return Task::batch(commands);
                }
                SessionLockEvent::Unlocked => {
                    tracing::info!("session unlocked");
                    self.state = State::Unlocked;

                    let mut commands = Vec::new();
                    for (_output, surface_id) in self.common.surface_ids.iter() {
                        self.common.surface_names.remove(surface_id);
                        self.common.window_size.remove(surface_id);
                        commands.push(destroy_lock_surface(*surface_id));
                    }
                    if cfg!(feature = "logind") && self.flags.logind_available {
                        // Um `Lock` chegou durante a saída: bloqueia de novo já (PAM recomeça
                        // pelo caminho normal do `Locked`).
                        if std::mem::take(&mut self.relock_pending) {
                            commands.push(self.start_lock());
                        }
                        return Task::batch(commands);
                        // When logind is available, stick around for more lock signals
                    } else {
                        // When logind is not available, exit immediately after unlocking
                        //TODO: cleaner method to exit?
                        process::exit(0);
                    }
                }
                //TODO: handle finished signal
                _ => {}
            },
            Message::Channel(value_tx) => {
                self.value_tx_opt = Some(value_tx);
            }
            Message::BackgroundState(bg_state) => {
                self.flags.user_data.bg_state = bg_state;
                self.flags.user_data.load_wallpapers_as_user();
                self.common.surface_images.clear();
                self.common.update_wallpapers(&self.flags.user_data);
            }
            Message::DropdownToggle(dropdown) => {
                if self.dropdown_opt == Some(dropdown) {
                    self.dropdown_opt = None;
                    return self.common.dropdown_blur_rects(false);
                } else {
                    self.dropdown_opt = Some(dropdown);
                    if dropdown == Dropdown::Power {
                        self.motion.open_menu(now);
                    }
                    return self.common.dropdown_blur_rects(true);
                }
            }
            Message::Inhibit(inhibit) => {
                if keep_inhibit(self.state.kind(), self.motion.exiting()) {
                    // Solto no próximo `SessionLockEvent::Locked` (suspende só bloqueado).
                    self.inhibit_opt = Some(inhibit);
                } else {
                    tracing::info!("no need to inhibit sleep when already locked");
                }
            }
            Message::KeyboardLayout(layout_i) => {
                if let Some(keyboard_layout) = &self.common.keyboard_layout {
                    keyboard_layout.set_group(layout_i as u32);
                }
                self.common.current_keyboard_layout = layout_i;
                if self.dropdown_opt == Some(Dropdown::Keyboard) {
                    self.dropdown_opt = None;
                    return self.common.dropdown_blur_rects(false);
                }
            }
            Message::Submit(value) => {
                if value.is_empty() {
                    return Task::none();
                }
                self.common.error_opt = None;
                self.authenticating = true;
                match self.value_tx_opt.take() {
                    Some(value_tx) => {
                        return cosmic::task::future(async move {
                            value_tx.send(value).await.unwrap();
                            Message::Channel(value_tx)
                        });
                    }
                    None => tracing::warn!("tried to submit when value_tx_opt not set"),
                }
            }
            Message::Suspend => {
                // Passa por power::perform: em modo aninhado só registra no log.
                return Self::power_task(PowerAction::Suspend, self.flags.nested);
            }
            Message::Power(action) => {
                self.dropdown_opt = None;
                return Task::batch([
                    self.common.dropdown_blur_rects(false),
                    Self::power_task(action, self.flags.nested),
                ]);
            }
            Message::TimeAppletConfig(config) => {
                self.flags.user_data.time_applet_config = config;
            }
            Message::Error(error) => {
                self.common.error_opt = Some(error);
                // Balanço só para senha enviada (erro de energia chega sem `authenticating`).
                self.motion.on_auth_error(self.authenticating, now);
                self.authenticating = false;
            }
            Message::Lock => match lock_decision(self.state.kind(), self.motion.exiting()) {
                LockStep::Lock => return self.start_lock(),
                LockStep::Defer => {
                    tracing::info!("lock requested during unlock; will relock");
                    self.relock_pending = true;
                }
                LockStep::Ignore => {
                    tracing::info!("session already locking or locked");
                }
            },
            Message::Unlock => {
                match unlock_step(matches!(self.state, State::Locked { .. }), &mut self.motion, now) {
                    UnlockStep::Fade => {
                        if crate::perf::enabled() {
                            crate::perf::line(&format!("exit: start t={}", crate::perf::now_ms()));
                        }
                        return cosmic::task::future(async {
                            tokio::time::sleep(motion::EXIT_FADE).await;
                            Message::UnlockReady
                        });
                    }
                    // Já saindo: um único unlock().
                    UnlockStep::Ignore => {}
                    // Locking/Unlocking/Unlocked: mesmos logs de antes.
                    UnlockStep::Finish => return self.finish_unlock(),
                }
            }
            Message::UnlockReady => {
                if crate::perf::enabled() {
                    crate::perf::line(&format!("exit: fire t={}", crate::perf::now_ms()));
                }
                return self.finish_unlock();
            }
            Message::Frame(_) => {
                crate::perf::count(crate::perf::Counter::Frame);
                if crate::perf::enabled() {
                    crate::perf::line(&self.motion.debug_line(crate::perf::now_ms()));
                }
            }
            Message::Hover(surface_id, target, on) => {
                self.motion.hover(surface_id, target, on, now);
            }
            Message::CursorLeft(surface_id) => self.motion.cursor_left(surface_id, now),
            Message::FieldFocus(focused) => self.motion.set_focus(focused, now),
            Message::DemoTick => {
                let event = self
                    .flags
                    .demo
                    .and_then(|mode| motion::demo_event(self.demo_slot, mode));
                self.demo_slot += 1;
                if let Some(open) = event.and_then(|e| apply_demo(&mut self.motion, e, now)) {
                    self.dropdown_opt = open.then_some(Dropdown::Power);
                }
            }
            Message::ReleaseInhibit => {
                if matches!(self.state, State::Locked { .. }) && !self.motion.exiting() {
                    tracing::info!("releasing sleep inhibit after lock frame commit barrier");
                    self.inhibit_opt = None;
                }
            }
            Message::Surface(a) => {
                return cosmic::task::message(cosmic::Action::Surface(a));
            }
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::time::Duration;

    #[test]
    fn pam_service_defaults_to_compiled_default() {
        assert_eq!(pam_service_from(None), DEFAULT_PAM_SERVICE);
    }
    #[test]
    fn pam_service_from_env() {
        assert_eq!(pam_service_from(Some(OsStr::new("login"))), "login");
    }
    #[test]
    fn pam_service_empty_falls_back() {
        assert_eq!(pam_service_from(Some(OsStr::new(""))), DEFAULT_PAM_SERVICE);
    }
    #[test]
    fn pam_service_keeps_existing_default() {
        let got = resolve_pam_service(None, |n| n == DEFAULT_PAM_SERVICE || n == "login");
        assert_eq!(got, DEFAULT_PAM_SERVICE);
    }
    #[test]
    fn pam_service_falls_back_to_login_when_default_missing() {
        // Arch/CachyOS: não há /etc/pam.d/cosmic-greeter; cair em `other` nega toda senha.
        assert_eq!(resolve_pam_service(None, |n| n == "login"), "login");
    }
    #[test]
    fn pam_service_env_missing_falls_back_to_login() {
        let got = resolve_pam_service(Some(OsStr::new("nao-existe")), |n| n == "login");
        assert_eq!(got, "login");
    }
    #[test]
    fn pam_service_env_existing_wins() {
        let got = resolve_pam_service(Some(OsStr::new("system-local-login")), |_| true);
        assert_eq!(got, "system-local-login");
    }
    #[test]
    fn pam_service_nothing_exists_keeps_candidate() {
        assert_eq!(resolve_pam_service(None, |_| false), DEFAULT_PAM_SERVICE);
    }
    #[test]
    fn pam_service_rejects_paths() {
        assert_eq!(pam_service_from(Some(OsStr::new("../shadow"))), DEFAULT_PAM_SERVICE);
        assert_eq!(pam_service_from(Some(OsStr::new("a/b"))), DEFAULT_PAM_SERVICE);
        assert_eq!(pam_service_from(Some(OsStr::new(".login"))), DEFAULT_PAM_SERVICE);
    }

    fn prompt(value: Option<&str>) -> Option<(String, bool, Option<String>)> {
        Some(("Password: ".into(), true, value.map(String::from)))
    }
    #[test]
    fn status_choice_matches_menu_rules() {
        use login_ui::StatusLine;
        // Campo visível: Caps Lock conta; autenticando vence tudo.
        assert_eq!(status_for(&prompt(Some("")), false, true, None), StatusLine::CapsLock);
        assert_eq!(status_for(&prompt(Some("")), true, true, Some("x")), StatusLine::Authenticating);
        assert_eq!(status_for(&prompt(Some("")), false, false, Some("x")), StatusLine::Error("x"));
        // Prompt sem valor = info; Caps Lock não conta sem campo.
        assert_eq!(status_for(&prompt(None), false, true, None), StatusLine::Info("Password: "));
        assert_eq!(status_for(&None, false, true, None), StatusLine::Empty);
    }
    #[test]
    fn unlock_fades_once_while_locked() {
        let t0 = Instant::now();
        let mut m = Motion::<u32>::new(t0);
        assert_eq!(unlock_step(true, &mut m, t0), UnlockStep::Fade);
        // PAM e logind quase juntos: o segundo não gera outro unlock().
        assert_eq!(unlock_step(true, &mut m, t0 + Duration::from_millis(20)), UnlockStep::Ignore);
        assert!(m.exiting());
    }
    #[test]
    fn unlock_outside_locked_goes_straight_through() {
        let t0 = Instant::now();
        let mut m = Motion::<u32>::new(t0);
        assert_eq!(unlock_step(false, &mut m, t0), UnlockStep::Finish);
        assert!(!m.exiting());
        // Rebloqueio: cancel_exit libera um novo fade.
        assert_eq!(unlock_step(true, &mut m, t0), UnlockStep::Fade);
        m.cancel_exit();
        assert_eq!(unlock_step(true, &mut m, t0), UnlockStep::Fade);
    }
    #[test]
    fn lock_during_exit_or_unlocking_is_deferred() {
        use StateKind::*;
        assert_eq!(lock_decision(Unlocked, false), LockStep::Lock);
        assert_eq!(lock_decision(Locked, true), LockStep::Defer);
        assert_eq!(lock_decision(Unlocking, false), LockStep::Defer);
    }
    #[test]
    fn lock_while_locked_or_locking_is_ignored() {
        use StateKind::*;
        assert_eq!(lock_decision(Locked, false), LockStep::Ignore);
        assert_eq!(lock_decision(Locking, false), LockStep::Ignore);
        assert_eq!(lock_decision(Locking, true), LockStep::Ignore);
    }
    #[test]
    fn inhibit_kept_unless_firmly_locked() {
        use StateKind::*;
        assert!(!keep_inhibit(Locked, false));
        assert!(keep_inhibit(Locked, true));
        assert!(keep_inhibit(Unlocking, false));
        assert!(keep_inhibit(Unlocked, false));
        assert!(keep_inhibit(Locking, false));
    }
    #[test]
    fn field_rejects_input_while_authenticating_or_exiting() {
        assert!(accepts_input(false, false));
        assert!(!accepts_input(true, false));
        assert!(!accepts_input(false, true));
    }

    #[test]
    fn demo_menu_steps_report_menu_state_and_fade_in_on_open() {
        let t0 = Instant::now();
        let mut m = Motion::<u32>::new(t0);
        assert_eq!(apply_demo(&mut m, motion::DemoEvent::Menu(true), t0), Some(true));
        assert!(m.is_animating(), "abrir o menu anima o fade");
        assert_eq!(apply_demo(&mut m, motion::DemoEvent::Menu(false), t0), Some(false));
    }

    #[test]
    fn wallpaper_is_reloaded_only_when_source_or_handle_changes() {
        let a = (PathBuf::from("/w/a.png"), Some(UNIX_EPOCH));
        let a_touched = (PathBuf::from("/w/a.png"), Some(UNIX_EPOCH + Duration::from_secs(1)));
        let b = (PathBuf::from("/w/b.png"), Some(UNIX_EPOCH));
        // Mesma origem com handle carregado: mantém (o desfoque pronto continua valendo).
        assert!(wallpaper_unchanged(Some(&a), Some(&a), true));
        // Arquivo trocado ou editado: recarrega.
        assert!(!wallpaper_unchanged(Some(&a), Some(&b), true));
        assert!(!wallpaper_unchanged(Some(&a), Some(&a_touched), true));
        // Origem sem handle (leitura falhou antes): tenta de novo.
        assert!(!wallpaper_unchanged(Some(&a), Some(&a), false));
        // Sem papel de parede custom nos dois lados: nada a fazer; handle sobrando: limpa.
        assert!(wallpaper_unchanged(None, None, false));
        assert!(!wallpaper_unchanged(None, None, true));
        assert!(!wallpaper_unchanged(None, Some(&a), false));
    }

    #[test]
    fn rotated_wallpaper_is_accepted_only_for_the_current_slot() {
        assert!(wallpaper_slice_is_current(Some(7), 7));
        // Fatia velha (a rotação já andou, ou a config deixou de rotacionar): descartada.
        assert!(!wallpaper_slice_is_current(Some(8), 7));
        assert!(!wallpaper_slice_is_current(None, 7));
    }

    #[test]
    fn rotated_wallpaper_load_skips_unchanged_and_loads_new() {
        let dir = std::env::temp_dir().join(format!("aenes-locker-rot-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("criar diretório temporário");
        let a = dir.join("a.png");
        fs::write(&a, b"conteudo-a").expect("gravar a");

        // Nada carregado ainda: carrega o escolhido.
        let loaded = load_rotated_wallpaper(Some(a.clone()), None, false).expect("carregar");
        assert_eq!(loaded.source.as_ref().map(|(p, _)| p.as_path()), Some(a.as_path()));
        assert!(loaded.handle.is_some());
        // Mesmo arquivo (mesmo mtime) já carregado: nada a trocar, sem ler de novo.
        assert!(load_rotated_wallpaper(Some(a.clone()), loaded.source.as_ref(), true).is_none());
        // Escolha vazia com papel de parede custom carregado: limpa.
        let cleared = load_rotated_wallpaper(None, loaded.source.as_ref(), true).expect("limpar");
        assert!(cleared.source.is_none() && cleared.handle.is_none());
        // Sem nada carregado nem escolhido: nada a fazer.
        assert!(load_rotated_wallpaper(None, None, false).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn demo_non_menu_steps_do_not_touch_the_menu() {
        let t0 = Instant::now();
        let mut m = Motion::<u32>::new(t0);
        for ev in [
            motion::DemoEvent::RestartEntry,
            motion::DemoEvent::Shake,
            motion::DemoEvent::Hover(true),
        ] {
            assert_eq!(apply_demo(&mut m, ev, t0), None);
        }
        assert!(m.is_animating());
    }
}
