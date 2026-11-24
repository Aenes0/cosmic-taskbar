// SPDX-License-Identifier: GPL-3.0-only

//! Full native COSMIC Control Panel application (`cosmic-taskbar control-panel`):
//! centralizes power profiles, battery/USB-C PD telemetry, Lenovo conservation
//! mode, taskbar/tray ordering, left Monitor panel settings, right Central panel
//! settings, multi-camera & physical privacy switches, network/security details,
//! display DDC/visuals, isolated login screen (Greeter), optional packages, and
//! auto-updater/services.

mod chrome;
mod data;
mod feedback;
mod health;
mod metrics;
mod nav;
mod pending;
mod power_config;
mod screenshot_config;
mod search_index;
mod undo;
mod views;

use std::time::{Duration, Instant};

use cosmic::Application as _;

use feedback::{Recent, RecentChange, ToastKind, Toasts};
use health::{TimerTimes, UnitHealth};
use metrics::HomeMetrics;
use nav::{Category, category_of};
use pending::{Applied, ApplyAction};
use power_config::{CpuTopologyInfo, PowerProfileKind, PowerProfilesConfig, ProfileTuning};
use screenshot_config::{OpenMode as ScreenshotOpenMode, ScreenshotConfig};

use cosmic::{
    Element, Task, app,
    iced::{self, Subscription, event, window},
};

use crate::central::monitor::sample::Reading;
use crate::central::{
    audio::{self, Privacy},
    bar_spacing::BarSpacing,
    layout::{self, BarItem, Layout, PanelIcons, Section as CentralSection, Show},
    monitor::{
        self,
        config::{
            BarItem as MonitorBarItem, BarMode as MonitorBarMode, CardMode as MonitorCardMode,
            Panel as MonitorPanel, PollComponent as MonitorPollComponent,
            ProcSort as MonitorProcSort, Settings as MonitorSettings,
        },
        weather::Place,
    },
    notifications::config::{NotifConfig, NotifFormat, NotifPosition},
    services::{
        self, Battery, Camera, DARK_MODE, DO_NOT_DISTURB, Display, Network, Peripheral,
        PowerProfiles, TilingConfig, Touchpad, Update,
    },
    tiles::Tile,
    tray::config::TrayConfig,
};
use crate::logi::config as osd_config;

pub const APP_ID: &str = "io.github.Aenes0.CosmicControlPanel";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Home,
    Capture,
    Power,
    Bar,
    Monitor,
    Central,
    Notifications,
    DevicesOsd,
    Camera,
    Network,
    Security,
    Backup,
    Displays,
    Login,
    Optional,
    System,
}

impl Section {
    #[cfg(test)]
    pub const ALL: [Section; 16] = [
        Section::Home,
        Section::Power,
        Section::Bar,
        Section::Monitor,
        Section::Central,
        Section::Notifications,
        Section::DevicesOsd,
        Section::Camera,
        Section::Network,
        Section::Security,
        Section::Backup,
        Section::Displays,
        Section::Login,
        Section::Capture,
        Section::Optional,
        Section::System,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Section::Home => "Visão geral",
            Section::Capture => "Captura de tela",
            Section::Power => "Energia & Bateria",
            Section::Bar => "Barra & Tray",
            Section::Monitor => "Monitor (Esquerda)",
            Section::Central => "Central (Direita)",
            Section::Notifications => "Notificações",
            Section::DevicesOsd => "OSD de Dispositivos",
            Section::Camera => "Câmeras & Áudio",
            Section::Network => "Rede & VPN",
            Section::Security => "Segurança & Privacidade",
            Section::Backup => "Backup & Sincronização",
            Section::Displays => "Monitores & Janelas",
            Section::Login => "Tela de Login",
            Section::Optional => "Pacotes & Opcionais",
            Section::System => "Atualizações & Serviços",
        }
    }


    pub fn from_arg(arg: &str) -> Option<Section> {
        match arg.trim().to_ascii_lowercase().as_str() {
            "home" | "inicio" => Some(Section::Home),
            "capture" | "captura" | "print" | "screenshot" => Some(Section::Capture),
            "power" | "energia" => Some(Section::Power),
            "bar" | "barra" | "tray" => Some(Section::Bar),
            "monitor" => Some(Section::Monitor),
            "central" => Some(Section::Central),
            "notifications" | "notificacoes" => Some(Section::Notifications),
            "osd" | "devices" => Some(Section::DevicesOsd),
            "camera" | "audio" => Some(Section::Camera),
            "network" | "rede" => Some(Section::Network),
            "security" | "seguranca" => Some(Section::Security),
            "backup" => Some(Section::Backup),
            "displays" | "monitores" => Some(Section::Displays),
            "login" | "greeter" => Some(Section::Login),
            "optional" | "opcionais" => Some(Section::Optional),
            "system" | "sistema" => Some(Section::System),
            _ => None,
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Section::Home => "go-home-symbolic",
            Section::Capture => "applets-screenshooter-symbolic",
            Section::Power => "battery-good-symbolic",
            Section::Bar => "view-app-grid-symbolic",
            Section::Monitor => "utilities-system-monitor-symbolic",
            Section::Central => "view-list-symbolic",
            Section::Notifications => "preferences-system-notifications-symbolic",
            Section::DevicesOsd => "input-keyboard-symbolic",
            Section::Camera => "camera-web-symbolic",
            Section::Network => "network-wired-symbolic",
            Section::Security => "security-high-symbolic",
            Section::Backup => "drive-harddisk-symbolic",
            Section::Displays => "video-display-symbolic",
            Section::Login => "system-lock-screen-symbolic",
            Section::Optional => "system-software-install-symbolic",
            Section::System => "emblem-system-symbolic",
        }
    }
}

#[derive(Clone, Debug)]
pub enum Message {
    SelectSection(Section),
    SnapshotLoaded(data::SystemSnapshot),
    ServiceUpdate(Update),
    PrivacyUpdate(Privacy),
    CameraLoaded(Camera),
    DisplaysLoaded(Vec<Display>),
    BrightnessLoaded(Option<(i32, i32)>),
    SettingsChanged,
    // Power
    SetPowerProfile(String),
    ToggleConservationMode,
    ToggleShowPercentage,
    PowerEditProfile(PowerProfileKind),
    PowerToggleBoost,
    PowerCpuMaxMhz(u32),
    PowerEpp(String),
    PowerIntelMaxPerf(u32),
    PowerRaplPl1(u32),
    PowerRaplPl2(u32),
    PowerToggleDisableCcd1,
    PowerMaxOnlineCores(u32),
    PowerNvidiaPowerLimit(u32),
    PowerNvidiaGpuClock(u32),
    PowerNvidiaMemClock(u32),
    PowerToggleNvidiaPersistence,
    PowerIntelIgpuMaxMhz(u32),
    PowerDisplayHz(String),
    PowerScreenOffMins(u32),
    PowerLockMins(u32),
    PowerSuspendAcMins(u32),
    PowerSuspendBatteryMins(u32),
    PowerPcieAspm(String),
    PowerSataAlpm(String),
    PowerToggleAudioSave,
    PowerToggleWifiSave,
    /// Barra "Aplicar" de Energia: ativa o plano em edição e aplica no hardware.
    ApplyPower,
    PowerResetDefaults,
    // Bar & Tray
    SetBarSpacing(BarSpacing),
    SetBarIcon(BarItem, Show),
    DragBarStart(BarItem),
    DragBarOver(BarItem),
    DragBarEnd,
    ToggleTrayHidden(String),
    RestartPanel,
    // Monitor (Left panel)
    MonitorToggleBar(MonitorBarItem),
    MonitorBarMode(MonitorBarItem, MonitorBarMode),
    MonitorThermometer(MonitorBarMode),
    MonitorPollSecs(u64),
    MonitorCardMode(MonitorPanel, MonitorCardMode),
    MonitorWeatherMode(MonitorCardMode),
    MonitorDragStart(MonitorPanel),
    MonitorDragOver(MonitorPanel),
    MonitorDragEnd,
    MonitorComponentPoll(MonitorPollComponent, u64),
    MonitorProcsCount(usize),
    MonitorProcSort(MonitorProcSort),
    MonitorTariffInput(String),
    MonitorRestWattsInput(String),
    MonitorReset,
    WeatherQueryInput(String),
    WeatherSearch,
    WeatherPlacesFound(Vec<Place>),
    WeatherPickPlace(Place),
    // Central (Right panel)
    CentralToggleSectionFold(CentralSection),
    CentralToggleDarkMode,
    CentralToggleDnd,
    CentralToggleNewWorkspacesTile,
    CentralToggleActiveHint,
    CentralSetTouchpad(Touchpad),
    CentralToggleHotEdge,
    CentralOpenHotEdge,
    CentralResetTiles,
    // Notifications
    NotifTimeout(u32),
    NotifFormat(NotifFormat),
    NotifPosition(NotifPosition),
    NotifOpacity(f32),
    NotifCornerRadius(f32),
    NotifMaxBodyLines(u32),
    NotifToggleShowBody,
    NotifToggleShowActions,
    NotifToggleGroupByApp,
    NotifAccentHex(String),
    NotifBgHex(String),
    NotifReset,
    NotifSendTest,
    // Devices OSD
    OsdDurationMs(u64),
    OsdBatteryLow(u8),
    OsdBatteryCritical(u8),
    OsdBatteryIntervalMins(u64),
    OsdEmojiCommand(String),
    /// Barra "Aplicar" do OSD: reinicia o serviço com a configuração gravada.
    ApplyOsd,
    OsdReset,
    OsdRunDemo,
    // Camera
    ToggleMicrophone,
    ToggleCamera,
    RestartCamera,
    IriunWindow(bool),
    RefreshCamera,
    // Network
    ToggleWifi,
    ConnectVpn(usize),
    OpenCosmicSettings(Option<&'static str>),
    // Displays & Visuals
    SetPrimaryDisplay(String),
    SetDdcBrightness(usize, f32),
    ApplyDdcBrightness(usize),
    SetInternalBrightness(f32),
    RunWallpapers,
    RunBravePwaFix,
    RunScreenshot,
    OpenScreenshotsDir,
    ScreenshotSaveDirInput(String),
    ScreenshotToggleCopy,
    ScreenshotOpenMode(ScreenshotOpenMode),
    ScreenshotSattyTool(String),
    ScreenshotBorderColor(String),
    ScreenshotBorderWidth(u32),
    ScreenshotOverlayOpacity(u32),
    ScreenshotTimeoutSecs(u32),
    ScreenshotToggleSound,
    ScreenshotReset,
    RefreshDisplays,
    // Login (Greeter & Lockscreen)
    GreeterStyle(String),
    GreeterWallpaperInput(String),
    GreeterWallpaperMode(String),
    GreeterWallpaperDirInput(String),
    GreeterToggleShuffle,
    GreeterIntervalMins(u32),
    GreeterTheme(String),
    GreeterFontFamily(String),
    GreeterFontWeight(String),
    GreeterClockSize(String),
    GreeterToggleClock24h,
    GreeterToggleShowSeconds,
    GreeterToggleShowDate,
    GreeterToggleShowAvatar,
    GreeterToggleShowUsername,
    GreeterBlurRadius(u32),
    GreeterDimOpacity(f32),
    GreeterCardOpacity(f32),
    GreeterCardRadius(f32),
    GreeterAccentColor(String),
    GreeterDefaultUserInput(String),
    GreeterSessionInput(String),
    GreeterToggleIsolatedBinary,
    GreeterTestLock,
    GreeterSaveOnly,
    GreeterApply,
    // Optional packages & Security/Power/Backup actions
    OpenInTerminal(&'static str, String),
    RunAction(&'static str, String, bool),
    SpawnApp(&'static str, Vec<&'static str>, &'static str),
    EnableUserUnit(&'static str),
    // System & Updates
    CheckUpdates,
    RunUpdateNow,
    ExportConfig,
    RestartUserUnit(&'static str),
    RefreshAll,
    ActionDone(String),
    /// Fim de uma aplicação (Energia/OSD): como `ActionDone`, e marca o que foi aplicado se deu certo.
    ApplyDone(Applying, Result<String, String>),
    SetWindowGaps(u32, u32),
    SetActivationPolicy(&'static str),
    // Moldura (painel-b)
    SelectCategory(Category),
    OpenPalette,
    ClosePalette,
    PaletteQuery(String),
    PaletteMove(i32),
    PaletteActivate { run: bool },
    /// Clique numa linha da paleta: seleciona e ativa (ação executa, ajuste e página nativa abrem).
    PaletteClick(usize),
    OpenNative(&'static str),
    JumpTo(&'static str),
    ToggleAdvanced,
    DismissToast(u64),
    /// Desfaz a mudança recente `idx` (reenvia a `Message` guardada com o valor anterior).
    Undo(usize),
    /// Desfaz uma mudança de Energia no plano em que ela foi feita, mesmo que o plano em edição tenha mudado.
    UndoAsProfile(PowerProfileKind, Box<Message>),
    /// Um segundo passou: expira avisos (só enquanto houver aviso visível).
    Tick,
    /// Hora de ler as métricas da Visão geral (só com a Visão geral aberta).
    MetricsTick,
    MetricsRead(Box<Reading>),
    WindowFocus(bool),
    /// Saúde das unidades rastreadas e horários do timer do backup.
    HealthLoaded(Vec<(&'static str, UnitHealth)>, TimerTimes),
    None,
}

/// O que um `ApplyDone` aplicou, para marcar como "aplicado" só quando deu certo.
#[derive(Clone, Debug)]
pub enum Applying {
    Power(PowerProfileKind, Box<ProfileTuning>),
    Osd(Box<osd_config::Config>),
}

/// Estado da paleta Ctrl K (aberta quando `ControlPanel::palette` é `Some`).
#[derive(Clone, Debug, Default)]
pub struct PaletteState {
    pub query: String,
    /// Índice em `chrome::flat_results(&query)`.
    pub cursor: usize,
}

pub struct ControlPanel {
    core: app::Core,
    section: Section,
    snapshot: data::SystemSnapshot,
    network: Network,
    battery: Option<Battery>,
    peripherals: Vec<Peripheral>,
    power: Option<PowerProfiles>,
    microphone: Option<bool>,
    privacy: Privacy,
    camera: Camera,
    displays: Option<Vec<Display>>,
    brightness: Option<(i32, i32)>,
    primary_display: Option<String>,
    show_percentage: bool,
    bar_spacing: BarSpacing,
    panel_icons: PanelIcons,
    tray_config: TrayConfig,
    dragging_bar: Option<BarItem>,
    // Monitor (left panel)
    monitor_settings: MonitorSettings,
    dragging_monitor_card: Option<MonitorPanel>,
    weather_place: Place,
    weather_query: String,
    weather_results: Option<Vec<Place>>,
    weather_searching: bool,
    tariff_input: String,
    rest_watts_input: String,
    // Central (right panel)
    central_layout: Layout,
    dark_mode: bool,
    do_not_disturb: bool,
    tiling_config: TilingConfig,
    touchpad: Option<Touchpad>,
    hot_edge: bool,
    // Power profiles tuning
    power_config: PowerProfilesConfig,
    editing_profile: PowerProfileKind,
    cpu_topology: CpuTopologyInfo,
    // Notifications & Devices OSD & Screenshot
    notif_config: NotifConfig,
    osd_config: osd_config::Config,
    screenshot_config: ScreenshotConfig,
    // Login (Greeter)
    greeter: data::GreeterConfig,
    window_gaps: (u32, u32),
    activation_policy: String,
    /// Uma ação de fundo está rodando: desabilita os botões de ação da página e a barra Aplicar.
    busy: bool,
    /// Aviso `Progress` que mostra o `busy`, para tirá-lo quando a ação termina.
    busy_toast: Option<u64>,
    /// Reenviando a `Message` guardada de um "Desfazer": não vira nova mudança recente.
    undoing: bool,
    // Moldura (painel-b)
    toasts: Toasts<Message>,
    recent: Recent<Message>,
    applied: Applied,
    palette: Option<PaletteState>,
    metrics: Option<HomeMetrics>,
    last_reading: Option<Reading>,
    metrics_inflight: bool,
    focused: bool,
    /// Grupos avançados da seção atual visíveis (substitui `power_show_advanced`/`security_show_advanced`).
    advanced_open: bool,
    /// Último grupo escolhido em "Ir para" (destaca o atalho).
    scroll_to: Option<&'static str>,
    /// Saúde (carregada/ativa) das unidades rastreadas da seção Atualizações & Serviços.
    unit_health: Vec<(&'static str, UnitHealth)>,
    backup_times: TimerTimes,
}

/// Rajadas do mesmo ajuste dentro desta janela viram uma entrada só em "Alterados recentemente".
const RECENT_COALESCE_WINDOW: Duration = Duration::from_secs(3);
/// Espera entre abrir o avançado e rolar até um grupo dele.
const JUMP_AFTER_EXPAND: Duration = Duration::from_millis(80);
/// Cadência do `Tick` que expira avisos.
const TOAST_TICK: Duration = Duration::from_secs(1);

/// Tipo do aviso a partir do texto das mensagens antigas (`✓`/`✗`/`Falha`/`Erro`, e também
/// "Não foi possível…", que `spawn_app` devolve quando nem abriu).
fn legacy_kind(text: &str) -> ToastKind {
    let kind = feedback::kind_from_legacy(text);
    if kind == ToastKind::Info && text.trim_start().starts_with("Não foi possível") {
        ToastKind::Error
    } else {
        kind
    }
}

/// Texto da barra "Aplicar" de Energia.
fn apply_bar_label(pending: &ApplyAction, editing: PowerProfileKind) -> String {
    match pending {
        ApplyAction::ApplyHardware => "Aplicar no hardware".to_owned(),
        ApplyAction::ActivateAndApply(_) => format!("Ativar e aplicar {}", editing.label()),
    }
}

/// Indicador de salvamento do cabeçalho.
fn save_status(pending_count: usize) -> String {
    match pending_count {
        0 => "Tudo salvo".to_owned(),
        1 => "1 ajuste para aplicar".to_owned(),
        n => format!("{n} ajustes para aplicar"),
    }
}

/// Fecha um "Aplicar": só um sucesso marca o que foi aplicado (Energia ou OSD); uma falha deixa a
/// pendência acesa e vira aviso de erro. Devolve o tipo e o texto do aviso.
fn settle_apply(
    applied: &mut pending::Applied,
    what: &Applying,
    result: Result<String, String>,
) -> (ToastKind, String) {
    match result {
        Ok(msg) => {
            let text = match what {
                Applying::Power(kind, tuning) => {
                    applied.mark_power(*kind, tuning);
                    msg
                }
                Applying::Osd(cfg) => {
                    applied.mark_osd(cfg);
                    format!("Configuração do OSD aplicada. {msg}")
                }
            };
            (ToastKind::Success, text)
        }
        Err(msg) => (ToastKind::Error, msg),
    }
}

/// Cursor da paleta depois de `delta` passos numa lista de `len` itens (dá a volta nas pontas).
fn moved_cursor(cursor: usize, delta: i32, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    (cursor as i64 + i64::from(delta)).rem_euclid(len as i64) as usize
}

pub fn run() -> cosmic::iced::Result {
    crate::shared::logging::init("warn,cosmic_taskbar=info", &[]);
    let mut initial_section: Option<Section> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--section"
            && let Some(val) = args.next()
        {
            initial_section = Section::from_arg(&val);
        } else if let Some(val) = arg.strip_prefix("--section=") {
            initial_section = Section::from_arg(val);
        }
    }
    let settings = cosmic::app::Settings::default()
        .size(iced::Size::new(1280.0, 900.0))
        .size_limits(iced::Limits::NONE.min_width(780.0).min_height(540.0));
    cosmic::app::run::<ControlPanel>(settings, initial_section)
}

impl ControlPanel {
    pub(crate) fn reorderable_bar_items(&self) -> Vec<BarItem> {
        let mut items = vec![BarItem::Privacy, BarItem::Notifications];
        for tile in Tile::ALL {
            if tile != Tile::Wifi && tile != Tile::Power && tile.available() {
                items.push(BarItem::Tile(tile));
            }
        }
        items.push(BarItem::Network);
        items.push(BarItem::Volume);
        items.push(BarItem::Peripherals);
        if self.battery.is_some() {
            items.push(BarItem::Battery);
        }
        if self.power.is_some() {
            items.push(BarItem::Tile(Tile::Power));
        }
        items.push(BarItem::Clock);
        items
    }

    /// Troca de seção: recomeça no topo, com o avançado recolhido, e recarrega o que a seção lê do disco.
    fn select_section(&mut self, sec: Section) -> app::Task<Message> {
        self.section = sec;
        self.advanced_open = false;
        self.scroll_to = None;
        Task::batch([views::scroll_page_to(0.0), self.section_load_task(sec)])
    }

    fn section_load_task(&mut self, sec: Section) -> app::Task<Message> {
        if sec == Section::Home {
            // Leitura nova: dois pontos próximos, não a média desde a última visita.
            self.metrics = None;
            self.last_reading = None;
            return self.read_metrics();
        }
        if sec == Section::Displays && self.displays.is_none() {
            return Task::future(async {
                cosmic::action::app(Message::DisplaysLoaded(
                    services::displays(services::DdcRead::All).await,
                ))
            });
        }
        if sec == Section::Camera {
            return Task::future(async {
                cosmic::action::app(Message::CameraLoaded(services::camera().await))
            });
        }
        if sec == Section::Bar {
            self.panel_icons = PanelIcons::load();
            self.bar_spacing = BarSpacing::load();
            self.tray_config = data::load_tray_config();
        }
        if sec == Section::Monitor {
            self.monitor_settings = MonitorSettings::load();
            self.weather_place = monitor::config::weather_place();
        }
        if sec == Section::Central {
            self.central_layout = Layout::load();
            self.dark_mode = DARK_MODE.get();
            self.do_not_disturb = DO_NOT_DISTURB.get();
            self.tiling_config = services::tiling_config();
            self.touchpad = services::touchpad();
            self.hot_edge = services::hot_edge();
        }
        if sec == Section::Notifications {
            let prev_accent = self.notif_config.accent_hex.clone();
            let prev_bg = self.notif_config.bg_hex.clone();
            self.notif_config = NotifConfig::load();
            if !prev_accent.is_empty()
                && crate::central::notifications::config::parse_hex_color(&prev_accent).is_none()
            {
                self.notif_config.accent_hex = prev_accent;
            }
            if !prev_bg.is_empty()
                && crate::central::notifications::config::parse_hex_color(&prev_bg).is_none()
            {
                self.notif_config.bg_hex = prev_bg;
            }
        }
        if sec == Section::DevicesOsd {
            self.osd_config = osd_config::load_or_default();
        }
        if sec == Section::Login {
            self.greeter = data::load_greeter_config();
        }
        if sec == Section::Optional {
            return Task::future(async {
                cosmic::action::app(Message::SnapshotLoaded(data::load_snapshot().await))
            });
        }
        Task::none()
    }

    /// Registra em "Alterados recentemente" a mudança simples de valor que `msg` vai fazer
    /// (antes de ela ser aplicada, para guardar o valor anterior no "Desfazer").
    fn record_recent(&mut self, msg: &Message) {
        let Some(d) = undo::describe(self, msg) else {
            return;
        };
        let change = RecentChange {
            label: format!("{} → {}", d.label, d.value),
            section: d.section,
            at: Instant::now(),
            undo: Some(d.undo),
        };
        let items = undo::coalesce(
            self.recent.items().to_vec(),
            change,
            d.coalesce,
            RECENT_COALESCE_WINDOW,
        );
        self.recent = Recent::default();
        for c in items.into_iter().rev() {
            self.recent.record(c);
        }
    }

    /// Enter / Ctrl+Enter na paleta: ajuste → seção, avançado e grupo; ação → executa (Ctrl+Enter
    /// com mapeamento simples) ou vai à seção; página nativa → `cosmic-settings <página>`.
    fn activate_palette(&mut self, run: bool) -> app::Task<Message> {
        let Some(p) = self.palette.as_ref() else {
            return Task::none();
        };
        let results = chrome::flat_results(&p.query);
        let Some(entry) = results.get(p.cursor).copied() else {
            return Task::none();
        };
        self.palette = None;
        match entry.kind {
            search_index::EntryKind::Native => match entry.native_page {
                Some(page) => self.update(Message::OpenNative(page)),
                None => Task::none(),
            },
            search_index::EntryKind::Action => match chrome::action_message(entry).filter(|_| run) {
                Some(action) => self.update(action),
                None => self.go_to(entry.section, false, None),
            },
            search_index::EntryKind::Setting => {
                self.go_to(entry.section, entry.advanced, Some(entry.group))
            }
        }
    }

    /// Vai à seção, abre o avançado se preciso e rola até o grupo.
    fn go_to(&mut self, section: Section, advanced: bool, group: Option<&'static str>) -> app::Task<Message> {
        let load = self.section_load_task(section);
        self.section = section;
        self.advanced_open = advanced;
        self.scroll_to = None;
        let scroll = match group {
            Some(g) => Task::done(cosmic::action::app(Message::JumpTo(g))),
            None => views::scroll_page_to(0.0),
        };
        Task::batch([load, scroll])
    }

    /// "Ir para": rola o scrollable da página até o grupo. O iced não informa onde um bloco está
    /// dentro do scrollable, então a posição é aproximada: `índice do grupo / total de grupos`.
    fn jump_to(&mut self, group: &'static str) -> app::Task<Message> {
        if !self.advanced_open && chrome::is_advanced_group(self.section, group) {
            // Primeiro abre o avançado; o salto espera a página ganhar o conteúdo novo.
            self.advanced_open = true;
            return Task::future(async move {
                tokio::time::sleep(JUMP_AFTER_EXPAND).await;
                cosmic::action::app(Message::JumpTo(group))
            });
        }
        self.scroll_to = Some(group);
        let groups = chrome::section_groups(self.section);
        match chrome::jump_fraction(&groups, group, self.advanced_open) {
            Some(fraction) => views::scroll_page_to(fraction),
            None => Task::none(),
        }
    }

    /// Aviso tipado no canto da janela.
    fn toast(&mut self, kind: ToastKind, text: impl Into<String>) {
        self.toasts.push(kind, text, None, Instant::now());
    }

    /// Aviso cujo tipo vem do texto (`✓`/`✗`/`Falha`/`Erro`); o `✓`/`✗` sai, o ícone já diz.
    fn toast_legacy(&mut self, text: impl Into<String>) {
        let text = text.into();
        let kind = legacy_kind(&text);
        let shown = text
            .trim_start()
            .strip_prefix(['✓', '✗'])
            .map_or(text.as_str(), str::trim_start)
            .to_owned();
        self.toast(kind, shown);
    }

    /// Começa uma ação de fundo: `busy` + aviso `Progress`.
    fn begin_busy(&mut self, text: impl Into<String>) {
        if let Some(id) = self.busy_toast.take() {
            self.toasts.dismiss(id);
        }
        self.busy = true;
        let id = self.toasts.push(ToastKind::Progress, text, None, Instant::now());
        self.busy_toast = Some(id);
    }

    /// Termina a ação de fundo: tira o `busy` e o aviso `Progress`.
    fn end_busy(&mut self) {
        self.busy = false;
        if let Some(id) = self.busy_toast.take() {
            self.toasts.dismiss(id);
        }
    }

    fn active_profile_kind(&self) -> Option<PowerProfileKind> {
        self.power
            .as_ref()
            .and_then(|p| PowerProfileKind::from_id(&p.active))
    }

    /// O que o botão "Aplicar" da página Energia faria agora (`None` = nada pendente). Sem daemon
    /// (plano ativo desconhecido) compara com a marca do plano em edição e oferece aplicar no
    /// hardware; a oferta de ativar outro plano só existe aqui, na barra da página.
    pub(crate) fn power_apply(&self) -> Option<ApplyAction> {
        pending::power_pending(
            &self.power_config,
            &self.applied,
            self.editing_profile,
            self.active_profile_kind(),
        )
    }

    /// O plano em edição tem ajuste de hardware gravado e ainda não aplicado (contagem do
    /// cabeçalho, ponto do trilho e alerta do Início; olhar outro plano não conta).
    pub(crate) fn power_dirty(&self) -> bool {
        pending::power_dirty(&self.power_config, &self.applied, self.editing_profile)
    }

    /// O OSD gravado difere do que estava no último reinício do serviço.
    pub(crate) fn osd_apply(&self) -> bool {
        pending::osd_pending(&self.osd_config, &self.applied)
    }

    /// Quantos itens esperam "Aplicar" (Energia e OSD).
    pub(crate) fn pending_count(&self) -> usize {
        usize::from(self.power_dirty()) + usize::from(self.osd_apply())
    }

    /// Ponto âmbar do trilho para a categoria.
    pub(crate) fn category_alert(&self, cat: Category) -> bool {
        chrome::category_alert(
            cat,
            &self.unit_health,
            self.power_dirty(),
            self.osd_apply(),
        )
    }

    /// Lê saúde das unidades rastreadas e horários do backup (somente leitura).
    fn load_health(&self) -> app::Task<Message> {
        let units: Vec<(&'static str, bool)> = self
            .snapshot
            .units
            .iter()
            .map(|u| (u.unit, u.is_system))
            .collect();
        Task::future(async move {
            let mut out = Vec::with_capacity(units.len());
            for (unit, system) in units {
                out.push((unit, health::unit_health(unit, system).await));
            }
            let backup = health::backup_timer().await;
            cosmic::action::app(Message::HealthLoaded(out, backup))
        })
    }

    /// Uma leitura das métricas fora da thread da UI.
    fn read_metrics(&mut self) -> app::Task<Message> {
        if self.metrics_inflight {
            return Task::none();
        }
        self.metrics_inflight = true;
        Task::future(async {
            let reading = tokio::task::spawn_blocking(metrics::read_now).await;
            match reading {
                Ok(r) => cosmic::action::app(Message::MetricsRead(Box::new(r))),
                Err(_) => cosmic::action::app(Message::None),
            }
        })
    }

    /// Termina uma ação (`ActionDone`/`ApplyDone`): aviso com o resultado e recarga do estado.
    fn finish_action(&mut self, msg: String) -> app::Task<Message> {
        self.end_busy();
        self.toast_legacy(msg);
        self.after_action()
    }

    /// Como `finish_action`, mas com o tipo do aviso já decidido (resultado de `Result`).
    fn finish_action_as(&mut self, kind: ToastKind, text: String) -> app::Task<Message> {
        self.end_busy();
        self.toast(kind, text);
        self.after_action()
    }

    /// Recarga do estado depois de uma ação.
    fn after_action(&mut self) -> app::Task<Message> {
        self.cpu_topology = power_config::detect_cpu_topology();
        Task::future(async {
            cosmic::action::app(Message::SnapshotLoaded(data::load_snapshot().await))
        })
    }

    fn persist_power_config(&mut self, sync_idle: bool) {
        if let Err(err) = self.power_config.save() {
            self.toast(ToastKind::Error, format!("Falha ao salvar perfis de energia: {err}"));
            return;
        }
        if sync_idle
            && self
                .power
                .as_ref()
                .is_some_and(|p| p.active == self.editing_profile.id())
        {
            let tuning = self.power_config.get(self.editing_profile);
            if let Err(err) = power_config::apply_idle_timeouts(tuning) {
                self.toast(ToastKind::Error, format!("Falha ao aplicar tempos de tela/login: {err}"));
            }
        }
    }

    fn persist_osd_config(&mut self) {
        if let Err(err) = self.osd_config.normalized().save() {
            self.toast(ToastKind::Error, err);
        }
    }

    fn persist_greeter_config(&mut self) {
        if let Err(err) = data::save_greeter_config(&self.greeter) {
            self.toast(ToastKind::Error, err);
        }
    }

    fn persist_screenshot_config(&mut self) {
        if let Err(err) = self.screenshot_config.save() {
            self.toast(ToastKind::Error, format!("Falha ao salvar config de captura: {err}"));
        }
    }

    fn reload_all(&self) -> app::Task<Message> {
        Task::batch([
            Task::future(async {
                cosmic::action::app(Message::SnapshotLoaded(data::load_snapshot().await))
            }),
            Task::future(async {
                cosmic::action::app(Message::CameraLoaded(services::camera().await))
            }),
            Task::future(async {
                cosmic::action::app(Message::DisplaysLoaded(
                    services::displays(services::DdcRead::Remembered).await,
                ))
            }),
            Task::future(async {
                cosmic::action::app(Message::BrightnessLoaded(services::read_brightness().await))
            }),
        ])
    }
}

impl cosmic::Application for ControlPanel {
    type Executor = cosmic::SingleThreadExecutor;
    type Message = Message;
    type Flags = Option<Section>;
    const APP_ID: &'static str = APP_ID;

    fn init(mut core: app::Core, initial_section: Option<Section>) -> (Self, app::Task<Message>) {
        core.window.show_headerbar = true;
        let monitor_settings = MonitorSettings::load();
        let tariff_input = monitor_settings
            .tariff
            .map(|t| monitor::decimal(t, 2))
            .unwrap_or_default();
        let rest_watts_input = if monitor_settings.rest_watts > 0.0 {
            monitor::decimal(monitor_settings.rest_watts, 0)
        } else {
            String::new()
        };
        let hardware = data::read_hardware();
        let power_config = PowerProfilesConfig::load(hardware.is_intel, hardware.is_laptop);
        let cpu_topology = power_config::detect_cpu_topology();
        let osd_config = osd_config::load_or_default();
        // O que está em disco ao abrir é a linha de base do "aplicado" (o plano ativo só é
        // conhecido depois; ao chegar, `Update::PowerProfiles` refaz a marca do ativo).
        let mut applied = Applied::default();
        for kind in PowerProfileKind::ALL {
            applied.mark_power(kind, power_config.get(kind));
        }
        applied.mark_osd(&osd_config);
        let mut app = Self {
            core,
            section: initial_section.unwrap_or(Section::Home),
            snapshot: data::SystemSnapshot {
                hardware,
                ..data::SystemSnapshot::default()
            },
            network: Network::default(),
            battery: None,
            peripherals: Vec::new(),
            power: None,
            microphone: None,
            privacy: Privacy::default(),
            camera: Camera::default(),
            displays: None,
            brightness: services::remembered_brightness(),
            primary_display: layout::primary_display(),
            show_percentage: services::SHOW_PERCENTAGE.get(),
            bar_spacing: BarSpacing::load(),
            panel_icons: PanelIcons::load(),
            tray_config: data::load_tray_config(),
            dragging_bar: None,
            monitor_settings,
            dragging_monitor_card: None,
            weather_place: monitor::config::weather_place(),
            weather_query: String::new(),
            weather_results: None,
            weather_searching: false,
            tariff_input,
            rest_watts_input,
            central_layout: Layout::load(),
            dark_mode: DARK_MODE.get(),
            do_not_disturb: DO_NOT_DISTURB.get(),
            tiling_config: services::tiling_config(),
            touchpad: services::touchpad(),
            hot_edge: services::hot_edge(),
            power_config,
            editing_profile: PowerProfileKind::Balanced,
            cpu_topology,
            notif_config: NotifConfig::load(),
            osd_config,
            screenshot_config: ScreenshotConfig::load(),
            greeter: data::load_greeter_config(),
            window_gaps: services::window_gaps(),
            activation_policy: services::activation_policy(),
            busy: false,
            busy_toast: None,
            undoing: false,
            toasts: Toasts::default(),
            recent: Recent::default(),
            applied,
            palette: None,
            metrics: None,
            last_reading: None,
            metrics_inflight: false,
            focused: true,
            advanced_open: false,
            scroll_to: None,
            unit_health: Vec::new(),
            backup_times: TimerTimes::default(),
        };
        let mut tasks = vec![app.reload_all()];
        if app.section == Section::Home {
            tasks.push(app.read_metrics());
        }
        // TEMP-PRINT: remover antes da entrega.
        if let Some(f) = std::env::var("CP_PRINT_SCROLL").ok().and_then(|v| v.parse::<f32>().ok()) {
            tasks.push(
                Task::future(async { tokio::time::sleep(std::time::Duration::from_millis(2800)).await })
                    .discard()
                    .chain(views::scroll_page_to(f)),
            );
        }
        if let Ok(q) = std::env::var("CP_PRINT_PAL") {
            tasks.push(
                Task::future(async { tokio::time::sleep(std::time::Duration::from_millis(1500)).await })
                    .discard()
                    .chain(Task::done(cosmic::action::app(Message::OpenPalette)))
                    .chain(Task::done(cosmic::action::app(Message::PaletteQuery(q)))),
            );
        }
        if std::env::var("CP_PRINT_ADV").is_ok() {
            app.advanced_open = true;
        }
        (app, Task::batch(tasks))
    }

    fn core(&self) -> &app::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut app::Core {
        &mut self.core
    }

    fn header_start(&self) -> Vec<Element<'_, Message>> {
        views::header_start(self)
    }

    fn header_center(&self) -> Vec<Element<'_, Message>> {
        views::header_center(self)
    }

    fn header_end(&self) -> Vec<Element<'_, Message>> {
        views::header_end(self)
    }

    fn update(&mut self, message: Message) -> app::Task<Message> {
        if !self.undoing {
            self.record_recent(&message);
        }
        match message {
            Message::SelectSection(sec) => {
                return self.select_section(sec);
            }
            Message::SelectCategory(cat) => {
                let target = if category_of(self.section) == cat {
                    self.section
                } else {
                    cat.sections().first().copied().unwrap_or(Section::Home)
                };
                return self.select_section(target);
            }
            Message::SnapshotLoaded(snapshot) => {
                self.snapshot = snapshot;
                self.tray_config = data::load_tray_config();
                return self.load_health();
            }
            Message::HealthLoaded(units, backup) => {
                self.unit_health = units;
                self.backup_times = backup;
            }
            Message::ServiceUpdate(update) => match update {
                Update::Network(net) => self.network = net,
                Update::Battery(bat) => self.battery = bat,
                Update::Peripherals(p) => self.peripherals = p,
                Update::PowerProfiles(p) => {
                    let old_active = self.active_profile_kind();
                    let new_active = p
                        .as_ref()
                        .and_then(|pp| PowerProfileKind::from_id(&pp.active));
                    if self.power.is_none()
                        && let Some(active) = new_active
                    {
                        self.editing_profile = active;
                    }
                    self.power = p;
                    // Troca de plano (aqui ou pelo Central): o daemon aplica o plano novo, então o
                    // que está gravado dele passa a ser o "aplicado". Um sinal que só repete o
                    // plano ativo (outra propriedade mudou) não apaga uma pendência real.
                    if new_active != old_active
                        && let Some(kind) = new_active
                    {
                        self.applied.forget_power(kind);
                        self.applied.mark_power(kind, self.power_config.get(kind));
                    }
                }
                Update::Microphone(m) => self.microphone = m,
                Update::Brightness(b) => self.brightness = b,
                _ => {}
            },
            Message::PrivacyUpdate(privacy) => self.privacy = privacy,
            Message::CameraLoaded(camera) => self.camera = camera,
            Message::DisplaysLoaded(displays) => self.displays = Some(displays),
            Message::BrightnessLoaded(brightness) => {
                if brightness.is_some() {
                    self.brightness = brightness;
                }
            }
            Message::SettingsChanged => {
                self.panel_icons = PanelIcons::load();
                self.bar_spacing = BarSpacing::load();
                self.primary_display = layout::primary_display();
                self.show_percentage = services::SHOW_PERCENTAGE.get();
                self.tray_config = data::load_tray_config();
                self.central_layout = Layout::load();
                self.dark_mode = DARK_MODE.get();
                self.do_not_disturb = DO_NOT_DISTURB.get();
                self.tiling_config = services::tiling_config();
                self.touchpad = services::touchpad();
                self.hot_edge = services::hot_edge();
                self.monitor_settings = MonitorSettings::load();
                self.weather_place = monitor::config::weather_place();
                let prev_accent = self.notif_config.accent_hex.clone();
                let prev_bg = self.notif_config.bg_hex.clone();
                self.notif_config = NotifConfig::load();
                if !prev_accent.is_empty()
                    && crate::central::notifications::config::parse_hex_color(&prev_accent).is_none()
                {
                    self.notif_config.accent_hex = prev_accent;
                }
                if !prev_bg.is_empty()
                    && crate::central::notifications::config::parse_hex_color(&prev_bg).is_none()
                {
                    self.notif_config.bg_hex = prev_bg;
                }
            }
            // Power
            Message::SetPowerProfile(profile) => {
                // A marca de "aplicado" só muda em `Update::PowerProfiles`, quando o daemon confirma
                // que o plano ativo mudou: clicar no plano que já é o ativo não aplica nada.
                self.begin_busy(format!("Alterando plano de energia para {profile}..."));
                return Task::future(async move {
                    let msg = match services::set_power_profile(profile.clone()).await {
                        Ok(()) => format!("✓ Plano de energia alterado para {profile}."),
                        Err(err) => {
                            let has_offline = std::fs::read_to_string("/sys/devices/system/cpu/offline")
                                .is_ok_and(|s| !s.trim().is_empty());
                            if has_offline {
                                let online_cmd = "for c in /sys/devices/system/cpu/cpu[1-9]*/online; do [ -w \"$c\" ] && echo 1 > \"$c\" 2>/dev/null || true; done";
                                let _ = tokio::process::Command::new("pkexec")
                                    .args(["sh", "-c", online_cmd])
                                    .status()
                                    .await;
                                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                                match services::set_power_profile(profile.clone()).await {
                                    Ok(()) => format!("✓ CPUs reativadas e plano alterado para {profile}."),
                                    Err(err2) => format!("✗ Falha ao alterar plano: {err2}"),
                                }
                            } else {
                                format!("✗ Falha ao alterar plano: {err}")
                            }
                        }
                    };
                    cosmic::action::app(Message::ActionDone(msg))
                });
            }
            Message::ToggleConservationMode => {
                if let Some(bat) = self.battery.as_mut()
                    && let Some(cur) = bat.conservation_mode
                {
                    let next = !cur;
                    bat.conservation_mode = Some(next);
                    return Task::future(async move {
                        let msg = match services::set_conservation_mode(next).await {
                            Ok(()) => {
                                if next {
                                    "Modo conservação (80%) ativado.".to_owned()
                                } else {
                                    "Modo conservação (80%) desativado.".to_owned()
                                }
                            }
                            Err(err) => format!("Não foi possível alterar conservação: {err}"),
                        };
                        cosmic::action::app(Message::ActionDone(msg))
                    });
                }
            }
            Message::ToggleShowPercentage => {
                self.show_percentage = !self.show_percentage;
                services::SHOW_PERCENTAGE.set(self.show_percentage);
            }
            Message::PowerEditProfile(kind) => {
                self.editing_profile = kind;
            }
            Message::PowerToggleBoost => {
                let t = self.power_config.get_mut(self.editing_profile);
                t.boost = !t.boost;
                self.persist_power_config(false);
            }
            Message::PowerCpuMaxMhz(mhz) => {
                self.power_config.get_mut(self.editing_profile).cpu_max_mhz = mhz;
                self.persist_power_config(false);
            }
            Message::PowerEpp(epp) => {
                self.power_config.get_mut(self.editing_profile).epp = epp;
                self.persist_power_config(false);
            }
            Message::PowerIntelMaxPerf(pct) => {
                self.power_config.get_mut(self.editing_profile).intel_max_perf_pct =
                    pct.clamp(10, 100);
                self.persist_power_config(false);
            }
            Message::PowerRaplPl1(w) => {
                self.power_config.get_mut(self.editing_profile).rapl_pl1_w = w;
                self.persist_power_config(false);
            }
            Message::PowerRaplPl2(w) => {
                self.power_config.get_mut(self.editing_profile).rapl_pl2_w = w;
                self.persist_power_config(false);
            }
            Message::PowerToggleDisableCcd1 => {
                let t = self.power_config.get_mut(self.editing_profile);
                t.disable_ccd1 = !t.disable_ccd1;
                self.persist_power_config(false);
            }
            Message::PowerMaxOnlineCores(cores) => {
                self.power_config.get_mut(self.editing_profile).max_online_cores = cores;
                self.persist_power_config(false);
            }
            Message::PowerNvidiaPowerLimit(w) => {
                self.power_config.get_mut(self.editing_profile).nvidia_power_limit_w = w;
                self.persist_power_config(false);
            }
            Message::PowerNvidiaGpuClock(mhz) => {
                self.power_config.get_mut(self.editing_profile).nvidia_gpu_clock_mhz = mhz;
                self.persist_power_config(false);
            }
            Message::PowerNvidiaMemClock(mhz) => {
                self.power_config.get_mut(self.editing_profile).nvidia_mem_clock_mhz = mhz;
                self.persist_power_config(false);
            }
            Message::PowerToggleNvidiaPersistence => {
                let t = self.power_config.get_mut(self.editing_profile);
                t.nvidia_persistence = !t.nvidia_persistence;
                self.persist_power_config(false);
            }
            Message::PowerIntelIgpuMaxMhz(mhz) => {
                self.power_config.get_mut(self.editing_profile).intel_igpu_max_mhz = mhz;
                self.persist_power_config(false);
            }
            Message::PowerDisplayHz(hz) => {
                self.power_config.get_mut(self.editing_profile).display_hz = hz;
                self.persist_power_config(false);
            }
            Message::PowerScreenOffMins(mins) => {
                self.power_config.get_mut(self.editing_profile).screen_off_mins = mins;
                self.persist_power_config(true);
            }
            Message::PowerLockMins(mins) => {
                self.power_config.get_mut(self.editing_profile).lock_mins = mins;
                self.persist_power_config(true);
            }
            Message::PowerSuspendAcMins(mins) => {
                self.power_config.get_mut(self.editing_profile).suspend_ac_mins = mins;
                self.persist_power_config(true);
            }
            Message::PowerSuspendBatteryMins(mins) => {
                self.power_config.get_mut(self.editing_profile).suspend_battery_mins = mins;
                self.persist_power_config(true);
            }
            Message::PowerPcieAspm(aspm) => {
                self.power_config.get_mut(self.editing_profile).pcie_aspm = aspm;
                self.persist_power_config(false);
            }
            Message::PowerSataAlpm(alpm) => {
                self.power_config.get_mut(self.editing_profile).sata_alpm = alpm;
                self.persist_power_config(false);
            }
            Message::PowerToggleAudioSave => {
                let t = self.power_config.get_mut(self.editing_profile);
                t.audio_power_save = !t.audio_power_save;
                self.persist_power_config(false);
            }
            Message::PowerToggleWifiSave => {
                let t = self.power_config.get_mut(self.editing_profile);
                t.wifi_power_save = !t.wifi_power_save;
                self.persist_power_config(false);
            }
            // Cada mudança de Energia já grava em `persist_power_config`; "Aplicar" só ativa o plano
            // em edição e o aplica no hardware (mesmo caminho do antigo "Salvar e aplicar plano").
            Message::ApplyPower => {
                if self.busy {
                    return Task::none();
                }
                let kind = self.editing_profile;
                let profile_id = kind.id().to_owned();
                let tuning = self.power_config.get(kind).clone();
                let applying = Applying::Power(kind, Box::new(tuning.clone()));
                self.begin_busy(format!("Aplicando plano {}...", kind.label()));
                return Task::future(async move {
                    _ = services::set_power_profile(profile_id.clone()).await;
                    let result = power_config::apply_profile_now(&profile_id, &tuning).await;
                    cosmic::action::app(Message::ApplyDone(applying, result))
                });
            }
            Message::PowerResetDefaults => {
                let hw = &self.snapshot.hardware;
                self.power_config = PowerProfilesConfig::defaults_for(hw.is_intel, hw.is_laptop);
                match self.power_config.save() {
                    Ok(()) => {
                        self.toast(
                            ToastKind::Success,
                            "Padrões do hardware restaurados e salvos. Use \"Aplicar\" para aplicar agora.",
                        );
                    }
                    Err(err) => {
                        self.toast(
                            ToastKind::Error,
                            format!("Padrões restaurados, mas falhou ao salvar: {err}"),
                        );
                    }
                }
            }
            // Bar & Tray
            Message::SetBarSpacing(spacing) => {
                self.bar_spacing = spacing;
                spacing.save();
            }
            Message::SetBarIcon(item, show) => {
                self.panel_icons.set(item, show);
            }
            Message::DragBarStart(item) => {
                self.dragging_bar = Some(item);
            }
            Message::DragBarOver(target) => {
                if let Some(from) = self.dragging_bar
                    && from != target
                {
                    let items = self.reorderable_bar_items();
                    self.panel_icons.move_to(from, target, &items);
                }
            }
            Message::DragBarEnd => {
                self.dragging_bar = None;
            }
            Message::ToggleTrayHidden(key) => {
                let mut hidden = self.tray_config.hidden.clone();
                if hidden.iter().any(|k| k == &key) {
                    hidden.retain(|k| k != &key);
                } else {
                    hidden.push(key);
                }
                self.tray_config.hidden = hidden.clone();
                data::save_tray_hidden(hidden);
            }
            Message::RestartPanel => {
                self.begin_busy("Reiniciando o painel...");
                return Task::future(async {
                    let msg =
                        data::run_user_tool("reiniciar-painel", &[], "Painel reiniciado.").await;
                    cosmic::action::app(Message::ActionDone(msg))
                });
            }
            // Monitor (Left panel)
            Message::MonitorToggleBar(item) => {
                self.monitor_settings.toggle_bar(item);
            }
            Message::MonitorBarMode(item, mode) => {
                self.monitor_settings.set_bar_mode(item, mode);
            }
            Message::MonitorThermometer(mode) => {
                self.monitor_settings.set_thermometer(mode);
            }
            Message::MonitorPollSecs(secs) => {
                self.monitor_settings.set_poll_secs(secs);
            }
            Message::MonitorCardMode(panel, mode) => {
                self.monitor_settings.set_card_mode(panel, mode);
            }
            Message::MonitorWeatherMode(mode) => {
                self.monitor_settings.set_weather_mode(mode);
            }
            Message::MonitorDragStart(panel) => {
                self.dragging_monitor_card = Some(panel);
            }
            Message::MonitorDragOver(target) => {
                if let Some(from) = self.dragging_monitor_card
                    && from != target
                {
                    self.monitor_settings.move_card_to(from, target);
                }
            }
            Message::MonitorDragEnd => {
                self.dragging_monitor_card = None;
            }
            Message::MonitorComponentPoll(comp, val) => {
                self.monitor_settings.set_component_poll(comp, val);
            }
            Message::MonitorProcsCount(count) => {
                self.monitor_settings.set_procs_count(count);
            }
            Message::MonitorProcSort(sort) => {
                self.monitor_settings.click_proc_sort(sort);
            }
            Message::MonitorTariffInput(text) => {
                let value = text.trim().replace(',', ".").parse::<f64>().ok();
                self.monitor_settings.set_tariff(value);
                self.tariff_input = text;
            }
            Message::MonitorRestWattsInput(text) => {
                let value = text.trim().replace(',', ".").parse::<f64>().unwrap_or(0.0);
                self.monitor_settings.set_rest_watts(value);
                self.rest_watts_input = text;
            }
            Message::MonitorReset => {
                self.monitor_settings.reset();
                self.toast(ToastKind::Success, "Layout do Monitor restaurado para o padrão.");
            }
            Message::WeatherQueryInput(query) => {
                self.weather_query = query;
            }
            Message::WeatherSearch => {
                let query = self.weather_query.trim().to_owned();
                if query.is_empty() || self.weather_searching {
                    return Task::none();
                }
                self.weather_searching = true;
                return Task::future(async move {
                    let places = monitor::weather::place::search(query).await;
                    cosmic::action::app(Message::WeatherPlacesFound(places))
                });
            }
            Message::WeatherPlacesFound(places) => {
                self.weather_searching = false;
                self.weather_results = Some(places);
            }
            Message::WeatherPickPlace(place) => {
                monitor::config::set_weather_place(&place);
                let label = if place.detail.is_empty() {
                    place.name.clone()
                } else {
                    format!("{} ({})", place.name, place.detail)
                };
                self.weather_place = place;
                self.weather_query.clear();
                self.weather_results = None;
                self.toast(ToastKind::Success, format!("Local do clima definido para {label}."));
            }
            // Central (Right panel)
            Message::CentralToggleSectionFold(sec) => {
                self.central_layout.toggle_fold(sec);
            }
            Message::CentralToggleDarkMode => {
                self.dark_mode = !self.dark_mode;
                DARK_MODE.set(self.dark_mode);
            }
            Message::CentralToggleDnd => {
                self.do_not_disturb = !self.do_not_disturb;
                DO_NOT_DISTURB.set(self.do_not_disturb);
                if !self.do_not_disturb {
                    services::set_dnd_until(None);
                }
            }
            Message::CentralToggleNewWorkspacesTile => {
                let next = !self.tiling_config.new_workspaces_tile;
                self.tiling_config.new_workspaces_tile = next;
                services::set_tiling_config("autotile", next);
            }
            Message::CentralToggleActiveHint => {
                let next = !self.tiling_config.active_hint;
                self.tiling_config.active_hint = next;
                services::set_tiling_config("active_hint", next);
            }
            Message::CentralSetTouchpad(mode) => {
                self.touchpad = Some(mode);
                services::set_touchpad_mode(mode);
            }
            Message::CentralToggleHotEdge => {
                self.hot_edge = !self.hot_edge;
                services::set_hot_edge(self.hot_edge);
            }
            Message::CentralOpenHotEdge => {
                self.toast_legacy(data::spawn_app("cosmic-ext-hot-edge", &[], "Hot Edge"));
            }
            Message::CentralResetTiles => {
                data::reset_central_tiles();
                self.toast(
                    ToastKind::Success,
                    "Grade de tiles da Central restaurada para o padrão.",
                );
            }
            // Notifications
            Message::NotifTimeout(secs) => {
                self.notif_config.timeout_secs = secs;
                if let Err(err) = self.notif_config.save() {
                    self.toast(ToastKind::Error, err);
                }
            }
            Message::NotifFormat(fmt) => {
                self.notif_config.format = fmt;
                if let Err(err) = self.notif_config.save() {
                    self.toast(ToastKind::Error, err);
                }
            }
            Message::NotifPosition(pos) => {
                self.notif_config.position = pos;
                if let Err(err) = self.notif_config.save() {
                    self.toast(ToastKind::Error, err);
                }
            }
            Message::NotifOpacity(val) => {
                self.notif_config.opacity = ((val * 100.0).round() / 100.0).clamp(
                    crate::central::notifications::config::MIN_OPACITY,
                    crate::central::notifications::config::MAX_OPACITY,
                );
                if let Err(err) = self.notif_config.save() {
                    self.toast(ToastKind::Error, err);
                }
            }
            Message::NotifCornerRadius(r) => {
                self.notif_config.corner_radius = r.round().clamp(
                    crate::central::notifications::config::MIN_CORNER_RADIUS,
                    crate::central::notifications::config::MAX_CORNER_RADIUS,
                );
                if let Err(err) = self.notif_config.save() {
                    self.toast(ToastKind::Error, err);
                }
            }
            Message::NotifMaxBodyLines(lines) => {
                self.notif_config.max_body_lines = lines.clamp(
                    crate::central::notifications::config::MIN_BODY_LINES,
                    crate::central::notifications::config::MAX_BODY_LINES,
                );
                if let Err(err) = self.notif_config.save() {
                    self.toast(ToastKind::Error, err);
                }
            }
            Message::NotifToggleShowBody => {
                self.notif_config.show_body = !self.notif_config.show_body;
                if let Err(err) = self.notif_config.save() {
                    self.toast(ToastKind::Error, err);
                }
            }
            Message::NotifToggleShowActions => {
                self.notif_config.show_actions = !self.notif_config.show_actions;
                if let Err(err) = self.notif_config.save() {
                    self.toast(ToastKind::Error, err);
                }
            }
            Message::NotifToggleGroupByApp => {
                self.notif_config.group_by_app = !self.notif_config.group_by_app;
                if let Err(err) = self.notif_config.save() {
                    self.toast(ToastKind::Error, err);
                }
            }
            Message::NotifAccentHex(hex) => {
                let valid = hex.trim().is_empty()
                    || crate::central::notifications::config::parse_hex_color(&hex).is_some();
                self.notif_config.accent_hex = hex;
                if valid && let Err(err) = self.notif_config.save() {
                    self.toast(ToastKind::Error, err);
                }
            }
            Message::NotifBgHex(hex) => {
                let valid = hex.trim().is_empty()
                    || crate::central::notifications::config::parse_hex_color(&hex).is_some();
                self.notif_config.bg_hex = hex;
                if valid && let Err(err) = self.notif_config.save() {
                    self.toast(ToastKind::Error, err);
                }
            }
            Message::NotifReset => {
                self.notif_config = NotifConfig::default();
                match self.notif_config.save() {
                    Ok(()) => self.toast(
                        ToastKind::Success,
                        "Configurações de notificações restauradas para o padrão.",
                    ),
                    Err(err) => self.toast(ToastKind::Error, err),
                }
            }
            Message::NotifSendTest => {
                let timeout_ms = if self.notif_config.timeout_secs == 0 {
                    "0".to_owned()
                } else {
                    (self.notif_config.timeout_secs * 1000).to_string()
                };
                self.toast_legacy(data::spawn_app(
                    "notify-send",
                    &[
                        "-a",
                        "Painel de Controle",
                        "-i",
                        "preferences-system-notifications-symbolic",
                        "-t",
                        &timeout_ms,
                        "Notificação de teste",
                        "Pré-visualização do formato, transparência, cores e duração configurados.",
                    ],
                    "Notificação de teste",
                ));
            }
            // Devices OSD
            Message::OsdDurationMs(ms) => {
                self.osd_config.duration =
                    std::time::Duration::from_millis(ms.clamp(200, 20_000));
                self.persist_osd_config();
            }
            Message::OsdBatteryLow(pct) => {
                self.osd_config.battery_low = pct.clamp(2, 99);
                if self.osd_config.battery_critical >= self.osd_config.battery_low {
                    self.osd_config.battery_critical = self.osd_config.battery_low - 1;
                }
                self.persist_osd_config();
            }
            Message::OsdBatteryCritical(pct) => {
                let max_crit = self.osd_config.battery_low.saturating_sub(1).max(1);
                self.osd_config.battery_critical = pct.clamp(1, max_crit);
                self.persist_osd_config();
            }
            Message::OsdBatteryIntervalMins(mins) => {
                self.osd_config.legacy_battery_interval =
                    std::time::Duration::from_secs(mins.clamp(1, 1440) * 60);
                self.persist_osd_config();
            }
            Message::OsdEmojiCommand(cmd) => {
                let trimmed = cmd.trim();
                self.osd_config.emoji_command = if trimmed.is_empty() {
                    None
                } else {
                    Some(cmd)
                };
                self.persist_osd_config();
            }
            // Cada mudança do OSD já grava em `persist_osd_config`; "Aplicar" só reinicia o serviço.
            Message::ApplyOsd => {
                if self.busy {
                    return Task::none();
                }
                self.osd_config = self.osd_config.normalized();
                if let Err(err) = self.osd_config.save() {
                    self.toast(ToastKind::Error, err);
                    return Task::none();
                }
                let applying = Applying::Osd(Box::new(self.osd_config.clone()));
                self.begin_busy("Reiniciando o OSD de Dispositivos...");
                return Task::future(async move {
                    let msg = data::restart_user_unit("logi-osd.service").await;
                    let result = if legacy_kind(&msg) == ToastKind::Error { Err(msg) } else { Ok(msg) };
                    cosmic::action::app(Message::ApplyDone(applying, result))
                });
            }
            Message::OsdReset => {
                self.osd_config = osd_config::Config::default();
                match self.osd_config.save() {
                    Ok(_) => self.toast(
                        ToastKind::Success,
                        "Configuração do OSD restaurada para o padrão.",
                    ),
                    Err(err) => self.toast(ToastKind::Error, err),
                }
            }
            Message::OsdRunDemo => {
                let exe = std::env::current_exe()
                    .map(|p| {
                        let s = p.to_string_lossy();
                        s.strip_suffix(" (deleted)").unwrap_or(&s).to_owned()
                    })
                    .unwrap_or_else(|_| "cosmic-taskbar".to_owned());
                self.toast_legacy(data::spawn_app(
                    &exe,
                    &["logi", "--demo"],
                    "Demonstração do OSD de Dispositivos",
                ));
            }
            // Camera
            Message::ToggleMicrophone => {
                return Task::future(async {
                    services::toggle_microphone().await;
                    cosmic::action::app(Message::None)
                });
            }
            Message::ToggleCamera => {
                let verb = if self.camera.on { "stop" } else { "start" };
                return Task::future(async move {
                    let cam = services::camera_command(verb).await;
                    cosmic::action::app(Message::CameraLoaded(cam))
                });
            }
            Message::RestartCamera => {
                return Task::future(async {
                    let cam = services::camera_command("restart").await;
                    cosmic::action::app(Message::CameraLoaded(cam))
                });
            }
            Message::IriunWindow(show) => {
                return Task::future(async move {
                    let cam = services::camera_window(show).await;
                    cosmic::action::app(Message::CameraLoaded(cam))
                });
            }
            Message::RefreshCamera => {
                return Task::future(async {
                    cosmic::action::app(Message::CameraLoaded(services::camera().await))
                });
            }
            // Network
            Message::ToggleWifi => {
                let next = !self.network.wifi_enabled;
                return Task::future(async move {
                    _ = services::set_wifi(next).await;
                    cosmic::action::app(Message::None)
                });
            }
            Message::ConnectVpn(index) => {
                if let Some(vpn) = self.network.vpns.get(index).cloned() {
                    return Task::future(async move {
                        let msg = match services::connect_vpn(vpn).await {
                            Ok(()) => "Estado da VPN atualizado.".to_owned(),
                            Err(err) => format!("Falha ao alternar VPN: {err}"),
                        };
                        cosmic::action::app(Message::ActionDone(msg))
                    });
                }
            }
            Message::OpenCosmicSettings(page) => {
                let mut cmd = std::process::Command::new("cosmic-settings");
                if let Some(p) = page {
                    cmd.arg(p);
                }
                _ = cmd.spawn();
            }
            // Displays
            Message::SetPrimaryDisplay(name) => {
                layout::set_primary_display(&name);
                self.primary_display = Some(name);
            }
            Message::SetDdcBrightness(index, percent) => {
                if let Some((_, cur, max)) = self
                    .displays
                    .as_mut()
                    .and_then(|d| d.get_mut(index))
                    .and_then(|d| d.ddc.as_mut())
                {
                    *cur = (percent / 100.0 * *max as f32).round() as u32;
                }
            }
            Message::ApplyDdcBrightness(index) => {
                if let Some((num, cur, max)) = self
                    .displays
                    .as_ref()
                    .and_then(|d| d.get(index))
                    .and_then(|d| d.ddc)
                {
                    return Task::future(async move {
                        services::set_ddc_brightness(num, cur, max).await;
                        cosmic::action::app(Message::None)
                    });
                }
            }
            Message::SetInternalBrightness(percent) => {
                if let Some((cur, max)) = self.brightness.as_mut() {
                    *cur = ((percent / 100.0 * *max as f32) as i32)
                        .max(*max / 100)
                        .max(1);
                    let (v, m) = (*cur, *max);
                    return Task::future(async move {
                        services::set_brightness(v, m).await;
                        cosmic::action::app(Message::None)
                    });
                }
            }
            Message::RunWallpapers => {
                self.begin_busy("Reaplicando wallpapers nos monitores...");
                return Task::future(async {
                    let msg = data::run_user_tool(
                        "wallpapers-monitores",
                        &[],
                        "Wallpapers reaplicados nos monitores.",
                    )
                    .await;
                    cosmic::action::app(Message::ActionDone(msg))
                });
            }
            Message::RunBravePwaFix => {
                self.begin_busy("Corrigindo ícones de PWAs do Brave...");
                return Task::future(async {
                    let msg = data::run_user_tool(
                        "fix-brave-pwa-icons",
                        &[],
                        "Ícones de PWAs do Brave verificados e corrigidos.",
                    )
                    .await;
                    cosmic::action::app(Message::ActionDone(msg))
                });
            }
            Message::RunScreenshot => {
                let home = std::env::var("HOME").unwrap_or_default();
                let bin = std::path::PathBuf::from(&home).join(".local/bin/print-recorte");
                self.toast_legacy(data::spawn_app(
                    &bin.to_string_lossy(),
                    &[],
                    "Recorte de tela",
                ));
            }
            Message::OpenScreenshotsDir => {
                let dir = self.screenshot_config.resolved_dir();
                _ = std::fs::create_dir_all(&dir);
                self.toast_legacy(data::spawn_app(
                    "xdg-open",
                    &[&dir.to_string_lossy()],
                    "Pasta de capturas",
                ));
            }
            Message::ScreenshotSaveDirInput(val) => {
                self.screenshot_config.save_dir = val;
                self.persist_screenshot_config();
            }
            Message::ScreenshotToggleCopy => {
                self.screenshot_config.copy_clipboard = !self.screenshot_config.copy_clipboard;
                self.persist_screenshot_config();
            }
            Message::ScreenshotOpenMode(mode) => {
                self.screenshot_config.open_mode = mode;
                self.persist_screenshot_config();
            }
            Message::ScreenshotSattyTool(tool) => {
                self.screenshot_config.satty_tool = tool;
                self.persist_screenshot_config();
            }
            Message::ScreenshotBorderColor(color) => {
                self.screenshot_config.border_color = color;
                self.persist_screenshot_config();
            }
            Message::ScreenshotBorderWidth(w) => {
                self.screenshot_config.border_width = w.clamp(1, 6);
                self.persist_screenshot_config();
            }
            Message::ScreenshotOverlayOpacity(pct) => {
                self.screenshot_config.overlay_opacity = pct.clamp(0, 85);
                self.persist_screenshot_config();
            }
            Message::ScreenshotTimeoutSecs(secs) => {
                self.screenshot_config.notify_timeout_secs = secs.clamp(30, 3600);
                self.persist_screenshot_config();
            }
            Message::ScreenshotToggleSound => {
                self.screenshot_config.play_sound = !self.screenshot_config.play_sound;
                self.persist_screenshot_config();
            }
            Message::ScreenshotReset => {
                self.screenshot_config = ScreenshotConfig::default();
                self.persist_screenshot_config();
                self.toast(
                    ToastKind::Success,
                    "Configurações de recorte (Win+Shift+S) restauradas para o padrão.",
                );
            }
            Message::RefreshDisplays => {
                self.displays = None;
                return Task::future(async {
                    cosmic::action::app(Message::DisplaysLoaded(
                        services::displays(services::DdcRead::All).await,
                    ))
                });
            }
            // Login (Greeter & Lockscreen)
            Message::GreeterStyle(val) => {
                self.greeter.style = val;
                self.persist_greeter_config();
            }
            Message::GreeterWallpaperInput(val) => {
                self.greeter.wallpaper = val;
                self.persist_greeter_config();
            }
            Message::GreeterWallpaperMode(val) => {
                self.greeter.wallpaper_mode = val;
                self.persist_greeter_config();
            }
            Message::GreeterWallpaperDirInput(val) => {
                self.greeter.wallpaper_dir = val;
                self.persist_greeter_config();
            }
            Message::GreeterToggleShuffle => {
                self.greeter.wallpaper_shuffle = !self.greeter.wallpaper_shuffle;
                self.persist_greeter_config();
            }
            Message::GreeterIntervalMins(mins) => {
                self.greeter.wallpaper_interval_mins = mins.clamp(1, 1440);
                self.persist_greeter_config();
            }
            Message::GreeterTheme(theme) => {
                self.greeter.theme = theme;
                self.persist_greeter_config();
            }
            Message::GreeterFontFamily(val) => {
                self.greeter.font_family = val;
                self.persist_greeter_config();
            }
            Message::GreeterFontWeight(val) => {
                self.greeter.font_weight = val;
                self.persist_greeter_config();
            }
            Message::GreeterClockSize(val) => {
                self.greeter.clock_size = val;
                self.persist_greeter_config();
            }
            Message::GreeterToggleClock24h => {
                self.greeter.clock_24h = !self.greeter.clock_24h;
                self.persist_greeter_config();
            }
            Message::GreeterToggleShowSeconds => {
                self.greeter.show_seconds = !self.greeter.show_seconds;
                self.persist_greeter_config();
            }
            Message::GreeterToggleShowDate => {
                self.greeter.show_date = !self.greeter.show_date;
                self.persist_greeter_config();
            }
            Message::GreeterToggleShowAvatar => {
                self.greeter.show_avatar = !self.greeter.show_avatar;
                self.persist_greeter_config();
            }
            Message::GreeterToggleShowUsername => {
                self.greeter.show_username = !self.greeter.show_username;
                self.persist_greeter_config();
            }
            Message::GreeterBlurRadius(radius) => {
                self.greeter.blur_radius = radius.min(60);
                self.persist_greeter_config();
            }
            Message::GreeterDimOpacity(val) => {
                self.greeter.dim_opacity = ((val * 100.0).round() / 100.0).clamp(0.0, 0.85);
                self.persist_greeter_config();
            }
            Message::GreeterCardOpacity(val) => {
                self.greeter.card_opacity = ((val * 100.0).round() / 100.0).clamp(0.0, 0.95);
                self.persist_greeter_config();
            }
            Message::GreeterCardRadius(r) => {
                self.greeter.card_radius = r.round().clamp(4.0, 32.0);
                self.persist_greeter_config();
            }
            Message::GreeterAccentColor(hex) => {
                let valid = hex.trim().is_empty()
                    || crate::central::notifications::config::parse_hex_color(&hex).is_some();
                self.greeter.accent_color = hex;
                if valid {
                    self.persist_greeter_config();
                }
            }
            Message::GreeterDefaultUserInput(val) => {
                self.greeter.default_user = val;
                self.persist_greeter_config();
            }
            Message::GreeterSessionInput(val) => {
                self.greeter.session = val;
                self.persist_greeter_config();
            }
            Message::GreeterToggleIsolatedBinary => {
                self.greeter.isolated_binary = !self.greeter.isolated_binary;
                self.persist_greeter_config();
            }
            Message::GreeterTestLock => {
                self.persist_greeter_config();
                self.toast_legacy(data::spawn_app(
                    "loginctl",
                    &["lock-session"],
                    "Bloqueio de tela",
                ));
            }
            Message::GreeterSaveOnly => match data::save_greeter_config(&self.greeter) {
                Ok(path) => {
                    self.toast_legacy(format!(
                        "Configuração salva em {}. Clique em \"Aplicar na tela de login\" para ativar.",
                        path.display()
                    ));
                }
                Err(err) => self.toast(ToastKind::Error, err),
            },
            Message::GreeterApply => {
                self.begin_busy("Aplicando preferências na tela de login...");
                let cfg = self.greeter.clone();
                return Task::future(async move {
                    let msg = data::apply_greeter(cfg).await;
                    cosmic::action::app(Message::ActionDone(msg))
                });
            }
            // Optional packages & Security/Power/Backup actions
            Message::OpenInTerminal(title, cmd) => {
                self.toast_legacy(data::open_in_terminal(title, &cmd));
            }
            Message::RunAction(label, cmd, privileged) => {
                if self.busy {
                    return Task::none();
                }
                self.begin_busy(format!("Executando: {label}..."));
                return Task::future(async move {
                    let msg = data::run_shell_action(label, &cmd, privileged).await;
                    cosmic::action::app(Message::ActionDone(msg))
                });
            }
            Message::SpawnApp(program, args, label) => {
                self.toast_legacy(data::spawn_app(program, &args, label));
            }
            Message::EnableUserUnit(unit) => {
                self.begin_busy(format!("Habilitando {unit}..."));
                return Task::future(async move {
                    let msg = data::enable_user_unit(unit).await;
                    cosmic::action::app(Message::ActionDone(msg))
                });
            }
            // System & Updates
            Message::CheckUpdates => {
                self.begin_busy("Verificando atualizações...");
                return Task::future(async {
                    let msg = data::run_user_tool(
                        "cosmic-taskbar-update",
                        &["--check"],
                        "Verificação concluída.",
                    )
                    .await;
                    cosmic::action::app(Message::ActionDone(msg))
                });
            }
            Message::RunUpdateNow => {
                self.begin_busy("Verificando e instalando atualizações do GitHub...");
                return Task::future(async {
                    let msg = data::run_user_tool(
                        "cosmic-taskbar-update",
                        &["--force"],
                        "Atualização concluída.",
                    )
                    .await;
                    cosmic::action::app(Message::ActionDone(msg))
                });
            }
            Message::ExportConfig => {
                self.begin_busy("Exportando o perfil visual...");
                return Task::future(async {
                    let msg = data::export_repo_config().await;
                    cosmic::action::app(Message::ActionDone(msg))
                });
            }
            Message::RestartUserUnit(unit) => {
                self.begin_busy(format!("Reiniciando {unit}..."));
                return Task::future(async move {
                    let msg = data::restart_user_unit(unit).await;
                    cosmic::action::app(Message::ActionDone(msg))
                });
            }
            Message::RefreshAll => {
                self.monitor_settings = MonitorSettings::load();
                self.weather_place = monitor::config::weather_place();
                self.central_layout = Layout::load();
                let prev_accent = self.notif_config.accent_hex.clone();
                let prev_bg = self.notif_config.bg_hex.clone();
                self.notif_config = NotifConfig::load();
                if !prev_accent.is_empty()
                    && crate::central::notifications::config::parse_hex_color(&prev_accent).is_none()
                {
                    self.notif_config.accent_hex = prev_accent;
                }
                if !prev_bg.is_empty()
                    && crate::central::notifications::config::parse_hex_color(&prev_bg).is_none()
                {
                    self.notif_config.bg_hex = prev_bg;
                }
                self.osd_config = osd_config::load_or_default();
                self.greeter = data::load_greeter_config();
                self.power_config = PowerProfilesConfig::load(
                    self.snapshot.hardware.is_intel,
                    self.snapshot.hardware.is_laptop,
                );
                self.cpu_topology = power_config::detect_cpu_topology();
                return self.reload_all();
            }
            Message::ActionDone(msg) => {
                return self.finish_action(msg);
            }
            Message::ApplyDone(what, result) => {
                let (kind, text) = settle_apply(&mut self.applied, &what, result);
                return self.finish_action_as(kind, text);
            }
            Message::SetWindowGaps(outer, inner) => {
                self.window_gaps = (outer, inner);
                services::set_window_gaps(outer, inner);
                self.toast_legacy(format!(
                    "Espaçamento de janela dividida aplicado: margem externa {} px · entre janelas {} px.",
                    outer, inner
                ));
            }
            Message::SetActivationPolicy(policy) => {
                self.activation_policy = policy.to_owned();
                services::set_activation_policy(policy);
                let label = match policy {
                    "Urgent" => "Não roubar foco (apenas destacar na barra)",
                    "FocusIfActiveWorkspace" => "Focar apenas na área de trabalho atual",
                    _ => "Sempre focar (padrão COSMIC)",
                };
                self.toast_legacy(format!("Prioridade de foco de janelas: {label}."));
            }
            // Moldura: paleta Ctrl K
            Message::OpenPalette => {
                if self.palette.is_none() {
                    self.palette = Some(PaletteState::default());
                }
                return cosmic::widget::text_input::focus(views::palette_input_id());
            }
            Message::ClosePalette => {
                self.palette = None;
            }
            Message::PaletteQuery(query) => {
                if let Some(p) = self.palette.as_mut() {
                    p.query = query;
                    p.cursor = 0;
                }
            }
            Message::PaletteMove(delta) => {
                if let Some(p) = self.palette.as_mut() {
                    p.cursor = moved_cursor(p.cursor, delta, chrome::flat_results(&p.query).len());
                }
            }
            Message::PaletteActivate { run } => {
                return self.activate_palette(run);
            }
            Message::PaletteClick(idx) => {
                let Some(p) = self.palette.as_mut() else {
                    return Task::none();
                };
                p.cursor = idx;
                // Clique = Enter: vai à seção. Executar a ação só com Ctrl+Enter.
                return self.activate_palette(false);
            }
            Message::OpenNative(page) => {
                // Destacado e sem esperar o fim; só avisa se nem abriu.
                let msg = data::spawn_app("cosmic-settings", &[page], "COSMIC Settings");
                if legacy_kind(&msg) == ToastKind::Error {
                    self.toast_legacy(msg);
                }
            }
            Message::JumpTo(group) => {
                return self.jump_to(group);
            }
            Message::ToggleAdvanced => {
                self.advanced_open = !self.advanced_open;
            }
            // Moldura: avisos e desfazer
            Message::DismissToast(id) => {
                self.toasts.dismiss(id);
            }
            Message::Tick => {
                self.toasts.expire(Instant::now());
            }
            Message::Undo(idx) => {
                let Some(undo) = self.recent.take_undo(idx) else {
                    return Task::none();
                };
                self.undoing = true;
                let task = self.update(undo);
                self.undoing = false;
                self.toast(ToastKind::Info, "Alteração desfeita.");
                return task;
            }
            Message::UndoAsProfile(kind, inner) => {
                let editing = self.editing_profile;
                self.editing_profile = kind;
                let task = self.update(*inner);
                self.editing_profile = editing;
                return task;
            }
            // Moldura: métricas da Visão geral
            Message::MetricsTick => {
                if self.section == Section::Home {
                    return self.read_metrics();
                }
            }
            Message::MetricsRead(now) => {
                self.metrics_inflight = false;
                if self.section == Section::Home {
                    if let Some(prev) = &self.last_reading {
                        self.metrics = Some(metrics::compute(
                            prev,
                            &now,
                            now.gpu.as_ref(),
                            self.monitor_settings.rest_watts as f32,
                            self.monitor_settings.tariff.unwrap_or(0.0) as f32,
                        ));
                    }
                    self.last_reading = Some(*now);
                }
            }
            Message::WindowFocus(focused) => {
                self.focused = focused;
            }
            Message::None => {}
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            services::subscription().map(Message::ServiceUpdate),
            services::settings_subscription().map(|()| Message::SettingsChanged),
            audio::privacy_subscription().map(Message::PrivacyUpdate),
        ];
        if self.dragging_bar.is_some() {
            subs.push(event::listen_with(|ev, _, _| match ev {
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                    iced::mouse::Button::Left,
                )) => Some(Message::DragBarEnd),
                _ => None,
            }));
        }
        if self.dragging_monitor_card.is_some() {
            subs.push(event::listen_with(|ev, _, _| match ev {
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                    iced::mouse::Button::Left,
                )) => Some(Message::MonitorDragEnd),
                _ => None,
            }));
        }
        subs.push(event::listen_with(shell_event));
        if self.palette.is_some() {
            subs.push(event::listen_with(palette_key));
        }
        if !self.toasts.visible().is_empty() {
            subs.push(iced::time::every(TOAST_TICK).map(|_| Message::Tick));
        }
        if self.section == Section::Home {
            let every = if self.focused {
                metrics::POLL_FOCUSED
            } else {
                metrics::POLL_UNFOCUSED
            };
            subs.push(iced::time::every(every).map(|_| Message::MetricsTick));
        }
        Subscription::batch(subs)
    }

    fn view(&self) -> Element<'_, Message> {
        views::view(self)
    }

    fn on_close_requested(&self, _id: window::Id) -> Option<Message> {
        None
    }
}

/// Atalhos globais e foco da janela. `Captured` = um campo de texto já tratou a tecla.
fn shell_event(ev: iced::Event, status: event::Status, _id: window::Id) -> Option<Message> {
    use iced::keyboard::{Event as Key, Key as Pressed, key::Named};
    match ev {
        iced::Event::Window(window::Event::Focused) => Some(Message::WindowFocus(true)),
        iced::Event::Window(window::Event::Unfocused) => Some(Message::WindowFocus(false)),
        iced::Event::Keyboard(Key::KeyPressed { key, modifiers, .. }) => match key.as_ref() {
            Pressed::Character("k" | "K") if modifiers.control() => Some(Message::OpenPalette),
            // "/" só quando nenhum campo de texto o consumiu como caractere digitado.
            Pressed::Character("/") if status == event::Status::Ignored && !modifiers.control() => {
                Some(Message::OpenPalette)
            }
            Pressed::Named(Named::Escape) if status == event::Status::Ignored => {
                Some(Message::ClosePalette)
            }
            _ => None,
        },
        _ => None,
    }
}

/// Teclas da paleta (só assinadas com a paleta aberta). Valem mesmo com o campo de busca
/// focado, que captura Enter e as setas; por isso a paleta não deve ligar `on_submit`.
fn palette_key(ev: iced::Event, _status: event::Status, _id: window::Id) -> Option<Message> {
    use iced::keyboard::{Event as Key, Key as Pressed, key::Named};
    let iced::Event::Keyboard(Key::KeyPressed { key, modifiers, .. }) = ev else {
        return None;
    };
    match key.as_ref() {
        Pressed::Named(Named::Escape) => Some(Message::ClosePalette),
        Pressed::Named(Named::ArrowDown) => Some(Message::PaletteMove(1)),
        Pressed::Named(Named::ArrowUp) => Some(Message::PaletteMove(-1)),
        Pressed::Named(Named::Enter) => Some(Message::PaletteActivate { run: modifiers.control() }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_bar_label_says_what_the_button_does() {
        assert_eq!(
            apply_bar_label(&ApplyAction::ApplyHardware, PowerProfileKind::Balanced),
            "Aplicar no hardware"
        );
        assert_eq!(
            apply_bar_label(
                &ApplyAction::ActivateAndApply(PowerProfileKind::Performance),
                PowerProfileKind::Performance
            ),
            "Ativar e aplicar Desempenho"
        );
        assert_eq!(
            apply_bar_label(
                &ApplyAction::ActivateAndApply(PowerProfileKind::PowerSaver),
                PowerProfileKind::PowerSaver
            ),
            "Ativar e aplicar Economia"
        );
    }

    #[test]
    fn save_status_counts_pending_items() {
        assert_eq!(save_status(0), "Tudo salvo");
        assert_eq!(save_status(1), "1 ajuste para aplicar");
        assert_eq!(save_status(2), "2 ajustes para aplicar");
    }

    #[test]
    fn palette_cursor_wraps_and_survives_empty_results() {
        assert_eq!(moved_cursor(0, 1, 3), 1);
        assert_eq!(moved_cursor(2, 1, 3), 0);
        assert_eq!(moved_cursor(0, -1, 3), 2);
        assert_eq!(moved_cursor(4, 1, 0), 0);
        assert_eq!(moved_cursor(1, 0, 1), 0);
    }

    #[test]
    fn failed_apply_keeps_pending_and_succeeded_apply_clears_it() {
        let mut cfg = PowerProfilesConfig::defaults_for(false, false);
        let kind = PowerProfileKind::Balanced;
        let mut applied = Applied::default();
        applied.mark_power(kind, cfg.get(kind));
        cfg.get_mut(kind).boost = !cfg.get(kind).boost;
        assert!(pending::power_dirty(&cfg, &applied, kind));
        let what = Applying::Power(kind, Box::new(cfg.get(kind).clone()));

        // pkexec cancelado: nada é marcado, a pendência continua e o aviso é de erro.
        let (toast, text) = settle_apply(&mut applied, &what, Err("economia-energia falhou: cancelado".into()));
        assert_eq!(toast, ToastKind::Error);
        assert!(text.contains("cancelado"));
        assert!(pending::power_dirty(&cfg, &applied, kind));

        let (toast, _) = settle_apply(&mut applied, &what, Ok("Plano aplicado.".into()));
        assert_eq!(toast, ToastKind::Success);
        assert!(!pending::power_dirty(&cfg, &applied, kind));
    }

    #[test]
    fn failed_osd_restart_keeps_osd_pending() {
        let saved = osd_config::Config::default();
        let mut applied = Applied::default();
        applied.mark_osd(&saved);
        let edited = osd_config::Config { battery_low: saved.battery_low - 1, ..saved.clone() };
        assert!(pending::osd_pending(&edited, &applied));
        let what = Applying::Osd(Box::new(edited.clone()));

        let (toast, _) = settle_apply(&mut applied, &what, Err("Falha ao reiniciar logi-osd.service".into()));
        assert_eq!(toast, ToastKind::Error);
        assert!(pending::osd_pending(&edited, &applied));

        let (toast, text) = settle_apply(&mut applied, &what, Ok("Serviço logi-osd.service reiniciado.".into()));
        assert_eq!(toast, ToastKind::Success);
        assert!(text.contains("reiniciado"));
        assert!(!pending::osd_pending(&edited, &applied));
    }

    #[test]
    fn legacy_prefixes_pick_the_toast_kind() {
        assert_eq!(legacy_kind("✓ Plano aplicado."), ToastKind::Success);
        assert_eq!(legacy_kind("Falha ao reiniciar x"), ToastKind::Error);
        assert_eq!(legacy_kind("Não foi possível abrir Hot Edge: x"), ToastKind::Error);
        assert_eq!(legacy_kind("Brave iniciado."), ToastKind::Info);
    }
}
