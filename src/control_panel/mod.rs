// SPDX-License-Identifier: GPL-3.0-only

//! Full native COSMIC Control Panel application (`cosmic-taskbar control-panel`):
//! centralizes power profiles, battery/USB-C PD telemetry, Lenovo conservation
//! mode, taskbar/tray ordering, left Monitor panel settings, right Central panel
//! settings, multi-camera & physical privacy switches, network/security details,
//! display DDC/visuals, isolated login screen (Greeter), optional packages, and
//! auto-updater/services.

mod data;
mod views;

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
            BarItem as MonitorBarItem, BarMode as MonitorBarMode, Panel as MonitorPanel,
            Settings as MonitorSettings,
        },
        weather::Place,
    },
    services::{
        self, Battery, Camera, DARK_MODE, DO_NOT_DISTURB, Display, Network, Peripheral,
        PowerProfiles, TilingConfig, Touchpad, Update,
    },
    tiles::Tile,
    tray::config::TrayConfig,
};

pub const APP_ID: &str = "io.github.Aenes0.CosmicControlPanel";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Power,
    Bar,
    Monitor,
    Central,
    Camera,
    Network,
    Displays,
    Login,
    Optional,
    System,
}

impl Section {
    pub const ALL: [Section; 10] = [
        Section::Power,
        Section::Bar,
        Section::Monitor,
        Section::Central,
        Section::Camera,
        Section::Network,
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
            Section::Camera => "Câmeras & Privacidade",
            Section::Network => "Rede & Segurança",
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
            Section::Camera => "camera-web-symbolic",
            Section::Network => "network-wired-symbolic",
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
    MonitorShiftPanel(MonitorPanel, bool),
    MonitorToggleHidden(MonitorPanel),
    MonitorToggleFold(MonitorPanel),
    MonitorToggleWeatherExpanded,
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
    RefreshDisplays,
    // Login (Greeter)
    GreeterWallpaperInput(String),
    GreeterTheme(String),
    GreeterToggleClock24h,
    GreeterToggleShowSeconds,
    GreeterBlurRadius(u32),
    GreeterDimOpacity(f32),
    GreeterDefaultUserInput(String),
    GreeterSessionInput(String),
    GreeterToggleIsolatedBinary,
    GreeterSaveOnly,
    GreeterApply,
    // Optional packages
    OpenInTerminal(&'static str, String),
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
    // Login (Greeter)
    greeter: data::GreeterConfig,
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
        let app = Self {
            core,
            section: Section::Power,
            snapshot: data::SystemSnapshot {
                hardware: data::read_hardware(),
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
            greeter: data::load_greeter_config(),
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
                Update::PowerProfiles(p) => self.power = p,
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
            }
            // Power
            Message::SetPowerProfile(profile) => {
                return Task::future(async move {
                    let msg = match services::set_power_profile(profile.clone()).await {
                        Ok(()) => format!("Plano de energia alterado para {profile}."),
                        Err(err) => format!("Falha ao alterar plano: {err}"),
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
            Message::MonitorShiftPanel(panel, down) => {
                self.monitor_settings.shift(panel, down);
            }
            Message::MonitorToggleHidden(panel) => {
                self.monitor_settings.toggle_hidden(panel);
            }
            Message::MonitorToggleFold(panel) => {
                self.monitor_settings.toggle_fold(panel);
            }
            Message::MonitorToggleWeatherExpanded => {
                self.monitor_settings.toggle_weather_expanded();
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
                _ = std::process::Command::new(bin).spawn();
            }
            Message::RefreshDisplays => {
                self.displays = None;
                return Task::future(async {
                    cosmic::action::app(Message::DisplaysLoaded(
                        services::displays(services::DdcRead::All).await,
                    ))
                });
            }
            // Login (Greeter)
            Message::GreeterWallpaperInput(val) => {
                self.greeter.wallpaper = val;
            }
            Message::GreeterTheme(theme) => {
                self.greeter.theme = theme;
            }
            Message::GreeterToggleClock24h => {
                self.greeter.clock_24h = !self.greeter.clock_24h;
            }
            Message::GreeterToggleShowSeconds => {
                self.greeter.show_seconds = !self.greeter.show_seconds;
            }
            Message::GreeterBlurRadius(radius) => {
                self.greeter.blur_radius = radius.min(60);
            }
            Message::GreeterDimOpacity(val) => {
                self.greeter.dim_opacity = ((val * 100.0).round() / 100.0).clamp(0.0, 0.85);
            }
            Message::GreeterDefaultUserInput(val) => {
                self.greeter.default_user = val;
            }
            Message::GreeterSessionInput(val) => {
                self.greeter.session = val;
            }
            Message::GreeterToggleIsolatedBinary => {
                self.greeter.isolated_binary = !self.greeter.isolated_binary;
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
            // Optional packages
            Message::OpenInTerminal(title, cmd) => {
                self.notice = Some(data::open_in_terminal(title, &cmd));
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
                self.greeter = data::load_greeter_config();
                return self.reload_all();
            }
            Message::ActionDone(msg) => {
                self.busy = false;
                self.notice = Some(msg);
                return Task::future(async {
                    cosmic::action::app(Message::SnapshotLoaded(data::load_snapshot().await))
                });
            }
            Message::DismissNotice => {
                self.notice = None;
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
        Subscription::batch(subs)
    }

    fn view(&self) -> Element<'_, Message> {
        views::view(self)
    }

    fn on_close_requested(&self, _id: window::Id) -> Option<Message> {
        None
    }
}
