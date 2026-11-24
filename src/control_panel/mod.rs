// SPDX-License-Identifier: GPL-3.0-only

//! Full native COSMIC Control Panel application (`cosmic-taskbar control-panel`):
//! centralizes power profiles, battery/USB-C PD telemetry, Lenovo conservation
//! mode, taskbar/tray ordering, left Monitor panel settings, right Central panel
//! settings, multi-camera & physical privacy switches, network/security details,
//! display DDC/visuals, isolated login screen (Greeter), optional packages, and
//! auto-updater/services.

mod data;
mod power_config;
mod screenshot_config;
mod views;

use power_config::{CpuTopologyInfo, PowerProfileKind, PowerProfilesConfig};
use screenshot_config::{OpenMode as ScreenshotOpenMode, ScreenshotConfig};

use cosmic::{
    Element, Task, app,
    iced::{self, Subscription, event, window},
};

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
    pub const ALL: [Section; 14] = [
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
        Section::Optional,
        Section::System,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Section::Power => "Energia & Bateria",
            Section::Bar => "Barra & Tray",
            Section::Monitor => "Monitor (Esquerda)",
            Section::Central => "Central (Direita)",
            Section::Notifications => "Notificações",
            Section::DevicesOsd => "OSD de Dispositivos",
            Section::Camera => "Câmeras & Áudio",
            Section::Network => "Rede",
            Section::Security => "Segurança & Privacidade",
            Section::Backup => "Backup & Sincronização",
            Section::Displays => "Monitores & Visuais",
            Section::Login => "Tela de Login",
            Section::Optional => "Pacotes & Opcionais",
            Section::System => "Atualizações & Serviços",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
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
    PowerSaveOnly,
    PowerSaveAndApply,
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
    OsdSaveOnly,
    OsdSaveAndRestart,
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
    DismissNotice,
    ToggleSecurityAdvanced,
    None,
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
    security_show_advanced: bool,
    busy: bool,
    notice: Option<String>,
}

pub fn run() -> cosmic::iced::Result {
    crate::shared::logging::init("warn,cosmic_taskbar=info", &[]);
    let settings = cosmic::app::Settings::default()
        .size(iced::Size::new(1040.0, 720.0))
        .size_limits(iced::Limits::NONE.min_width(780.0).min_height(540.0));
    cosmic::app::run::<ControlPanel>(settings, ())
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

    fn persist_power_config(&mut self, sync_idle: bool) {
        if let Err(err) = self.power_config.save() {
            self.notice = Some(format!("Falha ao salvar perfis de energia: {err}"));
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
                self.notice = Some(format!("Falha ao aplicar tempos de tela/login: {err}"));
            }
        }
    }

    fn persist_osd_config(&mut self) {
        if let Err(err) = self.osd_config.normalized().save() {
            self.notice = Some(err);
        }
    }

    fn persist_greeter_config(&mut self) {
        if let Err(err) = data::save_greeter_config(&self.greeter) {
            self.notice = Some(err);
        }
    }

    fn persist_screenshot_config(&mut self) {
        if let Err(err) = self.screenshot_config.save() {
            self.notice = Some(format!("Falha ao salvar config de captura: {err}"));
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
    type Flags = ();
    const APP_ID: &'static str = APP_ID;

    fn init(mut core: app::Core, _flags: ()) -> (Self, app::Task<Message>) {
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
        let app = Self {
            core,
            section: Section::Power,
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
            osd_config: osd_config::load_or_default(),
            screenshot_config: ScreenshotConfig::load(),
            greeter: data::load_greeter_config(),
            security_show_advanced: false,
            busy: false,
            notice: None,
        };
        let load = app.reload_all();
        (app, load)
    }

    fn core(&self) -> &app::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut app::Core {
        &mut self.core
    }

    fn header_center(&self) -> Vec<Element<'_, Message>> {
        vec![cosmic::widget::text::title4("Painel de Controle").into()]
    }

    fn update(&mut self, message: Message) -> app::Task<Message> {
        match message {
            Message::SelectSection(sec) => {
                self.section = sec;
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
            }
            Message::SnapshotLoaded(snapshot) => {
                self.snapshot = snapshot;
                self.tray_config = data::load_tray_config();
            }
            Message::ServiceUpdate(update) => match update {
                Update::Network(net) => self.network = net,
                Update::Battery(bat) => self.battery = bat,
                Update::Peripherals(p) => self.peripherals = p,
                Update::PowerProfiles(p) => {
                    if self.power.is_none()
                        && let Some(active) = p.as_ref().and_then(|pp| PowerProfileKind::from_id(&pp.active))
                    {
                        self.editing_profile = active;
                    }
                    self.power = p;
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
                self.busy = true;
                self.notice = Some(format!("Alterando plano de energia para {profile}..."));
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
            Message::PowerSaveOnly => match self.power_config.save() {
                Ok(()) => {
                    self.notice = Some(format!(
                        "Perfis de energia salvos em {}.",
                        PowerProfilesConfig::config_path().display()
                    ));
                }
                Err(err) => {
                    self.notice = Some(format!("Falha ao salvar perfis de energia: {err}"));
                }
            },
            Message::PowerSaveAndApply => {
                if let Err(err) = self.power_config.save() {
                    self.notice = Some(format!("Falha ao salvar perfis de energia: {err}"));
                    return Task::none();
                }
                let kind = self.editing_profile;
                let profile_id = kind.id().to_owned();
                let tuning = self.power_config.get(kind).clone();
                self.busy = true;
                self.notice = Some(format!("Aplicando plano {}...", kind.label()));
                return Task::future(async move {
                    _ = services::set_power_profile(profile_id.clone()).await;
                    let msg = power_config::apply_profile_now(&profile_id, &tuning).await;
                    cosmic::action::app(Message::ActionDone(msg))
                });
            }
            Message::PowerResetDefaults => {
                let hw = &self.snapshot.hardware;
                self.power_config = PowerProfilesConfig::defaults_for(hw.is_intel, hw.is_laptop);
                match self.power_config.save() {
                    Ok(()) => {
                        self.notice = Some(
                            "Padrões do hardware restaurados e salvos. Clique em \"Salvar e aplicar plano\" para aplicar agora."
                                .to_owned(),
                        );
                    }
                    Err(err) => {
                        self.notice = Some(format!("Padrões restaurados, mas falhou ao salvar: {err}"));
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
                self.busy = true;
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
                self.notice = Some("Layout do Monitor restaurado para o padrão.".to_owned());
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
                self.notice = Some(format!("Local do clima definido para {label}."));
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
                self.notice = Some(data::spawn_app("cosmic-ext-hot-edge", &[], "Hot Edge"));
            }
            Message::CentralResetTiles => {
                data::reset_central_tiles();
                self.notice =
                    Some("Grade de tiles da Central restaurada para o padrão.".to_owned());
            }
            // Notifications
            Message::NotifTimeout(secs) => {
                self.notif_config.timeout_secs = secs;
                if let Err(err) = self.notif_config.save() {
                    self.notice = Some(err);
                }
            }
            Message::NotifFormat(fmt) => {
                self.notif_config.format = fmt;
                if let Err(err) = self.notif_config.save() {
                    self.notice = Some(err);
                }
            }
            Message::NotifPosition(pos) => {
                self.notif_config.position = pos;
                if let Err(err) = self.notif_config.save() {
                    self.notice = Some(err);
                }
            }
            Message::NotifOpacity(val) => {
                self.notif_config.opacity = ((val * 100.0).round() / 100.0).clamp(
                    crate::central::notifications::config::MIN_OPACITY,
                    crate::central::notifications::config::MAX_OPACITY,
                );
                if let Err(err) = self.notif_config.save() {
                    self.notice = Some(err);
                }
            }
            Message::NotifCornerRadius(r) => {
                self.notif_config.corner_radius = r.round().clamp(
                    crate::central::notifications::config::MIN_CORNER_RADIUS,
                    crate::central::notifications::config::MAX_CORNER_RADIUS,
                );
                if let Err(err) = self.notif_config.save() {
                    self.notice = Some(err);
                }
            }
            Message::NotifMaxBodyLines(lines) => {
                self.notif_config.max_body_lines = lines.clamp(
                    crate::central::notifications::config::MIN_BODY_LINES,
                    crate::central::notifications::config::MAX_BODY_LINES,
                );
                if let Err(err) = self.notif_config.save() {
                    self.notice = Some(err);
                }
            }
            Message::NotifToggleShowBody => {
                self.notif_config.show_body = !self.notif_config.show_body;
                if let Err(err) = self.notif_config.save() {
                    self.notice = Some(err);
                }
            }
            Message::NotifToggleShowActions => {
                self.notif_config.show_actions = !self.notif_config.show_actions;
                if let Err(err) = self.notif_config.save() {
                    self.notice = Some(err);
                }
            }
            Message::NotifToggleGroupByApp => {
                self.notif_config.group_by_app = !self.notif_config.group_by_app;
                if let Err(err) = self.notif_config.save() {
                    self.notice = Some(err);
                }
            }
            Message::NotifAccentHex(hex) => {
                let valid = hex.trim().is_empty()
                    || crate::central::notifications::config::parse_hex_color(&hex).is_some();
                self.notif_config.accent_hex = hex;
                if valid && let Err(err) = self.notif_config.save() {
                    self.notice = Some(err);
                }
            }
            Message::NotifBgHex(hex) => {
                let valid = hex.trim().is_empty()
                    || crate::central::notifications::config::parse_hex_color(&hex).is_some();
                self.notif_config.bg_hex = hex;
                if valid && let Err(err) = self.notif_config.save() {
                    self.notice = Some(err);
                }
            }
            Message::NotifReset => {
                self.notif_config = NotifConfig::default();
                self.notice = Some(match self.notif_config.save() {
                    Ok(()) => "Configurações de notificações restauradas para o padrão.".to_owned(),
                    Err(err) => err,
                });
            }
            Message::NotifSendTest => {
                let timeout_ms = if self.notif_config.timeout_secs == 0 {
                    "0".to_owned()
                } else {
                    (self.notif_config.timeout_secs * 1000).to_string()
                };
                self.notice = Some(data::spawn_app(
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
            Message::OsdSaveOnly => {
                self.osd_config = self.osd_config.normalized();
                self.notice = Some(match self.osd_config.save() {
                    Ok(path) => format!(
                        "Configuração do OSD salva em {}. Reinicie o serviço para aplicar.",
                        path.display()
                    ),
                    Err(err) => err,
                });
            }
            Message::OsdSaveAndRestart => {
                self.osd_config = self.osd_config.normalized();
                if let Err(err) = self.osd_config.save() {
                    self.notice = Some(err);
                    return Task::none();
                }
                self.busy = true;
                return Task::future(async {
                    let msg = data::restart_user_unit("logi-osd.service").await;
                    cosmic::action::app(Message::ActionDone(format!(
                        "Configuração salva. {msg}"
                    )))
                });
            }
            Message::OsdReset => {
                self.osd_config = osd_config::Config::default();
                self.notice = Some(match self.osd_config.save() {
                    Ok(_) => "Configuração do OSD restaurada para o padrão.".to_owned(),
                    Err(err) => err,
                });
            }
            Message::OsdRunDemo => {
                let exe = std::env::current_exe()
                    .map(|p| {
                        let s = p.to_string_lossy();
                        s.strip_suffix(" (deleted)").unwrap_or(&s).to_owned()
                    })
                    .unwrap_or_else(|_| "cosmic-taskbar".to_owned());
                self.notice = Some(data::spawn_app(
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
                self.busy = true;
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
                self.busy = true;
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
                self.notice = Some(data::spawn_app(
                    &bin.to_string_lossy(),
                    &[],
                    "Recorte de tela",
                ));
            }
            Message::OpenScreenshotsDir => {
                let dir = self.screenshot_config.resolved_dir();
                _ = std::fs::create_dir_all(&dir);
                self.notice = Some(data::spawn_app(
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
                self.notice = Some("Configurações de recorte (Win+Shift+S) restauradas para o padrão.".to_owned());
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
                self.notice = Some(data::spawn_app(
                    "loginctl",
                    &["lock-session"],
                    "Bloqueio de tela",
                ));
            }
            Message::GreeterSaveOnly => match data::save_greeter_config(&self.greeter) {
                Ok(path) => {
                    self.notice = Some(format!(
                        "Configuração salva em {}. Clique em \"Aplicar na tela de login\" para ativar.",
                        path.display()
                    ));
                }
                Err(err) => self.notice = Some(err),
            },
            Message::GreeterApply => {
                self.busy = true;
                self.notice = Some("Aplicando preferências na tela de login...".to_owned());
                let cfg = self.greeter.clone();
                return Task::future(async move {
                    let msg = data::apply_greeter(cfg).await;
                    cosmic::action::app(Message::ActionDone(msg))
                });
            }
            // Optional packages & Security/Power/Backup actions
            Message::OpenInTerminal(title, cmd) => {
                self.notice = Some(data::open_in_terminal(title, &cmd));
            }
            Message::RunAction(label, cmd, privileged) => {
                self.busy = true;
                self.notice = Some(format!("Executando: {label}..."));
                return Task::future(async move {
                    let msg = data::run_shell_action(label, &cmd, privileged).await;
                    cosmic::action::app(Message::ActionDone(msg))
                });
            }
            Message::SpawnApp(program, args, label) => {
                self.notice = Some(data::spawn_app(program, &args, label));
            }
            Message::EnableUserUnit(unit) => {
                self.busy = true;
                return Task::future(async move {
                    let msg = data::enable_user_unit(unit).await;
                    cosmic::action::app(Message::ActionDone(msg))
                });
            }
            // System & Updates
            Message::CheckUpdates => {
                self.busy = true;
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
                self.busy = true;
                self.notice =
                    Some("Verificando e instalando atualizações do GitHub...".to_owned());
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
                self.busy = true;
                return Task::future(async {
                    let msg = data::export_repo_config().await;
                    cosmic::action::app(Message::ActionDone(msg))
                });
            }
            Message::RestartUserUnit(unit) => {
                self.busy = true;
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
                self.busy = false;
                self.notice = Some(msg);
                self.cpu_topology = power_config::detect_cpu_topology();
                return Task::future(async {
                    cosmic::action::app(Message::SnapshotLoaded(data::load_snapshot().await))
                });
            }
            Message::DismissNotice => {
                self.notice = None;
            }
            Message::ToggleSecurityAdvanced => {
                self.security_show_advanced = !self.security_show_advanced;
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
        Subscription::batch(subs)
    }

    fn view(&self) -> Element<'_, Message> {
        views::view(self)
    }

    fn on_close_requested(&self, _id: window::Id) -> Option<Message> {
        None
    }
}
