// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

mod ipc;

use crate::wayland::{self, WaylandUpdate};
use cctk::sctk::reexports::calloop;
use color_eyre::eyre::WrapErr;
use cosmic::app::{Core, Settings, Task};
use cosmic::cctk::wayland_protocols::xdg::shell::client::xdg_positioner::Gravity;
use cosmic::cosmic_config::{self, ConfigSet};
use cosmic::cosmic_theme::{self, CosmicPalette};
use cosmic::desktop::fde::{DesktopEntry, get_languages_from_env};
use cosmic::iced::core::text::{Ellipsize, EllipsizeHeightLimit};
use cosmic::iced::event::listen_with;
use cosmic::iced::event::wayland::OutputEvent;
use cosmic::iced::futures::SinkExt;
use cosmic::iced::platform_specific::runtime::wayland::layer_surface::{
    IcedMargin, IcedOutput, SctkLayerSurfaceSettings,
};
use cosmic::iced::platform_specific::shell::wayland::commands::layer_surface::{
    Anchor, KeyboardInteractivity, Layer, destroy_layer_surface,
};
use cosmic::iced::platform_specific::shell::wayland::commands::subsurface::reposition_subsurface;
use cosmic::iced::runtime::core::window::Id as SurfaceId;
use cosmic::iced::runtime::platform_specific::wayland::subsurface::SctkSubsurfaceSettings;
use cosmic::iced::{
    self, Alignment, Background, Border, Length, Point, Size, Subscription, window,
};
use cosmic::surface::action::{LiveSettings, app_layer_shell};
use cosmic::widget::{id_container, text};
use cosmic::{Element, executor, surface, theme, widget};
use cosmic_greeter_config::Config as CosmicGreeterConfig;
use cosmic_greeter_daemon::{UserData, UserFilter};
use cosmic_randr_shell::{KdlParseWithError, List};
use cosmic_settings_a11y_manager_subscription::{AccessibilityEvent, AccessibilityRequest};
use greetd_ipc::Request;
use kdl::KdlDocument;
use std::collections::{HashMap, hash_map};
use std::error::Error;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{fs, io, process, thread};
use tokio::process::Child;
use tokio::time;
use tracing::metadata::LevelFilter;
use tracing::warn;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};
use wayland_client::Proxy;
use wayland_client::protocol::wl_output::WlOutput;
use zbus::{Connection, proxy};

use crate::backdrop::{self, BackdropSet};
use crate::common::{self, Common, DEFAULT_MENU_ITEM_HEIGHT, MAX_WIDTH};
use crate::fl;
use crate::login_ui;
use crate::selection;
use crate::styles;
use crate::motion::{self, HoverTarget, Motion, Part};
use crate::style_config::{self, StyleConfig};

static USERNAME_ID: LazyLock<iced::id::Id> = LazyLock::new(|| iced::id::Id::new("username-id"));

/// Papel de parede com que o login abre: nos modos rotativos escolhe pelo horário (o greeter não tem
/// contador de bloqueios), para a primeira pintura já mostrar a imagem da vez.
fn initial_wallpaper(cfg: &StyleConfig, now: u64) -> Option<PathBuf> {
    cfg.pick_wallpaper_login_at(now)
}

/// Segundos desde a época Unix (0 se o relógio estiver antes dela).
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[proxy(
    interface = "com.system76.CosmicGreeter",
    default_service = "com.system76.CosmicGreeter",
    default_path = "/com/system76/CosmicGreeter"
)]
trait Greeter {
    async fn get_user_data(&self) -> Result<String, zbus::Error>;
}

async fn user_data_dbus() -> Result<Vec<UserData>, Box<dyn Error>> {
    let connection = Connection::system().await?;

    // `dbus_proxy` macro creates `MyGreaterProxy` based on `Notifications` trait.
    let proxy = GreeterProxy::new(&connection).await?;
    let reply = proxy.get_user_data().await?;

    let user_datas: Vec<UserData> = ron::from_str(&reply)?;
    Ok(user_datas)
}

/// Só no modo aninhado (harness) o fallback carrega a config do próprio usuário do processo:
/// sem o daemon (D-Bus negado) o `UserData` viria sem a config (ex.: formato 24 h).
fn should_load_config(nested: bool, entry_uid: u32, my_uid: u32) -> bool {
    nested && entry_uid == my_uid
}

fn current_uid() -> Option<u32> {
    use std::os::unix::fs::MetadataExt;
    fs::metadata("/proc/self").ok().map(|m| m.uid())
}

async fn user_data_fallback() -> Vec<UserData> {
    let user_filter = UserFilter::new().await;

    // The pwd::Passwd method is unsafe (but not labelled as such) due to using global state (libc pwent functions).
    /* unsafe */
    {
        pwd::Passwd::iter()
            .filter(|user| user_filter.filter(user))
            .map(|user| {
                let mut data = UserData::from(user);
                if let Some(my_uid) = current_uid()
                    && should_load_config(crate::power::nested_from_env(), data.uid, my_uid)
                {
                    data.load_config_as_user();
                }
                data
            })
            .collect()
    }
}

#[cfg(test)]
mod fallback_tests {
    use super::should_load_config;

    #[test]
    fn loads_config_only_when_nested_and_own_uid() {
        assert!(should_load_config(true, 1000, 1000));
        assert!(!should_load_config(true, 1001, 1000));
        assert!(!should_load_config(false, 1000, 1000));
    }
}

/// Linha de status do greeter: socket fechado = info do socket; aberto, campo visível (nome ou
/// senha) = Caps Lock conta; prompt sem valor = info. `menu` e `current_status` usam esta escolha.
fn greeter_status<'a>(
    socket_state: &SocketState,
    socket_error_text: &'a str,
    needs_username: bool,
    prompt_opt: &'a Option<(String, bool, Option<String>)>,
    authenticating: bool,
    caps_lock: bool,
    error_opt: Option<&'a str>,
) -> login_ui::StatusLine<'a> {
    let (field, info) = match socket_state {
        SocketState::Pending => (false, Some("Opening GREETD_SOCK")),
        SocketState::NotSet => (false, Some("GREETD_SOCK variable not set")),
        SocketState::Error(_) => (false, Some(socket_error_text)),
        SocketState::Open if needs_username => (true, None),
        SocketState::Open => match prompt_opt {
            Some((_, _, Some(_))) => (true, None),
            Some((prompt, _, None)) => (false, Some(prompt.as_str())),
            None => (false, None),
        },
    };
    login_ui::status_line(authenticating, caps_lock && field, error_opt, info)
}

/// `Login` (greetd autenticou): começa o fade de saída e devolve o número desta saída, que o
/// timer leva até `StartSessionReady`. `None` se já está saindo (ipc.rs pode mandar `Login` 2×).
fn begin_exit<K: Copy + PartialEq>(motion: &mut Motion<K>, seq: &mut u64, now: Instant) -> Option<u64> {
    if motion.start_exit(now) {
        *seq += 1;
        Some(*seq)
    } else {
        None
    }
}

/// Fim do timer: `StartSession` só se esta saída ainda vale (nenhum erro a cancelou e nenhuma
/// saída mais nova a substituiu).
fn exit_fires<K: Copy + PartialEq>(motion: &Motion<K>, seq: u64, fired: u64) -> bool {
    motion.exiting() && seq == fired
}

/// Erro do greetd/PAM: a tela volta na hora (cancela o fade de saída) e o campo balança se
/// havia senha enviada.
fn restore_on_error<K: Copy + PartialEq>(motion: &mut Motion<K>, was_authenticating: bool, now: Instant) {
    motion.cancel_exit();
    motion.on_auth_error(was_authenticating, now);
}

/// Mensagens de entrar/sair do hover de um alvo.
fn hovers(surface_id: SurfaceId, target: HoverTarget) -> login_ui::Hovers<Message> {
    login_ui::Hovers {
        enter: Message::Hover(surface_id, target, true),
        exit: Message::Hover(surface_id, target, false),
    }
}

pub fn main() -> Result<(), Box<dyn Error>> {
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
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut user_datas = match runtime.block_on(user_data_dbus()) {
        Ok(ok) => ok,
        Err(err) => {
            tracing::error!("failed to load user data from daemon: {}", err);
            runtime.block_on(user_data_fallback())
        }
    };

    // Sort user data by uid
    user_datas.sort_by(|a, b| a.uid.cmp(&b.uid));
    let (mut greeter_config, greeter_config_handler) = CosmicGreeterConfig::load();
    // Filter out users that were removed from the system since the last time we loaded config
    greeter_config.users.retain(|uid, _| {
        user_datas
            .binary_search_by(|probe| probe.uid.cmp(&uid.get()))
            .is_ok()
    });

    enum SessionType {
        X11,
        Wayland,
    }

    let session_dirs = xdg::BaseDirectories::with_prefix("wayland-sessions")
        .get_data_dirs()
        .into_iter()
        .map(|dir| (dir, SessionType::Wayland))
        .chain(
            xdg::BaseDirectories::with_prefix("xsessions")
                .get_data_dirs()
                .into_iter()
                .map(|dir| (dir, SessionType::X11)),
        );

    let sessions = {
        let mut sessions = HashMap::new();
        let locales = get_languages_from_env();
        for (session_dir, session_type) in session_dirs {
            let read_dir = match fs::read_dir(&session_dir) {
                Ok(ok) => ok,
                Err(err) => {
                    tracing::warn!(
                        "failed to read session directory {:?}: {:?}",
                        session_dir,
                        err
                    );
                    continue;
                }
            };

            for dir_entry_res in read_dir {
                let dir_entry = match dir_entry_res {
                    Ok(ok) => ok,
                    Err(err) => {
                        tracing::warn!(
                            "failed to read session directory {:?} entry: {:?}",
                            session_dir,
                            err
                        );
                        continue;
                    }
                };

                let entry = match DesktopEntry::from_path(dir_entry.path(), Some(&locales)) {
                    Ok(ok) => ok,
                    Err(err) => {
                        tracing::warn!(
                            "failed to read session file {:?}: {:?}",
                            dir_entry.path(),
                            err
                        );
                        continue;
                    }
                };

                let name = match entry.name(&locales) {
                    Some(some) => some,
                    None => {
                        tracing::warn!(
                            "failed to read session file {:?}: no Desktop Entry/Name attribute",
                            dir_entry.path()
                        );
                        continue;
                    }
                };

                let exec = match entry.exec() {
                    Some(some) => some,
                    None => {
                        tracing::warn!(
                            "failed to read session file {:?}: no Desktop Entry/Exec attribute",
                            dir_entry.path()
                        );
                        continue;
                    }
                };

                let mut command = Vec::new();
                let mut env = Vec::new();
                match session_type {
                    SessionType::X11 => {
                        //TODO: xinit may be better, but more complicated to set up
                        command.push("startx".to_string());
                        env.push("XDG_SESSION_TYPE=x11".to_string());
                    }
                    SessionType::Wayland => {
                        env.push("XDG_SESSION_TYPE=wayland".to_string());
                    }
                };

                if let Some(desktop_names) = entry
                    .groups
                    .desktop_entry()
                    .and_then(|g| g.entry("DesktopNames"))
                {
                    env.push(format!("XDG_CURRENT_DESKTOP={desktop_names}"));
                    if let Some(name) = desktop_names.split(':').next() {
                        env.push(format!("XDG_SESSION_DESKTOP={name}"));
                    }
                }

                // Session exec may contain environmental variables
                command.push("/usr/bin/env".to_string());

                // To ensure the env is set correctly, we also set it in the session command
                for arg in env.iter() {
                    command.push(arg.clone());
                }

                match shlex::split(exec) {
                    Some(args) => {
                        for arg in args {
                            command.push(arg)
                        }
                    }
                    None => {
                        tracing::warn!(
                            "failed to parse session file {:?} Exec field {:?}",
                            dir_entry.path(),
                            exec
                        );
                        continue;
                    }
                };

                tracing::info!("session {} using command {:?} env {:?}", name, command, env);
                if let Some(some) = sessions.insert(name.to_string(), (command, env)) {
                    tracing::warn!("session {} overwrote old command {:?}", name, some);
                }
            }
        }
        sessions
    };

    match process::Command::new("cosmic-osk")
        .arg("overlay")
        .env("RUST_LOG", "cosmic_osk=info")
        .spawn()
    {
        Ok(mut child) => {
            thread::spawn(move || match child.wait() {
                Ok(status) => tracing::warn!("cosmic-osk exited: {}", status),
                Err(err) => tracing::error!("failed to wait on cosmic-osk: {}", err),
            });
        }
        Err(err) => {
            tracing::error!("failed to spawn cosmic-osk: {}", err);
        }
    }

    let flags = Flags {
        user_icons: user_datas
            .iter_mut()
            .map(|d| d.icon_opt.take().map(widget::image::Handle::from_bytes))
            .collect(),
        user_datas,
        sessions,
        greeter_config,
        greeter_config_handler,
        #[cfg(feature = "logind")]
        logind_available: crate::logind::is_available(),
        #[cfg(not(feature = "logind"))]
        logind_available: false,
        nested: crate::power::nested_from_env(),
        demo: motion::demo_mode_from(
            std::env::var_os(motion::DEMO_ENV).as_deref(),
            crate::power::nested_from_env(),
        ),
    };

    let settings = Settings::default().no_main_window(true);

    crate::perf::init_from_env();
    cosmic::app::run::<App>(settings, flags)?;

    Ok(())
}

#[derive(Clone)]
pub struct Flags {
    user_datas: Vec<UserData>,
    user_icons: Vec<Option<widget::image::Handle>>,
    sessions: HashMap<String, (Vec<String>, Vec<String>)>,
    greeter_config: CosmicGreeterConfig,
    greeter_config_handler: Option<cosmic_config::Config>,
    logind_available: bool,
    nested: bool,
    /// Demo de movimento (só aninhado): dispara entrada/balanço/menu/hover sem entrada do usuário.
    demo: Option<motion::DemoMode>,
}

#[derive(Clone, Debug)]
pub enum SocketState {
    /// Opening GREETD_SOCK
    Pending,
    /// GREETD_SOCK is open
    Open,
    /// No GREETD_SOCK variable set
    NotSet,
    /// Failed to open GREETD_SOCK
    Error(Arc<io::Error>),
}

#[derive(Clone, Copy, Debug)]
pub enum DialogPage {
    Restart(Instant),
    Shutdown(Instant),
}

impl DialogPage {
    fn remaining(instant: Instant) -> Option<Duration> {
        let elapsed = instant.elapsed();
        let timeout = Duration::new(60, 0);
        if elapsed < timeout {
            Some(timeout - elapsed)
        } else {
            None
        }
    }
}

///TODO: this is custom code that should be better handled by libcosmic
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dropdown {
    Accessibility,
    Keyboard,
    User,
    Session,
    Power,
}

struct NameIndexPair {
    /// Selected username
    username: String,
    /// Index of the [`UserData`] for the selected username
    data_idx: Option<usize>,
}

/// Messages that are used specifically by our [`App`].
#[derive(Clone, Debug)]
pub enum Message {
    Common(common::Message),
    OutputEvent(OutputEvent, WlOutput),
    Auth(Option<String>),
    /// Non-fatal error message from PAM (`PAM_ERROR_MSG`), reported by greetd as an auth
    /// message of type `Error`. Unlike [`Message::Error`] the conversation stays alive.
    AuthError(String),
    ConfigUpdateUser,
    DialogCancel,
    DialogConfirm,
    DropdownToggle(Dropdown),
    Error(String),
    Exit,
    // Sets channel used to communicate with the greetd IPC subscription.
    GreetdChannel(tokio::sync::mpsc::Sender<Request>),
    /// Refreshes display outputs.
    RandrUpdate {
        /// Available outputs from cosmic-randr.
        randr: Arc<Result<List, cosmic_randr_shell::Error>>,
    },
    Heartbeat,
    KeyboardLayout(usize),
    Login,
    Reconnect,
    Reload(cosmic::Theme),
    RepositionMenu(window::Id, Size),
    Restart,
    Session(String),
    Shutdown,
    Socket(SocketState),
    Surface(surface::Action<Message>),
    Suspend,
    Power(crate::power::PowerAction),
    Username(String),
    EnterUser(bool, String),
    ScreenReader(bool),
    Magnifier(bool),
    HighContrast(bool),
    InvertColors(bool),
    WaylandUpdate(WaylandUpdate),
    /// Tick de animação (só enquanto `motion.is_animating()`).
    Frame(Instant),
    /// Fim do fade de saída número N: agora sim `StartSession` (se a saída ainda vale).
    StartSessionReady(u64),
    /// Hover de um alvo (campo, seta, ⏻) numa superfície.
    Hover(SurfaceId, motion::HoverTarget, bool),
    /// O cursor saiu da superfície: apaga os hovers dela.
    CursorLeft(SurfaceId),
    /// Foco do campo (senha ou nome): clique / Tab / Esc.
    FieldFocus(bool),
    /// Passo do roteiro de demo.
    DemoTick,
    /// Desfoque e análise do fundo da superfície principal, prontos (ou o erro do job).
    Backdrop(SurfaceId, Result<backdrop::Prepared, String>),
    /// Papel de parede da rotação por horário, lido fora da thread de UI (caminho + imagem).
    Wallpaper(u64, Option<(PathBuf, widget::image::Handle)>),
}

impl From<common::Message> for Message {
    fn from(message: common::Message) -> Self {
        Self::Common(message)
    }
}

/// The [`App`] stores application-specific state.
pub struct App {
    common: Common<Message>,
    flags: Flags,
    greetd_sender: Option<tokio::sync::mpsc::Sender<greetd_ipc::Request>>,
    socket_state: SocketState,
    /// Texto de `SocketState::Error`, pronto para a linha de status.
    socket_error_text: String,
    usernames: Vec<(String, String)>,
    selected_username: NameIndexPair,
    session_names: Vec<String>,
    selected_session: String,
    dialog_page_opt: Option<DialogPage>,
    dropdown_opt: Option<Dropdown>,
    heartbeat_handle: Option<cosmic::iced::task::Handle>,
    entering_name: bool,
    theme_builder: cosmic_theme::ThemeBuilder,
    surface_id_pairs: Vec<(window::Id, window::Id)>,

    randr_list: Option<cosmic_randr_shell::List>,

    accessibility: Accessibility,
    authenticating: bool,
    motion: Motion<SurfaceId>,
    /// Número da saída em curso (o timer de `StartSessionReady` leva o seu).
    exit_seq: u64,
    /// Próximo passo do roteiro de demo.
    demo_slot: u64,
    /// Visual style & wallpaper config (`~/.config/cosmic-taskbar/greeter.toml`).
    style_config: StyleConfig,
    /// Loaded custom/rotating wallpaper handle, if configured.
    custom_wallpaper: Option<widget::image::Handle>,
    /// Caminho do `custom_wallpaper` (a rotação não recarrega o mesmo arquivo).
    custom_wallpaper_path: Option<PathBuf>,
    /// Desfoque e análise já calculados (ou pedidos) por superfície principal.
    backdrops: BackdropSet<SurfaceId>,
    /// Última fatia de rotação já aplicada (`rotate_interval`).
    rotation_slot: Option<u64>,
}

#[derive(Default)]
struct Accessibility {
    pub wayland_sender: Option<calloop::channel::Sender<AccessibilityRequest>>,
    pub wayland_protocol_version: Option<u32>,

    pub state: cosmic_settings_daemon_config::greeter::GreeterAccessibilityState,
    pub helper: Option<cosmic::cosmic_config::Config>,

    pub screen_reader: Option<Child>,
    pub magnifier: bool,
    pub high_contrast: bool,
    pub invert_colors: bool,
}

impl App {
    /// Applies a display configuration via `cosmic-randr`.
    fn exec_randr(&self, user_config: cosmic_randr_shell::List) -> Task<Message> {
        let mut task = tokio::process::Command::new("cosmic-randr");
        task.arg("kdl");

        cosmic::task::future::<(), ()>(async move {
            task.stdin(Stdio::piped());
            let Ok(mut p) = task.spawn() else {
                return;
            };

            let kdl_doc = kdl::KdlDocument::from(user_config).to_string();
            use tokio::io::AsyncWriteExt;

            if let Some(mut stdin) = p.stdin.take() {
                if let Err(err) = stdin.write_all(kdl_doc.as_bytes()).await {
                    tracing::error!("Failed to write KDL to stdin: {err:?}");
                }
                if let Err(err) = stdin.flush().await {
                    tracing::error!("Failed to flush stdin: {err:?}");
                }
            }
            tracing::debug!("executing {task:?}");
            let status = p.wait().await;
            if let Err(err) = status {
                tracing::error!("Randr error: {err:?}");
            }
        })
        .discard()
    }

    /// Imagem de fundo da superfície principal `id`: o papel de parede configurado ou o do usuário.
    fn surface_image(&self, id: SurfaceId) -> Option<&widget::image::Handle> {
        self.custom_wallpaper
            .as_ref()
            .or_else(|| self.common.surface_images.get(&id))
    }

    /// Identidade do que a superfície `id` mostra; `None` sem imagem ou sem tamanho conhecido.
    fn surface_backdrop_tag(&self, id: SurfaceId) -> Option<backdrop::Tag> {
        backdrop::surface_tag(
            self.surface_image(id)?,
            self.style_config.blur_radius,
            self.common.output_size(id),
        )
    }

    /// Resultado pronto para o que `id` mostra agora (resultado de outra imagem não vale).
    fn ready_backdrop(&self, id: SurfaceId) -> Option<&backdrop::Prepared> {
        let tag = self.surface_backdrop_tag(id)?;
        self.backdrops.get(id).filter(|prepared| prepared.tag == tag)
    }

    /// Dispara o desfoque/análise das superfícies cuja imagem ou tamanho mudou. Barato (um tag e
    /// uma consulta por superfície); todo o trabalho pesado roda dentro do job.
    fn spawn_backdrops(&mut self) -> Task<Message> {
        let ids: Vec<SurfaceId> = self.common.surface_ids.values().copied().collect();
        let mut tasks = Vec::new();
        for id in ids {
            let Some(tag) = self.surface_backdrop_tag(id) else {
                continue;
            };
            if !self.backdrops.wants(id, tag) {
                continue;
            }
            // `Path` não tem bytes em memória (ler aqui seria I/O na UI): sem eles não há job.
            let Some(bytes) = self.surface_image(id).and_then(backdrop::handle_bytes) else {
                continue;
            };
            self.backdrops.mark_pending(id, tag);
            tasks.push(cosmic::task::future(async move {
                // `cache_dir` toca o disco (stat/mkdir): também fora da thread de UI.
                let cache = tokio::task::spawn_blocking(|| backdrop::cache_dir(true))
                    .await
                    .unwrap_or(None);
                Message::Backdrop(id, backdrop::prepare_async(bytes, tag, cache).await)
            }));
        }
        Task::batch(tasks)
    }

    /// Rotação por horário: quando a fatia muda, lê o próximo papel de parede fora da UI. Não mexe
    /// em `prompt_opt`, foco nem no texto digitado.
    fn rotate_wallpaper(&mut self) -> Task<Message> {
        let now = unix_now();
        let slot = self.style_config.rotation_slot(now);
        if !style_config::rotation_due(self.rotation_slot, slot) {
            return Task::none();
        }
        let Some(slot_value) = slot else {
            return Task::none();
        };
        self.rotation_slot = slot;
        let cfg = self.style_config.clone();
        cosmic::task::future(async move {
            let loaded = tokio::task::spawn_blocking(move || {
                let path = initial_wallpaper(&cfg, now)?;
                let handle = style_config::load_wallpaper_handle(&path)?;
                Some((path, handle))
            })
            .await
            .unwrap_or(None);
            Message::Wallpaper(slot_value, loaded)
        })
    }

    /// Menu de sessão (só o greeter): a lista vem de `session_names`, a marcada é a escolhida.
    fn session_menu(&self) -> Option<styles::ChoiceMenu<Message>> {
        if self.session_names.is_empty() {
            return None;
        }
        Some(styles::ChoiceMenu {
            open: self.dropdown_opt == Some(Dropdown::Session),
            on_toggle: Message::DropdownToggle(Dropdown::Session),
            label: fl!("sign-in-options"),
            items: self
                .session_names
                .iter()
                .map(|name| {
                    (
                        name.clone(),
                        name == &self.selected_session,
                        Message::Session(name.clone()),
                    )
                })
                .collect(),
        })
    }

    /// Menu de layout do teclado, só com 2+ layouts ativos.
    fn keyboard_menu(&self) -> Option<styles::ChoiceMenu<Message>> {
        if !style_config::wants_keyboard_menu(self.common.active_layouts.len()) {
            return None;
        }
        Some(styles::ChoiceMenu {
            open: self.dropdown_opt == Some(Dropdown::Keyboard),
            on_toggle: Message::DropdownToggle(Dropdown::Keyboard),
            label: fl!("keyboard-layout"),
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
        })
    }

    /// Tela estilizada (5 layouts + tipografia configurável).
    fn menu(&self, id: SurfaceId) -> Element<'_, Message> {
        crate::perf::count(crate::perf::Counter::View);
        crate::perf::mark_first_view();
        let cfg = &self.style_config;
        let user_idx = self.selected_username.data_idx;
        let user_data = user_idx.and_then(|i| self.flags.user_datas.get(i));
        let military_time = cfg.clock_24h;
        let metrics = login_ui::screen_metrics(self.common.screen_height(id));
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

        let accepts = crate::locker::accepts_input(self.authenticating, self.motion.exiting());
        let field_look = self.motion.look(Part::Field);
        let field_fx = || login_ui::FieldFx {
            look: field_look,
            hover: self.motion.hover_t(id, HoverTarget::Field),
            focus: self.motion.focus_t(),
            submit_hover: self.motion.hover_t(id, HoverTarget::Submit),
            field_hovers: Some(hovers(id, HoverTarget::Field)),
            submit_hovers: Some(hovers(id, HoverTarget::Submit)),
        };
        let mut field = None;
        if let SocketState::Open = &self.socket_state {
            if self.needs_username() {
                let value = self.selected_username.username.as_str();
                let input = widget::text_input(fl!("type-username"), value)
                    .padding([0, 10])
                    .line_height(iced::widget::text::LineHeight::Absolute(
                        login_ui::SUBMIT_SIZE.into(),
                    ))
                    .id(USERNAME_ID.clone())
                    .on_input(|input| Message::EnterUser(false, input))
                    .on_submit(Message::Username)
                    .on_focus(Message::FieldFocus(true))
                    .on_unfocus(Message::FieldFocus(false));
                let submit = (!value.is_empty()).then(|| Message::Username(value.to_string()));
                field = Some(styles::FieldParts {
                    input,
                    submit,
                    fx: field_fx(),
                });
            } else if let Some((prompt, secret, Some(value))) = &self.common.prompt_opt {
                let text_input_id = self
                    .common
                    .surface_names
                    .get(&id)
                    .and_then(|id| self.common.text_input_ids.get(id))
                    .cloned()
                    .unwrap_or_else(|| cosmic::widget::Id::new("text_input"));
                let mut text_input = login_ui::password_input(
                    login_ui::field_placeholder(prompt, *secret),
                    value.as_str(),
                    *secret,
                    common::Message::Prompt(prompt.clone(), !*secret, Some(value.clone())).into(),
                    field_look.alpha,
                )
                .id(text_input_id);

                // Sem entrada durante a autenticação nem durante o fade de saída, mas o campo
                // continua visível (as duas telas ficam iguais).
                if accepts {
                    text_input = text_input
                        .on_input(|input| {
                            common::Message::Prompt(prompt.clone(), *secret, Some(input)).into()
                        })
                        .on_submit(|v| Message::Auth(Some(v)));
                }
                text_input = text_input
                    .on_focus(Message::FieldFocus(true))
                    .on_unfocus(Message::FieldFocus(false));

                let submit = (accepts && !value.is_empty())
                    .then(|| Message::Auth(Some(value.clone())));
                field = Some(styles::FieldParts {
                    input: text_input,
                    submit,
                    fx: field_fx(),
                });
            }
            // Prompt sem valor (ex.: PAM_TEXT_INFO do leitor de digital): só a linha de status.
        }

        let full_name = user_data
            .map(|d| d.full_name.as_str())
            .unwrap_or(self.selected_username.username.as_str());
        let icon = user_idx
            .and_then(|i| self.flags.user_icons.get(i))
            .and_then(|icon| icon.as_ref());

        let power = crate::power::menu_visible(self.flags.logind_available, self.flags.nested)
            .then(|| styles::PowerParts {
                open: self.dropdown_opt == Some(Dropdown::Power),
                on_toggle: Message::DropdownToggle(Dropdown::Power),
                on_action: Message::Power,
                fx: login_ui::PowerFx {
                    look: self.motion.look(Part::Power),
                    hover: self.motion.hover_t(id, HoverTarget::Power),
                    menu: self.motion.menu_look(),
                    hovers: Some(hovers(id, HoverTarget::Power)),
                },
            });

        styles::view(styles::ScreenInput {
            cfg,
            metrics,
            clock: clock_data,
            user: styles::UserParts { icon, full_name },
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
            session: self.session_menu(),
            keyboard: self.keyboard_menu(),
            system: &self.common.system_info,
            backdrop: self
                .common
                .main_surface_of(id)
                .and_then(|main| self.ready_backdrop(main))
                .map(|prepared| &prepared.analysis),
        })
    }

    /// Layout original do upstream (cartão com relógio grande); fica escondido, não apagado.
    #[allow(dead_code)]
    fn menu_upstream(&self, id: SurfaceId) -> Element<'_, Message> {
        let window_width = self
            .common
            .window_size
            .get(&id)
            .map(|s| s.width)
            .unwrap_or(MAX_WIDTH);
        let menu_width = if window_width > MAX_WIDTH {
            MAX_WIDTH
        } else {
            window_width
        };
        let left_element = {
            let military_time = self
                .selected_username
                .data_idx
                .and_then(|i| self.flags.user_datas.get(i))
                .map(|user_data| user_data.time_applet_config.military_time)
                .unwrap_or_default();
            let date_time_column = self.common.time.date_time_widget(military_time);

            let mut status_row = widget::row::with_capacity(2)
                .padding(16.0)
                .spacing(12.0)
                .align_y(Alignment::Center);

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
            fn menu_checklist<'a>(
                label: impl Into<std::borrow::Cow<'a, str>> + 'a,
                value: bool,
                message: Message,
            ) -> Element<'a, Message> {
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
            }
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

                let menu = widget::container(items)
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
                    Element::from(t.container((id, true), menu))
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

            let mut user_button = widget::popover(
                widget::button::custom(widget::icon::from_name("system-users-symbolic"))
                    .padding(12.0)
                    .on_press(Message::DropdownToggle(Dropdown::User)),
            )
            .position(widget::popover::Position::Bottom);
            if matches!(self.dropdown_opt, Some(Dropdown::User)) {
                let mut items = Vec::with_capacity(self.usernames.len());
                for (name, full_name) in self.usernames.iter() {
                    items.push(menu_checklist(
                        full_name,
                        name == &self.selected_username.username,
                        Message::Username(name.clone()),
                    ));
                }
                let item_cnt = items.len();
                let menu_button = widget::menu::menu_button(vec![
                    Element::from(widget::space::horizontal().width(Length::Fixed(10.0))),
                    widget::text(fl!("enter-user"))
                        .align_x(iced::alignment::Horizontal::Left)
                        .into(),
                ])
                .on_press(Message::EnterUser(true, String::new()))
                .into();
                let items = if item_cnt >= 6 {
                    dropdown_menu(vec![
                        widget::scrollable(widget::column::with_children(items))
                            .height(Length::Fixed(DEFAULT_MENU_ITEM_HEIGHT * 6.))
                            .into(),
                        widget::divider::horizontal::light().into(),
                        menu_button,
                    ])
                } else {
                    items.push(menu_button);
                    dropdown_menu(items)
                };

                user_button = user_button.popup(items);
            }

            let mut session_button = widget::popover(
                widget::button::custom(widget::icon::from_name("application-menu-symbolic"))
                    .padding(12.0)
                    .on_press(Message::DropdownToggle(Dropdown::Session)),
            )
            .position(widget::popover::Position::Bottom);
            if matches!(self.dropdown_opt, Some(Dropdown::Session)) {
                let mut items = Vec::with_capacity(self.session_names.len());
                for session_name in self.session_names.iter() {
                    items.push(menu_checklist(
                        session_name,
                        session_name == &self.selected_session,
                        Message::Session(session_name.clone()),
                    ));
                }
                session_button = session_button.popup(dropdown_menu(items));
            }

            // Accessibility menu as a popup dialog
            let mut accessibility_dropdown = widget::popover(
                widget::button::custom(widget::icon::from_name(
                    "applications-accessibility-symbolic",
                ))
                .padding(12.0)
                .on_press(Message::DropdownToggle(Dropdown::Accessibility)),
            )
            .position(widget::popover::Position::Bottom);

            if matches!(self.dropdown_opt, Some(Dropdown::Accessibility)) {
                let mut items = Vec::new();
                items.push(menu_checklist(
                    fl!("accessibility", "screen-reader"),
                    self.accessibility.screen_reader.is_some(),
                    Message::ScreenReader(self.accessibility.screen_reader.is_none()),
                ));
                items.push(menu_checklist(
                    fl!("accessibility", "magnifier"),
                    self.accessibility.magnifier,
                    Message::Magnifier(!self.accessibility.magnifier),
                ));
                items.push(menu_checklist(
                    fl!("accessibility", "high-contrast"),
                    self.accessibility.high_contrast,
                    Message::HighContrast(!self.accessibility.high_contrast),
                ));
                items.push(menu_checklist(
                    fl!("accessibility", "invert-colors"),
                    self.accessibility.invert_colors,
                    Message::InvertColors(!self.accessibility.invert_colors),
                ));
                accessibility_dropdown = accessibility_dropdown.popup(dropdown_menu(items));
            }

            let accessibility_button = accessibility_dropdown;

            let mut button_row = iced::widget::row![
                widget::tooltip(
                    accessibility_button,
                    text(fl!("accessibility")),
                    widget::tooltip::Position::Top
                ),
                widget::tooltip(
                    widget::button::custom(widget::icon::from_name("input-keyboard-symbolic"))
                        .padding(12.0)
                        .on_press(Message::Common(common::Message::OnScreenKeyboard)),
                    text(fl!("on-screen-keyboard")),
                    widget::tooltip::Position::Top,
                ),
                widget::tooltip(
                    input_button,
                    text(fl!("keyboard-layout")),
                    widget::tooltip::Position::Top
                ),
                widget::tooltip(
                    user_button,
                    text(fl!("user")),
                    widget::tooltip::Position::Top
                ),
                widget::tooltip(
                    session_button,
                    text(fl!("session")),
                    widget::tooltip::Position::Top
                ),
            ];
            if self.flags.logind_available {
                button_row = button_row
                    .push(widget::tooltip(
                        widget::button::custom(widget::icon::from_name("system-suspend-symbolic"))
                            .padding(12.0)
                            .on_press(Message::Suspend),
                        text(fl!("suspend")),
                        widget::tooltip::Position::Top,
                    ))
                    .push(widget::tooltip(
                        widget::button::custom(widget::icon::from_name("system-reboot-symbolic"))
                            .padding(12.0)
                            .on_press(Message::Restart),
                        text(fl!("restart")),
                        widget::tooltip::Position::Top,
                    ))
                    .push(widget::tooltip(
                        widget::button::custom(widget::icon::from_name("system-shutdown-symbolic"))
                            .padding(12.0)
                            .on_press(Message::Shutdown),
                        text(fl!("shutdown")),
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

            let military_time = self
                .selected_username
                .data_idx
                .and_then(|i| self.flags.user_datas.get(i))
                .map(|user_data| user_data.time_applet_config.military_time)
                .unwrap_or_default();
            let space_height = match military_time {
                true => 63.0,
                false => 10.0,
            };

            // Add top spacing for better visual appearance
            // Bottom of the password text input field should align with bottom of time widget
            column = column.push(widget::space::vertical().height(Length::Fixed(space_height)));

            match &self.socket_state {
                SocketState::Pending => {
                    column = column.push(widget::text("Opening GREETD_SOCK"));
                }
                SocketState::Open => {
                    for (user_data, user_icon) in self
                        .flags
                        .user_datas
                        .iter()
                        .zip(self.flags.user_icons.iter())
                    {
                        if !self.entering_name && user_data.name == self.selected_username.username
                        {
                            // Display user icon or empty transparent box
                            if let Some(icon_handle) = user_icon {
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
                                    widget::container(
                                        widget::space::horizontal().width(Length::Fixed(78.0)),
                                    )
                                    .padding(0.0)
                                    .width(Length::Fill)
                                    .height(Length::Fixed(78.0))
                                    .align_x(Alignment::Center),
                                );
                            }
                            column = column.push(
                                widget::container(widget::text::title4(&user_data.full_name))
                                    .width(Length::Fill)
                                    .align_x(Alignment::Center),
                            );
                        }
                    }
                    if self.entering_name {
                        column = column.push(
                            widget::text_input(
                                fl!("type-username"),
                                self.selected_username.username.as_str(),
                            )
                            .id(USERNAME_ID.clone())
                            .on_input(|input| Message::EnterUser(false, input))
                            .on_submit(Message::Username),
                        )
                    }
                    if let Some((prompt, secret, value_opt)) = &self.common.prompt_opt
                        && !self.entering_name
                    {
                        match value_opt {
                            Some(value) => {
                                // Only show password input when not authenticating
                                if !self.authenticating {
                                    let text_input_id = self
                                        .common
                                        .surface_names
                                        .get(&id)
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
                                    .id(text_input_id)
                                    .on_input(|input| {
                                        common::Message::Prompt(
                                            prompt.clone(),
                                            *secret,
                                            Some(input),
                                        )
                                        .into()
                                    })
                                    .on_submit(|v| Message::Auth(Some(v)));

                                    if let Some(text_input_id) = self
                                        .common
                                        .surface_names
                                        .get(&id)
                                        .and_then(|id| self.common.text_input_ids.get(id))
                                    {
                                        text_input = text_input.id(text_input_id.clone());
                                    }

                                    if *secret {
                                        text_input = text_input.password()
                                    }

                                    column = column.push(text_input);

                                    if self.common.caps_lock {
                                        column = column.push(widget::text(fl!("caps-lock")));
                                    } else if self.common.error_opt.is_none() {
                                        column = column.push(widget::text(""));
                                    }
                                }
                            }
                            None => {
                                // `value_opt == None` is used for non-interactive auth messages
                                // (e.g. PAM_TEXT_INFO via greetd). This is where fingerprint
                                // prompts typically come through, so show the message to the user.
                                column = column.push(widget::text(prompt));
                            }
                        }
                    }
                }
                SocketState::NotSet => {
                    column = column.push(widget::text("GREETD_SOCK variable not set"));
                }
                SocketState::Error(err) => {
                    column = column.push(widget::text(format!(
                        "Failed to open GREETD_SOCK: {:?}",
                        err
                    )))
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

            id_container(
                widget::container(column)
                    .align_x(Alignment::Center)
                    .width(Length::Fill),
                if self.entering_name {
                    iced::id::Id::new("entering_name")
                } else {
                    iced::id::Id::new("main_menu")
                },
            )
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
                appearance.background = Some(iced::Background::Color(
                    // TODO if we can use popups instead of subsurfaces for the greeter and the lockscreen
                    // then we can allow transparency
                    theme.cosmic().background(theme.transparent).base.into(),
                ));
                appearance.border = iced::Border::default().rounded(16);
                appearance
            },
        )))
        .width(Length::Fixed(MAX_WIDTH));
        let menu = if let Some(t) = self.common.rectangle_tracker.as_ref() {
            Element::from(t.container((id, false), menu))
        } else {
            menu.into()
        };
        let menu = widget::container(widget::column::with_children(vec![
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
        .align_x(Alignment::Center);

        // TODO make these opaque?
        let popover = widget::popover(menu).modal(true);
        match self.dialog_page_opt {
            Some(DialogPage::Restart(instant)) => {
                let remaining = DialogPage::remaining(instant).unwrap_or_default();
                popover
                    .popup(
                        widget::dialog()
                            .title(fl!("restart-now"))
                            .icon(widget::icon::from_name("system-reboot-symbolic").size(64))
                            .body(fl!("restart-timeout", seconds = remaining.as_secs()))
                            .primary_action(
                                widget::button::suggested(fl!("restart"))
                                    .on_press(Message::DialogConfirm),
                            )
                            .secondary_action(
                                widget::button::standard(fl!("cancel"))
                                    .on_press(Message::DialogCancel),
                            ),
                    )
                    .into()
            }
            Some(DialogPage::Shutdown(instant)) => {
                let remaining = DialogPage::remaining(instant).unwrap_or_default();
                popover
                    .popup(
                        widget::dialog()
                            .title(fl!("shutdown-now"))
                            .icon(widget::icon::from_name("system-shutdown-symbolic").size(64))
                            .body(fl!("shutdown-timeout", seconds = remaining.as_secs()))
                            .primary_action(
                                widget::button::suggested(fl!("shutdown"))
                                    .on_press(Message::DialogConfirm),
                            )
                            .secondary_action(
                                widget::button::standard(fl!("cancel"))
                                    .on_press(Message::DialogCancel),
                            ),
                    )
                    .into()
            }
            None => popover.into(),
        }
    }

    /// Send a [`Request`] to the greetd IPC subscription.
    fn send_request(&self, request: Request) {
        if let Some(ref sender) = self.greetd_sender {
            let sender = sender.clone();
            tokio::task::spawn(async move {
                _ = sender.send(request).await;
            });
        }
    }

    fn update_user_data(&mut self) -> Task<Message> {
        let user_data = match self
            .selected_username
            .data_idx
            .and_then(|i| self.flags.user_datas.get(i))
        {
            Some(some) => some,
            None => {
                return Task::none();
            }
        };

        self.common.update_user_data(user_data);

        if let Some(builder) = &user_data.theme_builder_opt {
            self.theme_builder = builder.clone();
        }

        let mut tasks = Vec::new();
        self.accessibility.magnifier = user_data.accessibility_zoom.start_on_login;
        self.randr_list = None;
        tasks.push(cosmic::Task::future(async {
            let randr_fut = cosmic_randr_shell::list().await;
            cosmic::action::app(Message::RandrUpdate {
                randr: Arc::new(randr_fut),
            })
        }));
        if let Some(theme) = &user_data.theme_opt {
            self.accessibility.high_contrast = theme.is_high_contrast;
            tasks.push(cosmic::command::set_theme(cosmic::Theme::custom(Arc::new(
                theme.clone(),
            ))));
        }

        Task::batch(tasks)
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
    fn init(mut core: Core, flags: Self::Flags) -> (Self, Task<Message>) {
        core.set_app_type(cosmic::core::AppType::System);
        let mut tasks = Vec::new();
        let (mut common, common_task) = Common::init(core);
        common.on_output_event = Some(Box::new(|output_event, output| {
            Message::OutputEvent(output_event, output)
        }));
        tasks.push(common_task);

        //TODO: use full_name?
        let mut usernames: Vec<_> = flags
            .user_datas
            .iter()
            .map(|x| (x.name.clone(), x.full_name.clone()))
            .collect();
        usernames.sort_by(|a, b| a.1.cmp(&b.1));

        let style_config = StyleConfig::load();

        let users: Vec<(String, u32)> = flags
            .user_datas
            .iter()
            .map(|x| (x.name.clone(), x.uid))
            .collect();
        let last_user_uid = flags.greeter_config.last_user.as_ref().map(|u| u.get());
        let (username, uid) = selection::pick_user(&style_config.default_user, last_user_uid, &users)
            .and_then(|i| flags.user_datas.get(i))
            .map(|x| (x.name.clone(), NonZeroU32::new(x.uid)))
            .unwrap_or_default();

        let mut session_names: Vec<_> = flags.sessions.keys().map(|x| x.to_string()).collect();
        session_names.sort();

        let last_session = uid.and_then(|uid| {
            flags
                .greeter_config
                .users
                .get(&uid)
                .and_then(|user| user.last_session.as_deref())
        });
        let selected_session =
            selection::pick_session(&style_config.session, last_session, &session_names)
                .unwrap_or_default();
        let data_idx = flags.user_datas.iter().position(|d| d.name == username);
        let selected_username = NameIndexPair { username, data_idx };
        let accessibility = Accessibility {
            helper: cosmic_settings_daemon_config::greeter::GreeterAccessibilityState::config()
                .ok(),
            ..Default::default()
        };

        // Modos rotativos: a imagem da vez (pelo horário), já na primeira pintura. A leitura do
        // arquivo é a mesma de antes; decodificar/desfocar/hashear só no job de `backdrop`.
        let now_secs = unix_now();
        let custom_wallpaper_path = initial_wallpaper(&style_config, now_secs);
        let custom_wallpaper = custom_wallpaper_path
            .as_deref()
            .and_then(style_config::load_wallpaper_handle);
        let rotation_slot = style_config.rotation_slot(now_secs);

        let mut app = App {
            common,
            flags,
            greetd_sender: None,
            socket_state: SocketState::Pending,
            socket_error_text: String::new(),
            usernames,
            selected_username,
            session_names,
            selected_session,
            dialog_page_opt: None,
            dropdown_opt: None,
            heartbeat_handle: None,
            entering_name: false,
            accessibility,
            theme_builder: Default::default(),
            randr_list: None,
            surface_id_pairs: Vec::new(),
            authenticating: false,
            motion: Motion::new(Instant::now()),
            exit_seq: 0,
            demo_slot: 0,
            style_config,
            custom_wallpaper,
            custom_wallpaper_path,
            backdrops: BackdropSet::default(),
            rotation_slot,
        };
        // Grava a chave do status sem fade (nada de fade de 150 ms na partida).
        let key = login_ui::status_key(&app.current_status());
        app.motion.set_status_key(key);
        // Conteúdo escondido até a superfície abrir (`SubsurfaceOpened`); se o primeiro quadro
        // demorar (decode do papel de parede 4K), `ARM_TIMEOUT` mostra tudo sozinho.
        app.motion.arm_entry(Instant::now());
        (app, Task::batch(tasks))
    }

    /// Handle application events here.
    /// tick → `update_inner` → observa o status (fade só quando a linha muda).
    fn update(&mut self, message: Self::Message) -> Task<Message> {
        crate::perf::count(crate::perf::Counter::Update);
        let now = Instant::now();
        self.motion.tick(now);
        let task = self.update_inner(message);
        let key = login_ui::status_key(&self.current_status());
        self.motion.observe_status(key, now);
        Task::batch([task, self.spawn_backdrops()])
    }

    // Not used for layer surface window
    fn view(&self) -> Element<'_, Self::Message> {
        unimplemented!()
    }

    /// Creates a view after each update.
    fn view_window(&self, surface_id: SurfaceId) -> Element<'_, Self::Message> {
        crate::perf::count(crate::perf::Counter::WallpaperView);
        let img = self
            .surface_image(surface_id)
            .unwrap_or(&self.common.fallback_background);
        // A primeira pintura nunca espera o job: sem resultado pronto (ou de outro tag) é a original.
        let blur = self.style_config.blur_radius;
        let img = match self.surface_backdrop_tag(surface_id) {
            Some(tag) => {
                backdrop::pick_view_handle(blur, self.backdrops.get(surface_id), tag, img)
            }
            None => img,
        };
        login_ui::wallpaper_view(img, self.style_config.dim_opacity)
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let mut subs = vec![
            self.common.subscription().map(Message::from),
            ipc::subscription(),
            wayland::a11y_subscription().map(Message::WaylandUpdate),
            listen_with(|event, _status, id| match event {
                iced::Event::Window(window::Event::Resized(size))
                | iced::Event::Window(window::Event::Opened { size, .. }) => {
                    Some(Message::RepositionMenu(id, size))
                }
                // Mouse saindo da superfície (o `mouse_area` não avisa isso).
                iced::Event::Mouse(iced::mouse::Event::CursorLeft) => Some(Message::CursorLeft(id)),
                _ => None,
            }),
            // Ticks de animação: a assinatura só existe enquanto algo se mexe.
            self.motion.frames().map(Message::Frame),
            // Roteiro de demo (só aninhado com a variável de ambiente).
            motion::demo_ticks(self.flags.demo).map(|_| Message::DemoTick),
        ];
        // Timer de 1 s só quando o estilo mostra segundos; a rotação usa o tick de minuto.
        if self.style_config.needs_second_tick() {
            subs.push(
                cosmic::iced::time::every(Duration::from_secs(1))
                    .map(|_| Message::Common(common::Message::Tick)),
            );
        }
        Subscription::batch(subs)
    }
}

impl App {
    /// Sem usuário (nenhum último usuário nem lista), o campo de nome continua sendo o caminho
    /// do upstream para digitar o usuário.
    fn needs_username(&self) -> bool {
        self.entering_name
            || (self.selected_username.data_idx.is_none()
                && self.selected_username.username.is_empty())
    }

    /// A linha de status que `menu` mostra (mesma escolha, ver `greeter_status`).
    fn current_status(&self) -> login_ui::StatusLine<'_> {
        greeter_status(
            &self.socket_state,
            self.socket_error_text.as_str(),
            self.needs_username(),
            &self.common.prompt_opt,
            self.authenticating,
            self.common.caps_lock,
            self.common.error_opt.as_deref(),
        )
    }

    /// O antigo corpo de `Message::Login`, sem mudança (fora `update_inner`): pede o
    /// `StartSession` ao greetd e grava a última sessão do usuário.
    fn start_session(&mut self) -> Task<Message> {
        self.common.prompt_opt = None;
        self.common.error_opt = None;
        self.authenticating = false;

        // A sessão escolhida pode ter sumido (config com nome inexistente, .desktop removido):
        // recalcula a partir da configuração; sem nenhuma sessão, mostra o erro e não cai.
        if !self.flags.sessions.contains_key(&self.selected_session) {
            match selection::pick_session(&self.style_config.session, None, &self.session_names) {
                Some(session) => self.selected_session = session,
                None => {
                    self.common.error_opt = Some(fl!("session-not-found"));
                    return Task::none();
                }
            }
        }

        match self.flags.sessions.get(&self.selected_session).cloned() {
            Some((cmd, env)) => {
                self.send_request(Request::StartSession { cmd, env });
                self.update_inner(Message::ConfigUpdateUser)
            }
            None => {
                self.common.error_opt = Some(fl!("session-not-found"));
                Task::none()
            }
        }
    }

    /// O antigo `update`: os braços ficam como estavam (mais o movimento).
    fn update_inner(&mut self, message: Message) -> Task<Message> {
        let now = self.motion.now();
        match message {
            Message::Common(common_message) => {
                if matches!(&common_message, common::Message::Prompt(_, _, Some(_)))
                    && self.authenticating
                {
                    self.authenticating = false;
                    self.common.prompt_opt = None;
                }
                // In greetd's IPC protocol, the greeter must acknowledge auth messages by
                // sending PostAuthMessageResponse. For non-interactive "info" messages
                // (fingerprint prompts typically come through here), the correct response
                // is `None`. If we don't ACK, greetd will wait forever and the UI will
                // appear "stuck" on the last info message.
                if let common::Message::Prompt(_, _secret, None) = &common_message {
                    self.send_request(Request::PostAuthMessageResponse { response: None });
                }
                // A superfície apareceu: a cascata de entrada começa (se armada).
                if matches!(&common_message, common::Message::SubsurfaceOpened(_)) {
                    self.motion.start_entry(now);
                    self.motion.set_focus(true, now);
                }

                let is_tick = matches!(&common_message, common::Message::Tick);
                let task = self.common.update(common_message);
                if is_tick {
                    return Task::batch([task, self.rotate_wallpaper()]);
                }
                return task;
            }
            Message::Backdrop(surface_id, result) => {
                match result {
                    Ok(prepared) => {
                        self.backdrops.accept(surface_id, prepared);
                    }
                    // O pedido fica marcado como pendente: o mesmo tag não é pedido de novo, então o
                    // aviso sai uma vez por tag; a tela segue com a imagem original.
                    Err(err) => tracing::warn!("desfoque do fundo indisponível: {err}"),
                }
            }
            Message::Wallpaper(slot, loaded) => {
                if self.rotation_slot == Some(slot) {
                    if let Some((path, handle)) = loaded {
                        if self.custom_wallpaper_path.as_ref() != Some(&path) {
                            self.custom_wallpaper = Some(handle);
                            self.custom_wallpaper_path = Some(path);
                        }
                    }
                }
            }
            Message::OutputEvent(output_event, output) => {
                match output_event {
                    OutputEvent::Created(output_info_opt) => {
                        tracing::info!("output {}: created", output.id());

                        let surface_id = SurfaceId::unique();
                        let subsurface_id = SurfaceId::unique();
                        self.surface_id_pairs.push((surface_id, subsurface_id));

                        if let Some(old_surface_id) =
                            self.common.surface_ids.insert(output.clone(), surface_id)
                        {
                            //TODO: remove old surface?
                            tracing::warn!(
                                "output {}: already had surface ID {:?}",
                                output.id(),
                                old_surface_id
                            );
                        }
                        let menu_logical_size =
                            output_info_opt.as_ref().and_then(|info| info.logical_size);
                        let size = if let Some((w, h)) =
                            output_info_opt.as_ref().and_then(|info| info.logical_size)
                        {
                            Some((Some(w as u32), Some(h as u32)))
                        } else {
                            Some((None, None))
                        };
                        match output_info_opt {
                            Some(output_info) => match output_info.name {
                                Some(output_name) => {
                                    self.common
                                        .surface_names
                                        .insert(surface_id, output_name.clone());
                                    self.common
                                        .surface_names
                                        .insert(subsurface_id, output_name.clone());
                                    self.common.surface_images.remove(&surface_id);
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

                        let unwrapped_size = size
                            .map(|s| (s.0.unwrap_or(1920), s.1.unwrap_or(1080)))
                            .unwrap_or((1920, 1080));
                        // A subsuperfície cobre o monitor inteiro (layout C, sem cartão).
                        let rect = common::menu_rect(menu_logical_size);
                        let (loc, sub_size) = (rect.position(), rect.size());
                        self.common.window_size.insert(
                            surface_id,
                            Size::new(unwrapped_size.0 as f32, unwrapped_size.1 as f32),
                        );
                        self.common.subsurface_rects.insert(output.clone(), rect);
                        self.common
                            .subsurface_outputs
                            .insert(subsurface_id, output.clone());
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
                        return Task::batch([
                            self.update_user_data(),
                            cosmic::surface::surface_task(app_layer_shell(
                                |_: &App| LiveSettings {
                                    padding: None,
                                    corners: None,
                                    blur: Some(false),
                                },
                                move |_: &mut App| {
                                    let output = output.clone();
                                    SctkLayerSurfaceSettings {
                                        id: surface_id,
                                        layer: Layer::Top,
                                        keyboard_interactivity: KeyboardInteractivity::Exclusive,
                                        input_zone: None,
                                        anchor: Anchor::TOP
                                            | Anchor::LEFT
                                            | Anchor::BOTTOM
                                            | Anchor::RIGHT,
                                        output: IcedOutput::Output(output),
                                        namespace: "cosmic-greeter".into(),
                                        size: Some((None, None)),
                                        margin: IcedMargin {
                                            top: 0,
                                            bottom: 0,
                                            left: 0,
                                            right: 0,
                                        },
                                        exclusive_zone: -1,
                                        size_limits: iced::Limits::NONE
                                            .min_width(1.0)
                                            .min_height(1.0),
                                    }
                                },
                                None,
                            )),
                            cosmic::task::message(cosmic::Action::Surface(msg)),
                        ]);
                    }
                    OutputEvent::Removed => {
                        tracing::info!("output {}: removed", output.id());
                        match self.common.surface_ids.remove(&output) {
                            Some(surface_id) => {
                                self.common.surface_images.remove(&surface_id);
                                self.backdrops.forget(surface_id);
                                self.common.window_size.remove(&surface_id);
                                if let Some(n) = self.common.surface_names.remove(&surface_id) {
                                    self.common.text_input_ids.remove(&n);
                                }
                                return destroy_layer_surface(surface_id);
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
            Message::Socket(socket_state) => {
                // Conexão nova (ou perdida) com o greetd: a sessão antiga acabou; se o fade de
                // saída estava em curso, a tela volta (senão ficaria invisível e sem entrada).
                self.motion.cancel_exit();
                if let SocketState::Error(err) = &socket_state {
                    self.socket_error_text = format!("Failed to open GREETD_SOCK: {err:?}");
                }
                self.socket_state = socket_state;
                if let SocketState::Open = &self.socket_state {
                    // When socket is opened, send create session
                    self.send_request(Request::CreateSession {
                        username: self.selected_username.username.clone(),
                    });
                    // Sem usuário nenhum, o campo de nome é o único caminho: foca ele.
                    if self.selected_username.data_idx.is_none()
                        && self.selected_username.username.is_empty()
                    {
                        return widget::text_input::focus(USERNAME_ID.clone());
                    }
                }
            }
            Message::Reload(new) => {
                return cosmic::command::set_theme(new.clone());
            }
            Message::Session(selected_session) => {
                self.selected_session = selected_session;
                if self.dropdown_opt == Some(Dropdown::Session) {
                    self.dropdown_opt = None;
                    return self.common.dropdown_blur_rects(false);
                }
            }
            Message::EnterUser(focus_input, username) => {
                if self.dropdown_opt == Some(Dropdown::User) {
                    self.dropdown_opt = None;
                }
                self.entering_name = true;
                self.selected_username = NameIndexPair {
                    data_idx: self
                        .flags
                        .user_datas
                        .iter()
                        .position(|d| d.name == username),
                    username,
                };
                if focus_input {
                    return Task::batch([
                        self.common.dropdown_blur_rects(false),
                        widget::text_input::focus(USERNAME_ID.clone()),
                    ]);
                }
            }
            Message::Username(username) => {
                if self.dropdown_opt == Some(Dropdown::User) {
                    self.dropdown_opt = None;
                }
                if self.entering_name || username != self.selected_username.username {
                    self.entering_name = false;
                    self.authenticating = false;
                    let data_idx = self
                        .flags
                        .user_datas
                        .iter()
                        .position(|d| d.name == username);
                    self.selected_username = NameIndexPair { username, data_idx };
                    self.common.surface_images.clear();
                    for surface_id in self.common.surface_ids.values() {
                        self.backdrops.forget(*surface_id);
                    }
                    let last_session = data_idx.and_then(|i| {
                        self.flags
                            .user_datas
                            .get(i)
                            .and_then(|UserData { uid, .. }| {
                                NonZeroU32::new(*uid).and_then(|uid| {
                                    self.flags
                                        .greeter_config
                                        .users
                                        .get(&uid)
                                        .and_then(|conf| conf.last_session.as_deref())
                                })
                            })
                    });
                    if let Some(session) = selection::pick_session(
                        &self.style_config.session,
                        last_session,
                        &self.session_names,
                    ) {
                        self.selected_session = session;
                    }
                    if let SocketState::Open = &self.socket_state {
                        self.common.prompt_opt = None;
                        self.send_request(Request::CancelSession);
                    }
                    if let Some(randr_list) = self.randr_list.as_ref() {
                        return Task::batch([
                            self.common.dropdown_blur_rects(false),
                            self.update_inner(Message::RandrUpdate {
                                randr: Arc::new(Ok(randr_list.clone())),
                            }),
                        ]);
                    }
                }
            }
            Message::ConfigUpdateUser => {
                let Some(user_entry) = self.selected_username.data_idx.and_then(|i| {
                    self.flags
                        .user_datas
                        .get(i)
                        .and_then(|UserData { uid, .. }| {
                            NonZeroU32::new(*uid)
                                .map(|uid| self.flags.greeter_config.users.entry(uid))
                        })
                }) else {
                    tracing::error!(
                        "Couldn't find user: {:?} {:?}",
                        self.selected_username.username,
                        self.selected_username.data_idx,
                    );
                    return Task::none();
                };

                let Some(handler) = self.flags.greeter_config_handler.as_mut() else {
                    tracing::error!(
                        "Failed to update config for {} (UID: {}): no config handler",
                        self.selected_username.username,
                        user_entry.key()
                    );
                    return Task::none();
                };

                let uid = *user_entry.key();
                self.flags.greeter_config.last_user = Some(uid);
                if let Err(err) = handler.set("last_user", self.flags.greeter_config.last_user) {
                    tracing::error!(
                        "Failed to set {:?} as last user: {:?}",
                        self.flags.greeter_config.last_user,
                        err
                    );
                }
                match user_entry {
                    hash_map::Entry::Vacant(entry) => {
                        let last_session = Some(self.selected_session.clone());
                        entry.insert(cosmic_greeter_config::user::UserState { uid, last_session });
                    }
                    hash_map::Entry::Occupied(mut entry) => {
                        let last_session = entry.get_mut().last_session.as_mut();
                        if last_session
                            .as_ref()
                            .is_some_and(|session| session.as_str() == self.selected_session)
                        {
                            return Task::none();
                        }
                        if let Some(session) = last_session {
                            self.selected_session.clone_into(session);
                        } else {
                            let last_session = Some(self.selected_session.clone());
                            entry.insert(cosmic_greeter_config::user::UserState {
                                uid,
                                last_session,
                            });
                        }
                    }
                }

                // xxx Not sure why this doesn't work unless the handler is used directly
                // if let Err(err) = self
                //     .flags
                //     .greeter_config
                //     .set_users(&handler, self.flags.greeter_config.users.clone())
                if let Err(err) = handler.set("users", &self.flags.greeter_config.users) {
                    tracing::error!(
                        "Failed to set {} as last selected session for {} (UID: {}): {:?}",
                        self.selected_session,
                        self.selected_username.username,
                        uid,
                        err
                    );
                }
            }
            Message::Auth(response) => {
                if response.as_deref() == Some("") {
                    return Task::none();
                }
                self.common.error_opt = None;
                self.authenticating = true;
                self.send_request(Request::PostAuthMessageResponse { response });
            }
            Message::AuthError(error) => {
                // Nunca passa pelo fade de saída: a tela volta e balança (senha enviada).
                restore_on_error(&mut self.motion, self.authenticating, now);
                self.common.error_opt = Some(error);
                if self.authenticating {
                    // Cancel failed password session so pam_faillock records the attempt.
                    self.authenticating = false;
                    self.send_request(Request::CancelSession);
                } else {
                    // The conversation continues, so acknowledge like any other
                    // non-interactive auth message rather than cancelling the session.
                    self.send_request(Request::PostAuthMessageResponse { response: None });
                }
            }
            Message::Login => {
                // Fade de saída; `StartSession` só quando o timer `EXIT_FADE` disparar.
                if let Some(seq) = begin_exit(&mut self.motion, &mut self.exit_seq, now) {
                    if crate::perf::enabled() {
                        crate::perf::line(&format!("exit: start t={}", crate::perf::now_ms()));
                    }
                    return cosmic::task::future(async move {
                        tokio::time::sleep(motion::EXIT_FADE).await;
                        Message::StartSessionReady(seq)
                    });
                }
                // Login repetido durante o fade (ou depois do StartSession): nada.
            }
            Message::StartSessionReady(seq) => {
                if exit_fires(&self.motion, self.exit_seq, seq) {
                    if crate::perf::enabled() {
                        crate::perf::line(&format!("exit: fire t={}", crate::perf::now_ms()));
                    }
                    return self.start_session();
                }
                tracing::info!("exit fade cancelled or superseded; not starting session");
            }
            Message::Error(error) => {
                // Erro durante/depois do fade: a tela volta para tentar de novo.
                restore_on_error(&mut self.motion, self.authenticating, now);
                self.common.error_opt = Some(error);
                self.authenticating = false;

                self.send_request(Request::CancelSession);
            }
            Message::Reconnect => {
                return self.update_user_data();
            }
            Message::DialogCancel => {
                self.dialog_page_opt = None;
                if let Some(handle) = self.heartbeat_handle.take() {
                    handle.abort();
                }
                if self.dropdown_opt.is_some() {
                    return self.common.dropdown_blur_rects(true);
                }
            }
            Message::DialogConfirm => match self.dialog_page_opt.take() {
                Some(DialogPage::Restart(_)) => {
                    #[cfg(feature = "logind")]
                    return cosmic::task::future::<(), ()>(async move {
                        match crate::logind::reboot().await {
                            Ok(()) => (),
                            Err(err) => {
                                tracing::error!("failed to reboot: {:?}", err);
                            }
                        }
                    })
                    .discard();
                }
                Some(DialogPage::Shutdown(_)) => {
                    #[cfg(feature = "logind")]
                    return cosmic::task::future::<(), ()>(async move {
                        match crate::logind::power_off().await {
                            Ok(()) => (),
                            Err(err) => {
                                tracing::error!("failed to power off: {:?}", err);
                            }
                        }
                    })
                    .discard();
                }
                None => {
                    if self.dropdown_opt.is_some() {
                        return self.common.dropdown_blur_rects(true);
                    }
                }
            },
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
            Message::KeyboardLayout(layout_i) => {
                if let Some(keyboard_layout) = &self.common.keyboard_layout {
                    keyboard_layout.set_group(layout_i as u32);
                }
                self.common.current_keyboard_layout = layout_i;
                if self.dropdown_opt == Some(Dropdown::Keyboard) {
                    self.dropdown_opt = None
                }
            }
            Message::Power(action) => {
                self.dropdown_opt = None;
                let nested = self.flags.nested;
                // Só registra no log: `Message::Error` cancelaria a sessão do greetd.
                return Task::batch([
                    self.common.dropdown_blur_rects(false),
                    cosmic::task::future::<(), ()>(async move {
                        if let Err(err) = crate::power::perform(action, nested).await {
                            tracing::error!("failed to {action:?}: {err}");
                        }
                    })
                    .discard(),
                ]);
            }
            Message::Suspend => {
                #[cfg(feature = "logind")]
                return cosmic::task::future::<(), ()>(async move {
                    match crate::logind::suspend().await {
                        Ok(()) => (),
                        Err(err) => {
                            tracing::error!("failed to suspend: {:?}", err);
                        }
                    }
                })
                .discard();
            }
            Message::Restart | Message::Shutdown => {
                let instant = Instant::now();

                self.dialog_page_opt = Some(if matches!(message, Message::Restart) {
                    DialogPage::Restart(instant)
                } else {
                    DialogPage::Shutdown(instant)
                });

                if self.heartbeat_handle.is_none() {
                    let (heartbeat, handle) = cosmic::task::stream(cosmic::iced::stream::channel(
                        1,
                        |mut msg_tx: iced::futures::channel::mpsc::Sender<_>| async move {
                            let mut interval = time::interval(Duration::from_secs(1));

                            loop {
                                // Send heartbeat once a second to update time
                                msg_tx
                                    .send(cosmic::Action::App(Message::Heartbeat))
                                    .await
                                    .unwrap();

                                interval.tick().await;
                            }
                        },
                    ))
                    .abortable();

                    self.heartbeat_handle = Some(handle);
                    self.common.include_menu = false;
                    return Task::batch(vec![self.common.dropdown_blur_rects(false), heartbeat]);
                }
            }
            Message::Heartbeat => match self.dialog_page_opt {
                Some(DialogPage::Restart(instant)) | Some(DialogPage::Shutdown(instant)) => {
                    if DialogPage::remaining(instant).is_none() {
                        return self.update_inner(Message::DialogConfirm);
                    }
                }
                None => {}
            },
            Message::Exit => {
                let mut commands = Vec::new();
                for (_output, surface_id) in self.common.surface_ids.drain() {
                    self.common.surface_images.remove(&surface_id);
                    self.backdrops.forget(surface_id);
                    self.common.surface_names.remove(&surface_id);
                    if let Some(n) = self.common.surface_names.remove(&surface_id) {
                        self.common.text_input_ids.remove(&n);
                    }
                    commands.push(destroy_layer_surface(surface_id));
                }
                commands.push(Task::perform(async { process::exit(0) }, |x| x));
                return Task::batch(commands);
            }
            Message::GreetdChannel(sender) => {
                self.greetd_sender = Some(sender);
            }
            Message::Surface(a) => {
                return cosmic::task::message(cosmic::Action::Surface(a));
            }
            Message::ScreenReader(enabled) => {
                if enabled
                    && self
                        .accessibility
                        .screen_reader
                        .as_mut()
                        .is_none_or(|c| c.try_wait().is_ok())
                {
                    self.accessibility.screen_reader =
                        tokio::process::Command::new("/usr/bin/orca").spawn().ok();
                } else if let Some(mut c) = self.accessibility.screen_reader.take() {
                    return cosmic::task::future::<(), ()>(async move {
                        if let Err(err) = c.kill().await {
                            tracing::error!("Failed to stop screen reader: {err:?}");
                        }
                    })
                    .discard();
                }

                if let Some(helper) = self.accessibility.helper.as_ref() {
                    _ = self
                        .accessibility
                        .state
                        .set_screen_reader(helper, Some(enabled));
                }
            }
            Message::Magnifier(enabled) => {
                if let Some(tx) = &self.accessibility.wayland_sender {
                    self.accessibility.magnifier = enabled;
                    let _ = tx.send(AccessibilityRequest::Magnifier(enabled));
                    if let Some(helper) = self.accessibility.helper.as_ref() {
                        _ = self
                            .accessibility
                            .state
                            .set_magnifier(helper, Some(enabled));
                    }
                } else {
                    self.accessibility.magnifier = false;
                }
            }
            Message::HighContrast(enabled) => {
                self.accessibility.high_contrast = enabled;

                if let Some(helper) = self.accessibility.helper.as_ref() {
                    _ = self
                        .accessibility
                        .state
                        .set_high_contrast(helper, Some(enabled));
                }
                let builder = self.theme_builder.clone();

                return cosmic::task::future::<_, _>(async move {
                    let builder = builder.clone();
                    let (tx, rx) = tokio::sync::oneshot::channel();
                    std::thread::spawn(move || match apply_hc_theme(builder, enabled) {
                        Ok(t) => {
                            _ = tx.send(Some(t));
                        }
                        Err(err) => {
                            tracing::error!("{err:?}");
                            _ = tx.send(None);
                        }
                    });
                    if let Ok(Some(theme)) = rx.await {
                        cosmic::Action::App(Message::Reload(cosmic::Theme::custom(
                            std::sync::Arc::new(theme),
                        )))
                    } else {
                        cosmic::Action::None
                    }
                });
            }
            Message::InvertColors(enabled) => {
                if let Some(tx) = &self.accessibility.wayland_sender {
                    self.accessibility.invert_colors = enabled;
                    let _ = tx.send(AccessibilityRequest::ScreenFilter {
                        inverted: enabled,
                        filter: None,
                    });
                    if let Some(helper) = self.accessibility.helper.as_ref() {
                        _ = self
                            .accessibility
                            .state
                            .set_invert_colors(helper, Some(enabled));
                    }
                } else {
                    self.accessibility.invert_colors = false;
                }
            }
            Message::WaylandUpdate(update) => match update {
                WaylandUpdate::Errored => {
                    let _ = self.accessibility.wayland_sender.take();
                    self.accessibility.wayland_protocol_version = None;
                    self.accessibility.magnifier = false;
                    self.accessibility.invert_colors = false;
                }
                WaylandUpdate::State(AccessibilityEvent::Bound(ver)) => {
                    self.accessibility.wayland_protocol_version = Some(ver);
                }
                WaylandUpdate::State(AccessibilityEvent::Magnifier(enabled)) => {
                    self.accessibility.magnifier = enabled;
                }
                WaylandUpdate::State(AccessibilityEvent::ScreenFilter { inverted, .. }) => {
                    self.accessibility.invert_colors = inverted;
                }
                WaylandUpdate::State(AccessibilityEvent::Closed) => {
                    self.accessibility.wayland_sender = None;
                    self.accessibility.wayland_protocol_version = None;
                }
                WaylandUpdate::Started(tx) => {
                    let _ = tx.send(AccessibilityRequest::ScreenFilter {
                        inverted: self.accessibility.invert_colors,
                        filter: None,
                    });
                    let _ = tx.send(AccessibilityRequest::Magnifier(
                        self.accessibility.magnifier,
                    ));

                    self.accessibility.wayland_sender = Some(tx);
                }
            },
            Message::RandrUpdate { randr } => match randr.as_ref() {
                Ok(outputs) => {
                    let mut tasks = Vec::new();
                    self.randr_list = Some(outputs.clone());

                    let mut list: Option<List> = None;

                    let Some(cur_user_output_state) = self
                        .selected_username
                        .data_idx
                        .and_then(|i| self.flags.user_datas.get(i))
                        .map(|user_data| &user_data.kdl_output_lists)
                    else {
                        return Task::none();
                    };
                    'outer: for configured_list in cur_user_output_state
                        .iter()
                        .filter_map(|s| match KdlDocument::parse(s) {
                            Ok(doc) => Some(doc),
                            Err(err) => {
                                tracing::warn!("Invalid output KDL {err:?}");
                                None
                            }
                        })
                        .map(|kdl| match List::try_from(kdl) {
                            Ok(list) => list,
                            Err(KdlParseWithError { list, errors }) => {
                                for err in errors {
                                    tracing::warn!("KDL output error: {err:?}");
                                }
                                list
                            }
                        })
                    {
                        if configured_list.outputs.len() != outputs.outputs.len() {
                            continue;
                        }

                        for o in outputs.outputs.values() {
                            if configured_list.outputs.values().all(|configured| {
                                configured.name != o.name
                                    || configured.make != o.make
                                    || configured.model != o.model
                            }) {
                                continue 'outer;
                            }
                        }
                        if list
                            .as_ref()
                            .is_none_or(|old| old.outputs.len() < configured_list.outputs.len())
                        {
                            list = Some(configured_list);
                        }
                    }
                    if let Some(list) = list {
                        tasks.push(self.exec_randr(list))
                    } else {
                        tracing::warn!("Failed to apply user display config");
                    }

                    return Task::batch(tasks);
                }
                Err(err) => {
                    tracing::error!("Randr error: {err}");
                }
            },
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
                if let Some(open) =
                    event.and_then(|e| crate::locker::apply_demo(&mut self.motion, e, now))
                {
                    self.dropdown_opt = open.then_some(Dropdown::Power);
                }
            }
            Message::RepositionMenu(id, _size) => {
                let Some(subsurface_id) = self
                    .surface_id_pairs
                    .iter()
                    .find_map(|(p, s)| (*p == id).then_some(s))
                else {
                    tracing::error!("Failed to find subsurface menu id");
                    return Task::none();
                };
                // O menu ocupa o monitor inteiro: a posição é sempre a origem. O tamanho
                // continua o do `Created` (não há comando para redimensionar a subsuperfície).
                let loc = Point::ORIGIN;
                return reposition_subsurface(*subsurface_id, loc.x as i32, loc.y as i32);
            }
        }
        Task::none()
    }
}

pub fn apply_hc_theme(
    builder: cosmic_theme::ThemeBuilder,
    enabled: bool,
) -> Result<cosmic_theme::Theme, cosmic_config::Error> {
    let is_dark = builder.palette.is_dark();
    let mut builder = builder.clone();

    builder.palette = if is_dark {
        if enabled {
            CosmicPalette::HighContrastDark(builder.palette.inner())
        } else {
            CosmicPalette::Dark(builder.palette.inner())
        }
    } else if enabled {
        CosmicPalette::HighContrastLight(builder.palette.inner())
    } else {
        CosmicPalette::Light(builder.palette.inner())
    };

    let new_theme = builder.build();

    Ok(new_theme)
}

#[cfg(test)]
mod motion_tests {
    use super::*;
    use crate::motion::{Motion, Part};
    use login_ui::StatusLine;

    fn prompt(value: Option<&str>) -> Option<(String, bool, Option<String>)> {
        Some(("Password: ".into(), true, value.map(String::from)))
    }

    #[test]
    fn status_choice_matches_menu_rules() {
        let open = SocketState::Open;
        // Socket fechado: só a info do socket; Caps Lock não conta (sem campo).
        assert_eq!(
            greeter_status(&SocketState::Pending, "", false, &prompt(Some("")), false, true, None),
            StatusLine::Info("Opening GREETD_SOCK")
        );
        assert_eq!(
            greeter_status(&SocketState::NotSet, "", false, &None, false, false, None),
            StatusLine::Info("GREETD_SOCK variable not set")
        );
        let err = SocketState::Error(Arc::new(io::Error::other("x")));
        assert_eq!(
            greeter_status(&err, "falhou", false, &None, false, true, None),
            StatusLine::Info("falhou")
        );
        // Campo de nome visível: Caps Lock conta.
        assert_eq!(
            greeter_status(&open, "", true, &None, false, true, None),
            StatusLine::CapsLock
        );
        // Campo de senha: autenticando vence tudo; erro depois do Caps Lock.
        assert_eq!(
            greeter_status(&open, "", false, &prompt(Some("")), true, true, Some("x")),
            StatusLine::Authenticating
        );
        assert_eq!(
            greeter_status(&open, "", false, &prompt(Some("")), false, false, Some("x")),
            StatusLine::Error("x")
        );
        // Prompt sem valor = info; sem prompt = vazio (Caps Lock não conta sem campo).
        assert_eq!(
            greeter_status(&open, "", false, &prompt(None), false, true, None),
            StatusLine::Info("Password: ")
        );
        assert_eq!(
            greeter_status(&open, "", false, &None, false, true, None),
            StatusLine::Empty
        );
    }

    #[test]
    fn login_fades_once_then_fires_start_session() {
        let t0 = Instant::now();
        let mut m = Motion::<u32>::new(t0);
        let mut seq = 0;
        let first = begin_exit(&mut m, &mut seq, t0).expect("o primeiro Login começa o fade");
        // ipc.rs pode mandar Login de novo: nada de segundo timer.
        assert_eq!(begin_exit(&mut m, &mut seq, t0 + Duration::from_millis(10)), None);
        assert!(exit_fires(&m, seq, first));
    }

    #[test]
    fn error_during_fade_restores_screen_and_drops_the_timer() {
        let t0 = Instant::now();
        let mut m = Motion::<u32>::new(t0);
        let mut seq = 0;
        let first = begin_exit(&mut m, &mut seq, t0).unwrap();
        m.tick(t0 + Duration::from_millis(90));
        assert!(m.look(Part::Field).alpha < 1.0, "no meio do fade");
        restore_on_error(&mut m, true, t0 + Duration::from_millis(90));
        assert!(!m.exiting());
        assert_eq!(m.look(Part::Field).alpha, 1.0, "a tela volta na hora");
        assert!(m.is_animating(), "senha enviada: balanço");
        // O timer antigo não manda StartSession.
        assert!(!exit_fires(&m, seq, first));
        // Novo Login depois do erro: novo fade; só o timer novo vale.
        let second = begin_exit(&mut m, &mut seq, t0 + Duration::from_millis(500)).unwrap();
        assert_ne!(first, second);
        assert!(!exit_fires(&m, seq, first));
        assert!(exit_fires(&m, seq, second));
    }

    #[test]
    fn error_without_password_sent_does_not_shake() {
        let t0 = Instant::now();
        let mut m = Motion::<u32>::new(t0);
        restore_on_error(&mut m, false, t0);
        assert!(!m.exiting());
        assert!(!m.is_animating());
        assert_eq!(m.look(Part::Field), crate::motion::Look::REST);
    }
}

#[cfg(test)]
mod login_wallpaper_tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cosmic_greeter_login_test_{}_{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("cria o diretório de teste");
        dir
    }

    #[test]
    fn login_initial_wallpaper_uses_time() {
        let dir = temp_dir("initial");
        let a = dir.join("a.png");
        let b = dir.join("b.png");
        for p in [&a, &b] {
            fs::write(p, b"img").expect("grava a imagem de teste");
        }
        let mut cfg = StyleConfig::default();
        cfg.wallpaper_mode = "rotate_on_lock".into();
        cfg.wallpaper_dir = dir.to_string_lossy().into_owned();
        cfg.wallpaper_shuffle = false;
        cfg.wallpaper_interval_mins = 10;

        let first = initial_wallpaper(&cfg, 0);
        let later = initial_wallpaper(&cfg, 600);
        assert_eq!(first, Some(a));
        assert_eq!(later, Some(b));
        assert_ne!(first, later);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn login_initial_wallpaper_fixed_ignores_time() {
        let dir = temp_dir("fixed");
        let one = dir.join("one.png");
        fs::write(&one, b"img").expect("grava a imagem de teste");
        let mut cfg = StyleConfig::default();
        cfg.wallpaper = one.to_string_lossy().into_owned();
        assert_eq!(initial_wallpaper(&cfg, 0), Some(one.clone()));
        assert_eq!(initial_wallpaper(&cfg, 99_999), Some(one));
        let _ = fs::remove_dir_all(&dir);
    }
}
