// SPDX-License-Identifier: GPL-3.0-only

use std::time::Duration;

use cosmic::iced::Length;

use cosmic::{
    Element, Task, app,
    iced::{
        self, Alignment, Limits, Subscription, event,
        platform_specific::shell::commands::layer_surface::destroy_layer_surface, window,
    },
    theme,
    widget::{self, button},
};
use zbus::zvariant::OwnedObjectPath;

use crate::central::{
    pages,
    audio::{self, Mixer, Privacy},
    background::{self, Background},
    tray::components::status_menu,
    bar_spacing::BarSpacing,
    i18n::strings,
    layout::{self, BarItem, Layout, PanelIcons, Section, Show},
    tiles::{Fit, Kind},
    workspaces,
    media::{self, Control, Media},
    notifications::{self, Notification},
    services::{
        self, Battery, Bluetooth, DARK_MODE, DO_NOT_DISTURB, Joined, Network, PowerProfiles,
        SHOW_PERCENTAGE,
        Display, Peripheral, Sink, Update, Volume, WifiNetworks,
    },
    tiles::{Placed, Tile, icon, symbolic},
    transition::Transition,
    panels::{place::Which, service::panel_output},
    slow::{self, Slow},
};

/// Five columns of the grid, the gaps between them, and the panel's padding.
pub const WIDTH: f32 = 404.0;
/// The popup grows with what it shows up to this, until the screen's height
/// is known; past it the notifications scroll.
pub const MAX_HEIGHT: f32 = 800.0;
/// Of the screen, left out of the popup's height: the panel and a little air,
/// the popup taking the rest, as Windows' center.
const SCREEN_MARGIN: f32 = 66.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Main,
    Edit,
    Network,
    Wifi,
    Vpn,
    Bluetooth,
    Power,
    Sound,
    Displays,
    Tiling,
    DoNotDisturb,
    Session,
    Camera,
    /// Dedicated background apps & bar icons page (opened from the `^` shortcut).
    Background,
    /// A background app's own menu.
    TrayMenu,
}

impl Page {
    #[cfg(test)]
    pub const ALL: [Page; 15] = [
        Page::Main,
        Page::Edit,
        Page::Network,
        Page::Wifi,
        Page::Vpn,
        Page::Bluetooth,
        Page::Power,
        Page::Sound,
        Page::Displays,
        Page::Tiling,
        Page::DoNotDisturb,
        Page::Session,
        Page::Camera,
        Page::Background,
        Page::TrayMenu,
    ];

    /// What `from_name` takes.
    pub fn name(self) -> &'static str {
        match self {
            Page::Main => "main",
            Page::Edit => "edit",
            Page::Network => "network",
            Page::Wifi => "wifi",
            Page::Vpn => "vpn",
            Page::Bluetooth => "bluetooth",
            Page::Power => "power",
            Page::Sound => "sound",
            Page::Displays => "displays",
            Page::Tiling => "tiling",
            Page::DoNotDisturb => "dnd",
            Page::Session => "session",
            Page::Camera => "camera",
            Page::Background => "background",
            Page::TrayMenu => "tray",
        }
    }

    pub fn from_name(name: &str) -> Option<Page> {
        Some(match name {
            "main" => Page::Main,
            "edit" => Page::Edit,
            "network" => Page::Network,
            "wifi" => Page::Wifi,
            "vpn" => Page::Vpn,
            "bluetooth" => Page::Bluetooth,
            "power" => Page::Power,
            "sound" => Page::Sound,
            "displays" => Page::Displays,
            "tiling" => Page::Tiling,
            "dnd" => Page::DoNotDisturb,
            "session" => Page::Session,
            "camera" => Page::Camera,
            "background" => Page::Background,
            "tray" => Page::TrayMenu,
            _ => return None,
        })
    }
}

/// How long do not disturb lasts.
#[derive(Clone, Copy, Debug)]
pub enum Dnd {
    Off,
    Forever,
    /// Seconds.
    For(i64),
    /// Until 8 tomorrow morning.
    Tomorrow,
}

/// An item currently being dragged to reorder in `Page::Background`'s edit mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BgDrag {
    App(usize),
    BarItem(BarItem),
}

/// A notification: its id, and whether it is kept from an earlier run, whose
/// ids may be reused.
pub type NotificationKey = (u32, bool);

#[derive(Clone, Debug)]
pub enum Message {
    TogglePopup,
    /// The left button pressed on a window of the applet. On the bar's own,
    /// it opens or closes the popup: taken from the window, not the button,
    /// as after the popup closes the applet may not know where the pointer
    /// is until it moves, and a button then misses the click.
    PanelPress(window::Id),
    /// A press on the applet's area, when embedded in a corner applet: the
    /// same as PanelPress, without a window of its own to tell it by.
    CornerPress,
    /// Opens the popup on a page by itself, for screenshots.
    Preview(Page),
    /// A right click on an icon of the bar: the popup, open on its page.
    OpenAt(Page),
    /// When an icon shows on the bar.
    /// What the background apps say, ask or are asked.
    Tray(background::Msg),
    SetBarIcon(BarItem, Show),
    MoveBarIcon(BarItem, bool),
    BgDragStart(BgDrag),
    BgDragOver(BgDrag),
    BgDragEnd,
    ToggleFold(Section),
    /// An item's resize handle dragged this far across since pressed; let go.
    ResizeDrag(usize, f32),
    ResizeEnd,
    /// The pointer left an item while editing.
    HoverEnd(usize),
    /// While editing: the bubble whose "+" was pressed, where adding goes.
    AddTarget(usize),
    CloseAdding,
    /// A new bubble, or a line in the bubble given.
    AddStructure(Kind, usize),
    /// The calendar a month on or back; back to this month.
    CalendarShift(i32),
    CalendarToday,
    /// Every few seconds, for the clock on the bar.
    ClockTick,
    /// The clipboard service says the history changed (or an action it was
    /// asked for is done): read the file again.
    ClipboardChanged,
    /// The history as read off the UI's thread (none if it took too long);
    /// and whether the panel just opened, to fold for the room again.
    Clipboard(Option<Vec<crate::central::clipboard::Entry>>, bool),
    /// An entry of the clipboard's history, by id: copied again; pinned or
    /// not; deleted. All but the pinned, cleared. Asked of the service.
    CopyEntry(u64),
    PinEntry(u64),
    DeleteEntry(u64),
    ClearClipboard,
    /// Keeps the arrangement edited, or goes back to how it was.
    SaveEdit,
    CancelEdit,
    AddItem(Kind),
    /// The monitor the main brightness slider sets, by connector.
    SetPrimaryDisplay(String),
    Closed(window::Id),
    ShowPage(Page),
    Update(Update),
    SetVolume(f32),
    ToggleMute,
    SetBrightness(f32),
    ToggleWifi,
    ToggleBluetooth,
    NextPowerProfile,
    SetPowerProfile(String),
    ToggleDoNotDisturb,
    ToggleDarkMode,
    ToggleAirplaneMode,
    ToggleMicrophone,
    ToggleKeepAwake,
    ToggleTouchpad,
    ToggleHotEdge,
    /// Hot Edge's own settings app.
    OpenHotEdge,
    /// The camera's service on or off; restarted.
    ToggleCamera,
    RestartCamera,
    /// Asks how the camera is: on opening, and now and then on its page.
    ReadCamera,
    /// How the camera is, read; and after starting, stopping or restarting it.
    Camera(services::Camera),
    CameraDone(services::Camera),
    /// Iriun's window on the computer shown (true) or hidden, by the service.
    IriunWindow(bool),
    /// A screen's name and height.
    Output(Option<String>, Option<i32>),
    /// The panel's screen's height, turned or not, from cosmic-randr.
    ScreenHeight(Option<i32>),
    ToggleShowPercentage,
    SettingsChanged,
    Inhibited(Option<u32>),
    WifiNetworks(Option<WifiNetworks>),
    /// Join the network at this index of the list.
    JoinWifi(usize),
    ConnectBluetooth(OwnedObjectPath, bool),
    /// The last VPN used on, or those on off.
    ToggleVpn,
    /// A VPN, by index, on or off.
    ConnectVpn(usize),
    Sinks(Option<(Vec<Sink>, String)>),
    SetSink(String),
    Displays(Vec<Display>),
    Workspaces(workspaces::Event),
    ToggleTiling,
    SetNewWorkspacesTile(bool),
    ToggleActiveHint,
    A11y(services::A11y),
    ScreenReader(cosmic_settings_accessibility_subscription::Response),
    ToggleMagnifier,
    ToggleInvertColors,
    ToggleScreenReader,
    ToggleHighContrast,
    OpenWorkspaces,
    Media(Vec<Media>),
    SelectPlayer(String),
    /// The audio outputs, in the media card.
    ToggleMediaOutputs,
    MediaControl(Control),
    /// Dragging the progress bar, in microseconds, then letting go.
    Seek(f32),
    SeekDone,
    /// Every second while something plays, for the progress bar.
    Tick,
    /// The notifications service wrote the history down: read it again.
    NotificationsChanged,
    OpenNotification(NotificationKey),
    DismissNotification(NotificationKey),
    NotificationAction(u32, String),
    ClearNotifications,
    /// Show all of an app's notifications, or its newest only.
    ToggleGroup(String),
    SetDoNotDisturb(Dnd),
    CheckDoNotDisturb,
    Mixer(Mixer),
    /// An app, by name in the mixer, and its volume in percent.
    SetStreamVolume(String, f32),
    ToggleStreamMute(String),
    /// A volume slider let go.
    AdjustDone,
    SetSource(String),
    SetInputVolume(f32),
    ToggleInputMute,
    Privacy(Privacy),
    /// Scrolling over the volume or battery icon, in steps.
    ScrollVolume(f32),
    ScrollBrightness(f32),
    Suspend,
    Restart,
    LogOut,
    /// A display, by index, and its new scale.
    SetScale(usize, f32),
    /// A display's brightness over DDC while dragging, then when let go.
    SetDdcBrightness(usize, f32),
    ApplyDdcBrightness(usize),
    /// A tile pressed while editing; dragged over another; let go.
    DragStart(usize),
    DragOver(usize),
    DragEnd,
    RemoveTile(usize),
    ResetTiles,
    /// How far apart the icons on the panel are.
    SetBarSpacing(BarSpacing),
    /// A page of COSMIC Settings, or all of it.
    Settings(Option<&'static str>),
    Screenshot,
    Lock,
    PowerOff,
    None,
}

/// A tile being dragged to a new place, and whether it moved at all: one
/// that didn't was only clicked.
#[derive(Clone, Copy, Debug)]
pub struct Drag {
    pub index: usize,
    pub moved: bool,
}

#[derive(Default)]
pub struct App {
    core: app::Core,
    popup: Option<window::Id>,
    page: Option<Page>,
    pub(crate) tiles: Vec<Placed>,
    /// The tile being moved or resized while editing.
    pub(crate) selected_tile: Option<usize>,
    pub(crate) drag: Option<Drag>,
    pub(crate) network: Network,
    pub(crate) wifi_networks: Option<WifiNetworks>,
    pub(crate) bluetooth: Bluetooth,
    pub(crate) battery: Option<Battery>,
    pub(crate) peripherals: Vec<Peripheral>,
    pub(crate) volume: Option<Volume>,
    pub(crate) microphone: Option<bool>,
    pub(crate) sinks: Option<(Vec<Sink>, String)>,
    pub(crate) displays: Option<Vec<Display>>,
    pub(crate) workspace_tiling: Option<bool>,
    workspaces_tx: Option<cosmic::cctk::sctk::reexports::calloop::channel::Sender<bool>>,
    pub(crate) tiling_config: services::TilingConfig,
    a11y_tx: Option<cosmic::cctk::sctk::reexports::calloop::channel::Sender<services::AccessibilityRequest>>,
    pub(crate) magnifier: Option<bool>,
    /// Inverted colors, and the color filter on top, kept as they are.
    pub(crate) screen_filter: Option<(bool, Option<services::ColorFilter>)>,
    pub(crate) screen_reader: Option<bool>,
    screen_reader_tx: Option<tokio::sync::mpsc::UnboundedSender<cosmic_settings_accessibility_subscription::Request>>,
    /// Playing first.
    pub(crate) media: Vec<Media>,
    /// The player picked, by service.
    pub(crate) player: Option<String>,
    pub(crate) media_outputs: bool,
    pub(crate) mixer: Option<Mixer>,
    pub(crate) privacy: Privacy,
    pub(crate) dnd_until: Option<i64>,
    /// Apps whose notifications all show.
    pub(crate) expanded: std::collections::HashSet<String>,
    pub(crate) seeking: Option<f32>,
    /// Newest first.
    pub(crate) notifications: Vec<Notification>,
    pub(crate) brightness: Option<(i32, i32)>,
    pub(crate) power: Option<PowerProfiles>,
    pub(crate) do_not_disturb: bool,
    pub(crate) dark_mode: bool,
    pub(crate) show_percentage: bool,
    /// Held while keeping the computer awake.
    pub(crate) inhibit: Option<u32>,
    pub(crate) touchpad: Option<services::Touchpad>,
    pub(crate) hot_edge: bool,
    pub(crate) camera: services::Camera,
    /// Starts, stops or restarts of the camera not done yet: what's read
    /// meanwhile is from before, and would undo what the tile shows.
    camera_busy: usize,
    pub(crate) bar_spacing: BarSpacing,
    pub(crate) panel_icons: PanelIcons,
    pub(crate) layout: Layout,
    pub(crate) primary_display: Option<String>,
    /// The item being resized by its handle, and its size when grabbed.
    pub(crate) resizing: Option<(usize, Fit)>,
    /// What was copied, newest first.
    pub(crate) clipboard: Vec<crate::central::clipboard::Entry>,
    /// The month the calendar shows.
    pub(crate) calendar_month: crate::central::calendar::Month,
    /// The clock's lines as last drawn: time and date.
    clock: (String, String),
    /// When COSMIC last closed the popup, a click outside it.
    closed_at: Option<std::time::Instant>,
    /// Embedded in the corner applet: the press comes by area, not window.
    corner: bool,
    /// When the monitors' brightness was last read, on opening.
    displays_read: Option<std::time::Instant>,
    /// The item under the pointer while editing: it shows its handle.
    pub(crate) hovered: Option<usize>,
    /// The arrangement as it was when editing began: back to it on cancel.
    before_edit: Option<Vec<Placed>>,
    /// The bubble the sidebar adds to.
    pub(crate) add_target: usize,
    /// While editing, whether the floating panel of what can be added shows.
    pub(crate) adding: bool,
    /// The height of the screen the panel is on, in logical pixels.
    screen_height: Option<i32>,
    /// Whether that came from cosmic-randr, which knows a screen standing.
    screen_height_known: bool,
    /// Run by the panels' host, which owns the surface the panel shows on;
    /// else the bar's applet, which asks the host to show it.
    hosted: bool,
    /// When a volume or brightness slider last moved. What PipeWire or the
    /// settings daemon tell meanwhile is the slider's own change, from a
    /// moment before: it would pull the slider back, and reload the list
    /// under it.
    adjusting: Option<std::time::Instant>,
    /// The background apps: their icons on the bar and in the panel, and
    /// their menus as a page of it.
    pub(crate) background: Background,
    /// Whether the dedicated `Page::Background` is currently in edit mode.
    pub(crate) background_editing: bool,
    /// Active drag-and-drop reorder state in `Page::Background`'s edit mode.
    pub(crate) bg_drag: Option<BgDrag>,
    /// System bar icons that overflowed from the bar into the `^` popup on the
    /// active monitor (passed via `"background:id1,id2"`).
    pub(crate) overflow_bar_items: Vec<BarItem>,
    /// Where the back button in `Page::TrayMenu` returns to (`Main` by default).
    pub(crate) tray_menu_return: Option<Page>,
    /// The page (or `tray:<key>`) that was open right before closing (so the
    /// host can toggle any icon closed on a second click).
    last_closed_page: Option<String>,
    /// When an icon of a background app (or the `^` arrow) was last pressed on the bar.
    tray_pressed: Option<std::time::Instant>,
}

impl App {
    /// The button for a corner applet to hold: the press is by area.
    pub(crate) fn corner_view(&self) -> Element<'_, Message> {
        self.bar_button(true)
    }

    /// System bar items that can be shown and reordered on the bar, including
    /// the clock (so items like notifications or power can be placed after it).
    pub(crate) fn reorderable_bar_items(&self) -> Vec<BarItem> {
        let mut items = vec![BarItem::Privacy, BarItem::Notifications];
        for tile in Tile::ALL {
            if tile != Tile::Wifi && tile != Tile::Power && self.tile_state(tile).is_some() {
                items.push(BarItem::Tile(tile));
            }
        }
        items.push(BarItem::Network);
        items.push(BarItem::Volume);
        items.push(BarItem::Peripherals);
        if self.battery.is_some() {
            items.push(BarItem::Battery);
        }
        if self.tile_state(Tile::Power).is_some() {
            items.push(BarItem::Tile(Tile::Power));
        }
        items.push(BarItem::Clock);
        items
    }

    /// Estimates how many pixels a rendered system bar item needs along the
    /// panel's main axis, or `None` if it is currently not visible on the bar.
    fn bar_item_extent(&self, item: BarItem, active_tiles: &std::collections::HashMap<Tile, crate::central::tiles::TileState>) -> Option<f32> {
        let applet = &self.core.applet;
        let horizontal = applet.is_horizontal();
        let size = f32::from(applet.suggested_size(true).0);
        let major = f32::from(applet.suggested_padding(true).0);
        let icon_unit = size + 8.0;
        let text_extra = if horizontal { 34.0 } else { 18.0 };
        let shows = |it: BarItem| self.panel_icons.get(it);
        match item {
            BarItem::Privacy => {
                if shows(BarItem::Privacy) == Show::Never {
                    return None;
                }
                let count = usize::from(!self.privacy.camera.is_empty())
                    + usize::from(!self.privacy.microphone.is_empty());
                (count > 0).then_some(count as f32 * icon_unit)
            }
            BarItem::Notifications => {
                let new = self.new_count();
                let bell = match shows(BarItem::Notifications) {
                    Show::Always => true,
                    Show::Active => new > 0,
                    Show::Never => false,
                };
                bell.then_some(if new > 0 { icon_unit + 18.0 } else { icon_unit })
            }
            BarItem::Tile(tile) => active_tiles.contains_key(&tile).then_some(icon_unit),
            BarItem::Network => (shows(BarItem::Network) != Show::Never).then_some(icon_unit),
            BarItem::Volume => (self.volume.is_some() && shows(BarItem::Volume) != Show::Never)
                .then_some(icon_unit),
            BarItem::Peripherals => {
                if shows(BarItem::Peripherals) == Show::Never || self.peripherals.is_empty() {
                    return None;
                }
                Some(self.peripherals.len() as f32 * (icon_unit + text_extra))
            }
            BarItem::Battery => {
                let _ = self.battery.as_ref().filter(|_| shows(BarItem::Battery) != Show::Never)?;
                Some(if self.show_percentage { icon_unit + text_extra } else { icon_unit })
            }
            BarItem::Clock => (shows(BarItem::Clock) != Show::Never).then_some(if horizontal {
                76.0 + 2.0 * major
            } else {
                36.0 + 2.0 * major
            }),
        }
    }

    /// Computes which SNI apps and system bar items fit in the panel's
    /// `suggested_bounds`, always preserving `^`, Clock, and ShowDesktop;
    /// anything that doesn't fit overflows into the `^` popup.
    fn bar_plan(
        &self,
        in_corner: bool,
        active_tiles: &std::collections::HashMap<Tile, crate::central::tiles::TileState>,
    ) -> (std::collections::HashSet<usize>, std::collections::HashSet<BarItem>, Vec<BarItem>) {
        let applet = &self.core.applet;
        let horizontal = applet.is_horizontal();
        let size = f32::from(applet.suggested_size(true).0);
        let item_gap = f32::from((self.bar_spacing.gap() / 2).max(1));
        let icon_unit = size + 8.0;

        let sni_ids: Vec<usize> = self.background.on_bar().into_iter().map(|(id, _, _)| id).collect();
        let ordered_items = self.panel_icons.ordered(&self.reorderable_bar_items());

        let max_len = applet
            .suggested_bounds
            .as_ref()
            .map(|b| if horizontal { b.width } else { b.height })
            .filter(|&v| v > 0.0)
            .map(|v| (v - if in_corner { 12.0 } else { 0.0 }).max(0.0));

        let Some(max_len) = max_len else {
            return (
                sni_ids.into_iter().collect(),
                ordered_items.into_iter().collect(),
                Vec::new(),
            );
        };

        // Always reserve room for `^` and the Clock (when enabled).
        let mut used = icon_unit;
        if let Some(clock_len) = self.bar_item_extent(BarItem::Clock, active_tiles) {
            used += item_gap + clock_len;
        }

        let mut fit_sni = std::collections::HashSet::new();
        for id in sni_ids {
            if used + item_gap + icon_unit <= max_len {
                used += item_gap + icon_unit;
                fit_sni.insert(id);
            }
        }

        let mut fit_items = std::collections::HashSet::new();
        let mut overflow_items = Vec::new();
        for item in ordered_items {
            if item == BarItem::Clock {
                fit_items.insert(item);
                continue;
            }
            let Some(extent) = self.bar_item_extent(item, active_tiles) else {
                continue;
            };
            if used + item_gap + extent <= max_len {
                used += item_gap + extent;
                fit_items.insert(item);
            } else {
                overflow_items.push(item);
            }
        }

        (fit_sni, fit_items, overflow_items)
    }

    /// The page page-string for `^`, encoding any system bar items that
    /// overflowed on this monitor so `central --panels` shows them inside `^`.
    fn background_wire_page(&self) -> String {
        let active_tiles: std::collections::HashMap<Tile, crate::central::tiles::TileState> =
            self.bar_tiles().into_iter().collect();
        let (_, _, overflow) = self.bar_plan(self.corner, &active_tiles);
        if overflow.is_empty() {
            Page::Background.name().to_owned()
        } else {
            let ids: Vec<String> = overflow.into_iter().map(BarItem::id).collect();
            format!("{}:{}", Page::Background.name(), ids.join(","))
        }
    }

    fn apply_background_overflow(&mut self, page: &str) {
        self.overflow_bar_items = page
            .strip_prefix("background:")
            .map(|list| {
                list.split(',')
                    .filter_map(BarItem::from_id)
                    .filter(|item| *item != BarItem::Clock)
                    .collect()
            })
            .unwrap_or_default();
    }

    /// The bar's button: individual icon buttons, and the clock beside them
    /// (which alone opens the Central panel).
    fn bar_button(&self, in_corner: bool) -> Element<'_, Message> {
        let applet = &self.core.applet;
        let size = applet.suggested_size(true).0;
        let (major, minor) = applet.suggested_padding(true);
        let colored = |name: &str, color: Option<fn(&cosmic::Theme) -> iced::Color>| -> Element<'_, Message> {
            widget::icon::icon(icon(name))
                .size(size)
                .class(theme::Svg::Custom(std::rc::Rc::new(move |theme| {
                    iced::widget::svg::Style {
                        color: Some(color.map_or_else(
                            || theme.cosmic().background(theme.transparent).on.into(),
                            |color| color(theme),
                        )),
                    }
                })))
                .into()
        };
        let panel_icon = |name: &str| colored(name, None);
        // On a panel at a side of the screen, the icons go in a column, and
        // what an icon says goes under it, smaller, to fit the panel's width.
        let horizontal = applet.is_horizontal();
        let with_text = |icon: Element<'static, Message>, text: String, accent: bool| -> Element<'static, Message> {
            let label = if horizontal {
                applet.text(text)
            } else {
                widget::text::caption(text)
            };
            let label = if accent { label.class(theme::Text::Accent).into() } else { label.into() };
            if horizontal {
                widget::row::with_children(vec![icon, label])
                    .spacing(2)
                    .align_y(Alignment::Center)
                    .into()
            } else {
                widget::column::with_children(vec![icon, label])
                    .align_x(Alignment::Center)
                    .into()
            }
        };

        let icon_pad = if horizontal { [minor, 4] } else { [4, minor] };
        let icon_btn = |content: Element<'static, Message>, on_press: Message| -> Element<'static, Message> {
            button::custom(content)
                .padding(icon_pad)
                .class(theme::Button::AppletIcon)
                .on_press_down(on_press)
                .into()
        };
        // Each system icon has its own hover highlight and opens its own
        // dedicated page on left or right click (never the main Central panel;
        // only the clock opens Central). Clicking the same icon again toggles
        // its popup closed.
        let at = |content: Element<'static, Message>, page: Page| -> Element<'static, Message> {
            widget::mouse_area(icon_btn(content, Message::OpenAt(page)))
                .on_right_press(Message::OpenAt(page))
                .into()
        };

        let mut icons: Vec<Element<'_, Message>> = Vec::with_capacity(12);
        let shows = |item: BarItem| self.panel_icons.get(item);

        // Discreet arrow shortcut (`^`) to open the dedicated background apps
        // and bar icons page in `central --panels` (zero xdg_popup).
        let arrow_icon = match applet.anchor {
            cosmic::applet::cosmic_panel_config::PanelAnchor::Bottom => "go-up-symbolic",
            cosmic::applet::cosmic_panel_config::PanelAnchor::Top => "go-down-symbolic",
            cosmic::applet::cosmic_panel_config::PanelAnchor::Left => "go-next-symbolic",
            cosmic::applet::cosmic_panel_config::PanelAnchor::Right => "go-previous-symbolic",
        };
        let arrow_glyph: Element<'static, Message> = widget::container(
            widget::icon::icon(icon(arrow_icon))
                .size((size * 3 / 4).max(12))
                .class(theme::Svg::Custom(std::rc::Rc::new(|theme| {
                    iced::widget::svg::Style {
                        color: Some(theme.cosmic().background(theme.transparent).on.into()),
                    }
                }))),
        )
        .center(Length::Fixed(f32::from(size)))
        .into();
        let arrow = widget::mouse_area(icon_btn(
            arrow_glyph,
            Message::Tray(background::Msg::BarArrow),
        ))
        .on_right_press(Message::Tray(background::Msg::BarArrow));
        icons.push(arrow.into());

        // Compute which icons fit in the panel's `suggested_bounds`; `^`,
        // Clock, and ShowDesktop are always preserved, and overflowed icons
        // move into the `^` popup automatically.
        let active_tiles: std::collections::HashMap<Tile, crate::central::tiles::TileState> =
            self.bar_tiles().into_iter().collect();
        let (fit_sni, fit_items, _overflow) = self.bar_plan(in_corner, &active_tiles);

        // The background apps picked for the bar: each with its own hover
        // highlight, left click activates/opens menu, right click opens its menu.
        for (id, handle, _name) in self.background.on_bar() {
            if !fit_sni.contains(&id) {
                continue;
            }
            let symbolic = handle.symbolic;
            let glyph = widget::icon::icon(handle).size(size);
            let glyph = if symbolic {
                glyph.class(theme::Svg::Custom(std::rc::Rc::new(|theme| {
                    iced::widget::svg::Style {
                        color: Some(theme.cosmic().background(theme.transparent).on.into()),
                    }
                })))
            } else {
                glyph
            };
            let area = widget::mouse_area(icon_btn(
                glyph.into(),
                Message::Tray(background::Msg::BarPress(id)),
            ))
            .on_right_press(Message::Tray(background::Msg::OpenMenu(id)));
            icons.push(area.into());
        }

        // System icons in the user's chosen order (skipping any that overflowed into `^`).
        for item in self.panel_icons.ordered(&self.reorderable_bar_items()) {
            if !fit_items.contains(&item) {
                continue;
            }
            match item {
                BarItem::Privacy => {
                    if shows(BarItem::Privacy) != Show::Never {
                        let privacy = &self.privacy;
                        if !privacy.camera.is_empty() {
                            let green: fn(&cosmic::Theme) -> iced::Color =
                                |theme| theme.cosmic().success_color().into();
                            icons.push(at(
                                colored("camera-web-symbolic", Some(green)),
                                Page::Camera,
                            ));
                        }
                        if !privacy.microphone.is_empty() {
                            let orange: fn(&cosmic::Theme) -> iced::Color =
                                |theme| theme.cosmic().warning_color().into();
                            icons.push(at(
                                colored("audio-input-microphone-symbolic", Some(orange)),
                                Page::Sound,
                            ));
                        }
                    }
                }
                BarItem::Notifications => {
                    let new = self.new_count();
                    let bell = match shows(BarItem::Notifications) {
                        Show::Always => true,
                        Show::Active => new > 0,
                        Show::Never => false,
                    };
                    if bell {
                        if new == 0 {
                            icons.push(at(panel_icon("notification-symbolic"), Page::Main));
                        } else {
                            let accent: fn(&cosmic::Theme) -> iced::Color =
                                |theme| theme.cosmic().accent_color().into();
                            icons.push(at(
                                with_text(
                                    colored("notification-new-symbolic", Some(accent)),
                                    new.to_string(),
                                    true,
                                ),
                                Page::Main,
                            ));
                        }
                    }
                }
                BarItem::Tile(Tile::Power) => {
                    if let Some(state) = active_tiles.get(&Tile::Power) {
                        let color: fn(&cosmic::Theme) -> iced::Color =
                            match self.power.as_ref().map(|p| p.active.as_str()) {
                                Some("power-saver") => |_| iced::Color::from_rgb(0.30, 0.82, 0.50),
                                Some("performance") => |_| iced::Color::from_rgb(0.95, 0.35, 0.35),
                                _ => |_| iced::Color::from_rgb(0.96, 0.78, 0.25),
                            };
                        let glyph = size * 7 / 8;
                        let content: Element<'static, Message> = widget::container(
                            widget::icon::icon(icon(state.icon))
                                .size(glyph)
                                .class(theme::Svg::Custom(std::rc::Rc::new(move |th| {
                                    iced::widget::svg::Style { color: Some(color(th)) }
                                }))),
                        )
                        .center(Length::Fixed(f32::from(size)))
                        .into();
                        icons.push(at(content, Page::Power));
                    }
                }
                BarItem::Tile(tile) => {
                    if let Some(state) = active_tiles.get(&tile) {
                        let page = match tile {
                            Tile::Bluetooth => Page::Bluetooth,
                            Tile::Vpn => Page::Vpn,
                            Tile::Power => Page::Power,
                            Tile::DoNotDisturb => Page::DoNotDisturb,
                            Tile::Tiling => Page::Tiling,
                            Tile::Camera => Page::Camera,
                            _ => Page::Main,
                        };
                        icons.push(at(panel_icon(state.icon), page));
                    }
                }
                BarItem::Network => {
                    if shows(BarItem::Network) != Show::Never {
                        icons.push(at(panel_icon(self.network_icon()), Page::Network));
                    }
                }
                BarItem::Volume => {
                    if self.volume.is_some() && shows(BarItem::Volume) != Show::Never {
                        let area = widget::mouse_area(icon_btn(
                            panel_icon(self.volume_icon()),
                            Message::OpenAt(Page::Sound),
                        ))
                        .on_right_press(Message::OpenAt(Page::Sound))
                        .on_scroll(|delta| Message::ScrollVolume(scroll_steps(delta)))
                        .on_middle_press(Message::ToggleMute);
                        icons.push(area.into());
                    }
                }
                BarItem::Peripherals => {
                    if shows(BarItem::Peripherals) != Show::Never {
                        for peripheral in &self.peripherals {
                            icons.push(at(
                                with_text(
                                    panel_icon(&peripheral.icon),
                                    format!("{:.0}%", peripheral.percentage),
                                    false,
                                ),
                                Page::Power,
                            ));
                        }
                    }
                }
                BarItem::Battery => {
                    if let Some(battery) = self
                        .battery
                        .as_ref()
                        .filter(|_| shows(BarItem::Battery) != Show::Never)
                    {
                        let glyph = panel_icon(&symbolic(&battery.icon));
                        let content = if self.show_percentage {
                            with_text(glyph, format!("{:.0}%", battery.percentage), false)
                        } else {
                            glyph
                        };
                        let area = widget::mouse_area(icon_btn(content, Message::OpenAt(Page::Power)))
                            .on_right_press(Message::OpenAt(Page::Power))
                            .on_scroll(|delta| Message::ScrollBrightness(scroll_steps(delta)));
                        icons.push(area.into());
                    }
                }
                BarItem::Clock => {
                    if shows(BarItem::Clock) != Show::Never {
                        let (time, date) = if self.clock.0.is_empty() {
                            crate::central::calendar::bar_lines()
                        } else {
                            self.clock.clone()
                        };
                        let tight = iced::widget::text::LineHeight::Relative(1.05);
                        let lines: Element<'_, Message> = widget::column::with_children(vec![
                            widget::text::body(time).line_height(tight).into(),
                            widget::text::caption(date).line_height(tight).into(),
                        ])
                        .align_x(Alignment::Center)
                        .into();
                        let button = button::custom(lines)
                            .padding(if horizontal { [0, major] } else { [major, 0] })
                            .class(theme::Button::AppletIcon);
                        let clock_el: Element<'_, Message> = if in_corner {
                            button.on_press_down(Message::CornerPress).into()
                        } else {
                            button.on_press(Message::None).into()
                        };
                        icons.push(clock_el);
                    }
                }
            }
        }

        let item_gap = (self.bar_spacing.gap() / 2).max(1);
        if horizontal {
            widget::row::with_children(icons)
                .spacing(item_gap)
                .align_y(Alignment::Center)
                .into()
        } else {
            widget::column::with_children(icons)
                .spacing(item_gap)
                .align_x(Alignment::Center)
                .into()
        }
    }

    pub(crate) fn high_contrast(&self) -> bool {
        self.core.system_theme().cosmic().is_high_contrast
    }

    pub(crate) fn airplane_mode(&self) -> bool {
        !self.network.wifi_enabled && (self.bluetooth.adapter.is_none() || !self.bluetooth.powered)
    }

    pub(crate) fn network_icon(&self) -> &'static str {
        match &self.network.connection {
            Some((_, kind)) if kind == "802-3-ethernet" => "network-wired-symbolic",
            Some((_, kind)) if kind == "802-11-wireless" => "network-wireless-signal-good-symbolic",
            Some(_) => "network-vpn-symbolic",
            None if self.airplane_mode() => "airplane-mode-symbolic",
            None if !self.network.wifi_enabled => "network-wireless-disabled-symbolic",
            None => "network-wireless-offline-symbolic",
        }
    }

    pub(crate) fn volume_icon(&self) -> &'static str {
        match &self.volume {
            None => "audio-volume-muted-symbolic",
            Some(v) if v.muted || v.level <= 0.0 => "audio-volume-muted-symbolic",
            Some(v) if v.level < 0.34 => "audio-volume-low-symbolic",
            Some(v) if v.level < 0.67 => "audio-volume-medium-symbolic",
            Some(_) => "audio-volume-high-symbolic",
        }
    }

    /// Whether a notification came since the popup was last open.
    pub(crate) fn is_new(&self, notification: &Notification) -> bool {
        !notification.restored && !notification.seen
    }

    /// The player shown: the one picked, else the first, playing.
    pub(crate) fn shown_media(&self) -> Option<&Media> {
        self.player
            .as_ref()
            .and_then(|service| self.media.iter().find(|m| &m.service == service))
            .or(self.media.first())
    }

    /// Out of the list at once; the service forgets it for every applet.
    fn remove_notification(&mut self, key: NotificationKey) -> app::Task<Message> {
        notifications::remove(&mut self.notifications, key.0, key.1);
        notifications_ask(notifications::Action::Remove(key.0, key.1))
    }

    pub(crate) fn new_count(&self) -> usize {
        self.notifications.iter().filter(|n| self.is_new(n)).count()
    }

    fn stream_mut(&mut self, app: &str) -> Option<&mut audio::Stream> {
        self.mixer
            .as_mut()?
            .streams
            .iter_mut()
            .find(|stream| stream.app == app)
    }

    fn adjusting(&self) -> bool {
        self.adjusting.is_some_and(|at| at.elapsed() < Duration::from_secs(2))
    }

    /// How tall the popup can be: as the screen allows.
    pub fn max_height(&self) -> f32 {
        self.screen_height
            .map_or(MAX_HEIGHT, |height| (height as f32 - SCREEN_MARGIN).max(480.0))
    }

    fn close_popup(&mut self) -> app::Task<Message> {
        let seen = self.mark_seen();
        self.leave_edit(false);
        self.last_closed_page = self.page_name();
        self.background.close();
        self.background_editing = false;
        self.bg_drag = None;
        self.tray_menu_return = None;
        self.page = None;
        let destroy = match self.popup.take() {
            Some(id) => destroy_layer_surface(id),
            None => Task::none(),
        };
        Task::batch([seen, destroy])
    }

    /// The page currently shown, by its wire name (including `tray:<key>`).
    pub(crate) fn page_name(&self) -> Option<String> {
        if self.page == Some(Page::TrayMenu) {
            return self
                .background
                .open
                .and_then(|id| self.background.key_of(id))
                .map(|key| format!("tray:{key}"));
        }
        self.page.map(|p| p.name().to_owned())
    }

    /// The page that was shown right before the panel closed.
    pub(crate) fn last_page_name(&self) -> Option<String> {
        self.last_closed_page.clone()
    }

    /// The panel, for the panels' host: never on the bar, never opening by
    /// itself; what it read at start is all it waits for.
    pub(crate) fn hosted(core: app::Core) -> Self {
        let (mut app, _) = <Self as cosmic::Application>::init(core, ());
        app.hosted = true;
        app
    }

    /// The host's surface it shows on, while it shows.
    pub(crate) fn surface(&self) -> Option<window::Id> {
        self.popup
    }

    /// Shows on the host's surface `id`, on a page or the main one, on a
    /// screen this tall. Nothing slow is waited for: it comes after.
    pub(crate) fn show_hosted(&mut self, id: window::Id, page: Option<&str>, screen_height: Option<i32>) -> app::Task<Message> {
        if screen_height.is_some() {
            self.screen_height = screen_height;
            self.screen_height_known = true;
        }
        self.popup = Some(id);
        self.background_editing = false;
        self.tray_menu_return = None;
        let opened = self.opened();
        // On an app's menu, as a click on its icon on the bar: by the app's
        // key, the ids here being each process's own.
        if let Some(app) = page
            .and_then(|page| page.strip_prefix("tray:"))
            .and_then(|key| self.background.id_of(key))
        {
            self.page = Some(Page::TrayMenu);
            self.background.open_menu(app);
            return opened;
        }
        // Dedicated `^` page, optionally carrying overflowed system bar icons.
        if let Some(p) = page.filter(|p| *p == Page::Background.name() || p.starts_with("background:")) {
            self.apply_background_overflow(p);
            self.page = Some(Page::Background);
            return opened;
        }
        self.overflow_bar_items.clear();
        // Opened on a page, as a right click on the bar's icon, or a preview
        // (what it picks or adds while editing kept).
        match page.and_then(Page::from_name).filter(|page| *page != Page::Main) {
            Some(page) => {
                self.page = Some(page);
                Task::batch([opened, self.load(page)])
            }
            None => opened,
        }
    }

    /// Hides from the host's surface, and has it destroyed.
    pub(crate) fn close_hosted(&mut self) -> app::Task<Message> {
        self.close_popup()
    }

    /// What the host runs once at start, as init would: the brightness,
    /// read a while later, in the background.
    pub(crate) fn warm_up(&self) -> app::Task<Message> {
        read_brightness(BRIGHTNESS_AFTER)
    }

    /// What stays connected in the host while the panel is closed too, so
    /// as not to connect again at each opening: the accessibility protocol,
    /// when a tile needs it.
    pub(crate) fn lasting_subscription(&self) -> Subscription<Message> {
        // The background apps' icons stay known, for the bar and the strip.
        let tray = self.background.subscription(&self.core, self.tray_menu_shown()).map(Message::Tray);
        if self.wants_a11y() {
            Subscription::batch([services::a11y_subscription().map(Message::A11y), tray])
        } else {
            tray
        }
    }

    /// The app whose menu shows, where it does: on the panel's own page.
    fn tray_menu_shown(&self) -> Option<usize> {
        self.background
            .open
            .filter(|_| self.hosted && self.page == Some(Page::TrayMenu))
    }

    /// Whether an icon of a background app was just pressed on the bar: the
    /// press of the bar's own, which opens the panel, is not for this.
    fn tray_just_pressed(&self) -> bool {
        self.tray_pressed.is_some_and(|at| at.elapsed() < Duration::from_millis(250))
    }

    /// An app's menu, a page of the panel that shows.
    fn open_tray_menu(&mut self, app: usize) -> app::Task<Message> {
        if self.page != Some(Page::TrayMenu) {
            self.tray_menu_return = self.page;
        }
        self.background.open_menu(app);
        cosmic::Application::update(self, Message::ShowPage(Page::TrayMenu))
    }

    /// What the background apps say, ask or are asked.
    fn tray(&mut self, message: background::Msg) -> app::Task<Message> {
        use background::Msg;
        match message {
            Msg::Watcher(event) => {
                let task = self.background.event(event);
                // The app whose menu shows went.
                if self.page == Some(Page::TrayMenu) && self.background.open.is_none() {
                    self.page = Some(self.tray_menu_return.unwrap_or(Page::Main));
                }
                task
            }
            Msg::Menu((_, status_menu::Msg::CloseMenu)) => self.close_popup(),
            Msg::Menu((id, message)) => self.background.menu(id, message),
            Msg::Identity(id, key, title) => {
                self.background.learn(id, &key, &title);
                Task::none()
            }
            Msg::Config(config) => {
                self.background.set_config(config);
                Task::none()
            }
            Msg::Token(update) => self.background.token(update),
            Msg::BarPress(id) => {
                self.tray_pressed = Some(std::time::Instant::now());
                self.tray(Msg::Activate(id))
            }
            Msg::BarArrow => {
                self.tray_pressed = Some(std::time::Instant::now());
                if !self.hosted {
                    return ask_host(crate::central::panels::service::open(
                        Which::Central,
                        panel_output(),
                        self.background_wire_page(),
                    ));
                }
                if self.popup.is_some() {
                    if self.page == Some(Page::Background) {
                        return self.close_popup();
                    }
                    return cosmic::Application::update(self, Message::ShowPage(Page::Background));
                }
                Task::none()
            }
            Msg::OpenEdit => {
                let task = cosmic::Application::update(self, Message::ShowPage(Page::Background));
                self.background_editing = true;
                task
            }
            Msg::ToggleEdit => {
                self.background_editing = !self.background_editing;
                Task::none()
            }
            Msg::Activate(id) => match self.background.is_menu(id) {
                None => Task::none(),
                Some(true) => self.tray(Msg::OpenMenu(id)),
                Some(false) => {
                    let activate = self.background.activate(id);
                    if self.hosted {
                        Task::batch([activate, self.close_popup()])
                    } else {
                        activate
                    }
                }
            },
            Msg::OpenMenu(id) => {
                if self.popup.is_some() {
                    return self.open_tray_menu(id);
                }
                // From the bar: the panels' host shows it, on the app's key.
                let Some(key) = self.background.key_of(id).map(str::to_owned) else {
                    return Task::none();
                };
                if self.hosted {
                    return Task::none();
                }
                ask_host(crate::central::panels::service::open(
                    Which::Central,
                    panel_output(),
                    format!("tray:{key}"),
                ))
            }
            Msg::Toggle(id) => {
                self.background.toggle(id);
                Task::none()
            }
            Msg::Move(id, up) => {
                self.background.move_item(id, up);
                Task::none()
            }
        }
    }

    /// Whether the accessibility protocol is needed: binding it held the
    /// compositor's rendering for up to ~170 ms (measured), so only for the
    /// tiles that show what it tells.
    fn wants_a11y(&self) -> bool {
        wants_a11y(self.hosted, &self.tiles, |tile| self.panel_icons.get(BarItem::Tile(tile)) != Show::Never)
    }

    /// Turns to a page while it shows; asking for the same page (or same tray
    /// app menu) that is already open toggles the popup closed.
    pub(crate) fn turn_to(&mut self, page: &str) -> app::Task<Message> {
        if page == Page::Background.name() || page.starts_with("background:") {
            if self.page == Some(Page::Background) {
                return self.close_popup();
            }
            self.apply_background_overflow(page);
            return <Self as cosmic::Application>::update(self, Message::ShowPage(Page::Background));
        }
        if self.page_name().as_deref() == Some(page) {
            return self.close_popup();
        }
        if let Some(key) = page.strip_prefix("tray:") {
            return match self.background.id_of(key) {
                Some(app) => self.open_tray_menu(app),
                None => Task::none(),
            };
        }
        match Page::from_name(page) {
            Some(page) => <Self as cosmic::Application>::update(self, Message::ShowPage(page)),
            None => Task::none(),
        }
    }

    /// What opening reads, the panel on its main page.
    fn opened(&mut self) -> app::Task<Message> {
        self.page = Some(Page::Main);
        self.calendar_month = crate::central::calendar::Month::now();
        self.fold_for_room();
        // The monitors, for the main one's brightness slider: as last read
        // or set. Never read over DDC on opening: a read holds the GPU's
        // I²C and the compositor's render threads with it (measured: the
        // cursor stuck up to ~235 ms); the page of displays reads them.
        let stale = self.displays_read.is_none_or(|at| at.elapsed() > Duration::from_secs(120));
        let displays = if stale {
            self.displays_read = Some(std::time::Instant::now());
            read_displays(services::DdcRead::Remembered)
        } else {
            Task::none()
        };
        // What another screen's applet wrote down: read now, not
        // watched all the time; off the UI's thread.
        Task::batch([load_clipboard(true), displays, read_camera()])
    }

    /// No room for it all: the calendar and what was copied fold, for the
    /// notifications; with room, they open.
    fn fold_for_room(&mut self) {
        let folded = !self.everything_fits(false);
        self.layout.show(Section::Notifications, false);
        self.layout.show(Section::Calendar, folded);
        self.layout.show(Section::Clipboard, folded);
    }

    /// Everything shown counts as seen; the service is told if anything was new.
    fn mark_seen(&mut self) -> app::Task<Message> {
        self.media_outputs = false;
        if !self.notifications.iter().any(|n| !n.restored && !n.seen) {
            return Task::none();
        }
        notifications::mark_seen(&mut self.notifications);
        notifications_ask(notifications::Action::Seen)
    }

    /// Out of editing: keeping what was arranged, or back to how it was.
    fn leave_edit(&mut self, keep: bool) {
        let Some(before) = self.before_edit.take() else {
            return;
        };
        if keep {
            // Saved now, with editing over.
            let ids: Vec<String> = self.tiles.iter().map(|t| t.id()).collect();
            services::set_tiles(&ids);
        } else {
            self.tiles = before;
        }
        self.drag = None;
        self.resizing = None;
    }

    fn save_tiles(&self) {
        // While editing, kept until saved.
        if self.before_edit.is_some() {
            return;
        }
        let ids: Vec<String> = self.tiles.iter().map(|t| t.id()).collect();
        services::set_tiles(&ids);
    }

    /// What a page needs once shown.
    fn load(&self, page: Page) -> app::Task<Message> {
        match page {
            Page::Wifi => Task::batch([
                load_wifi(false, Duration::ZERO),
                // A scan takes a few seconds.
                load_wifi(true, Duration::from_secs(3)),
            ]),
            Page::Sound => Task::batch([
                Task::future(async {
                    cosmic::action::app(Message::Sinks(services::sinks().await))
                }),
                load_mixer(),
            ]),
            // The page of brightness: read now, as asked to see it.
            Page::Displays => Task::batch([read_displays(services::DdcRead::All), read_brightness(Duration::ZERO)]),
            Page::Camera => read_camera(),
            _ => Task::none(),
        }
    }
}

/// As One UI lays it out: the sliders standing at the left, a pill and
/// short pills beside them, icons under, what's playing at the end.
fn default_tiles() -> Vec<Placed> {
    use crate::central::tiles::Fit::*;
    let at = |kind, fit| Placed { kind, fit };
    vec![
        at(Kind::Brightness, Vertical),
        at(Kind::Tile(Tile::Wifi), Pill),
        at(Kind::Tile(Tile::DoNotDisturb), Icon),
        at(Kind::Tile(Tile::Bluetooth), Short),
        at(Kind::Tile(Tile::Power), Short),
        at(Kind::Tile(Tile::DarkMode), Icon),
        at(Kind::Tile(Tile::Airplane), Icon),
        at(Kind::Tile(Tile::KeepAwake), Icon),
        at(Kind::Tile(Tile::Screenshot), Icon),
        at(Kind::Tile(Tile::Microphone), Icon),
        at(Kind::Tile(Tile::Workspaces), Icon),
        at(Kind::Tile(Tile::Lock), Icon),
    ]
}

/// The saved grid; the old list of tiles gets the sliders and media around
/// it, once.
fn load_tiles() -> Vec<Placed> {
    let Some(ids) = services::tiles() else {
        return default_tiles();
    };
    let mut items: Vec<Placed> = ids.iter().filter_map(|id| Placed::from_id(id)).collect();
    if !layout::grid_ready() {
        let has = |items: &Vec<Placed>, kind| items.iter().any(|p: &Placed| p.kind == kind);
        if !has(&items, Kind::Brightness) {
            items.insert(0, Placed::new(Kind::Brightness));
        }
        let ids: Vec<String> = items.iter().map(|t| t.id()).collect();
        services::set_tiles(&ids);
        layout::set_grid_ready();
    }
    // The header's buttons became icons: on the first bubble's end, once.
    if !layout::actions_ready() {
        let end = items.iter().position(|p| p.kind == Kind::Bubble).unwrap_or(items.len());
        let mut at = end;
        for tile in [Tile::Screenshot, Tile::Lock] {
            if !items.iter().any(|p| p.kind == Kind::Tile(tile)) {
                items.insert(at, Placed { kind: Kind::Tile(tile), fit: crate::central::tiles::Fit::Icon });
                at += 1;
            }
        }
        let ids: Vec<String> = items.iter().map(|t| t.id()).collect();
        services::set_tiles(&ids);
        layout::set_actions_ready();
    }
    // Settings and power went back to the header, as small icons.
    if items.iter().any(|p| matches!(p.kind, Kind::Tile(Tile::Settings | Tile::Session))) {
        items.retain(|p| !matches!(p.kind, Kind::Tile(Tile::Settings | Tile::Session)));
        let ids: Vec<String> = items.iter().map(|t| t.id()).collect();
        services::set_tiles(&ids);
    }
    items
}

fn load_mixer() -> app::Task<Message> {
    Task::future(async { cosmic::action::app(Message::Mixer(audio::mixer().await)) })
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// The steps of a scroll: a notch of the wheel is one.
pub fn scroll_steps(delta: iced::mouse::ScrollDelta) -> f32 {
    match delta {
        iced::mouse::ScrollDelta::Lines { y, .. } => y,
        iced::mouse::ScrollDelta::Pixels { y, .. } => y / 40.0,
    }
}

/// "2 h 40 min".
pub fn duration(seconds: i64) -> String {
    let minutes = (seconds + 59) / 60;
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

/// Asks the notifications service for something, then reads the history again.
fn notifications_ask(action: notifications::Action) -> app::Task<Message> {
    Task::future(async move {
        notifications::ask(action).await;
        cosmic::action::app(Message::NotificationsChanged)
    })
}

/// A live notification leaves the history first, then the action runs (if
/// any) and the daemon closes it. The other way round, the daemon's closing
/// could reach the service before the removal, and keep it there as a swiped
/// away one.
fn close_live(id: u32, action: Option<String>) -> app::Task<Message> {
    Task::future(async move {
        notifications::ask(notifications::Action::Remove(id, false)).await;
        if let Some(action) = action {
            notifications::invoke(id, action).await;
        }
        notifications::dismiss(vec![id]).await;
        cosmic::action::app(Message::NotificationsChanged)
    })
}

/// Asks the clipboard service for something, then reads the history again.
fn clipboard_ask(action: crate::central::clipboard::Action) -> app::Task<Message> {
    Task::future(async move {
        crate::central::clipboard::ask(action).await;
        cosmic::action::app(Message::ClipboardChanged)
    })
}

fn load_wifi(scan: bool, delay: Duration) -> app::Task<Message> {
    Task::future(async move {
        let networks = slow::bounded(Slow::Wifi, async {
            if scan {
                _ = services::wifi_networks(true).await;
            }
            tokio::time::sleep(delay).await;
            services::wifi_networks(false).await.ok().flatten()
        });
        match networks.await {
            Some(networks) => cosmic::action::app(Message::WifiNetworks(networks)),
            None => cosmic::action::app(Message::None),
        }
    })
}

/// After starting, when the display's brightness is first asked for.
const BRIGHTNESS_AFTER: Duration = Duration::from_secs(15);

/// The display's brightness, asked of the settings daemon after `delay`;
/// nothing if it doesn't answer (what's written down stays).
fn read_brightness(delay: Duration) -> app::Task<Message> {
    Task::future(async move {
        tokio::time::sleep(delay).await;
        match slow::bounded(Slow::Brightness, services::read_brightness()).await.flatten() {
            Some(brightness) => cosmic::action::app(Message::Update(Update::Brightness(Some(brightness)))),
            None => cosmic::action::app(Message::None),
        }
    })
}

/// The tiles that show what the accessibility protocol tells.
const A11Y_TILES: [Tile; 2] = [Tile::Magnifier, Tile::InvertColors];

/// Hosted, a panel with one of those tiles placed needs it; on the bar, one
/// of them shown there.
fn wants_a11y(hosted: bool, tiles: &[Placed], on_bar: impl Fn(Tile) -> bool) -> bool {
    if hosted {
        tiles.iter().any(|placed| A11Y_TILES.iter().any(|tile| placed.kind == Kind::Tile(*tile)))
    } else {
        A11Y_TILES.into_iter().any(on_bar)
    }
}

/// The monitors, and the brightness asked for over DDC, which may hang.
fn read_displays(read: services::DdcRead) -> app::Task<Message> {
    Task::future(async move {
        match slow::bounded(Slow::Ddc, services::displays(read)).await {
            Some(displays) => cosmic::action::app(Message::Displays(displays)),
            None => cosmic::action::app(Message::None),
        }
    })
}

/// The clipboard's history, read off the UI's thread.
fn load_clipboard(opened: bool) -> app::Task<Message> {
    Task::future(async move {
        let entries = slow::blocking(Slow::Clipboard, crate::central::clipboard::load).await;
        cosmic::action::app(Message::Clipboard(entries, opened))
    })
}

/// Hands a request to the panels' host, not waiting on it.
fn ask_host(asking: impl Future<Output = ()> + Send + 'static) -> app::Task<Message> {
    Task::future(async move {
        asking.await;
        cosmic::action::app(Message::None)
    })
}

fn spawn(program: &str, args: &[&str]) {
    crate::central::host::spawn(program, args);
}

/// How the camera is, off the UI's thread: systemctl and a file.
fn read_camera() -> app::Task<Message> {
    if !Tile::Camera.available() {
        return Task::none();
    }
    Task::future(async { cosmic::action::app(Message::Camera(services::camera().await)) })
}

/// The camera's unit started, stopped or restarted, then read again.
fn camera_command(verb: &'static str) -> app::Task<Message> {
    Task::future(async move {
        cosmic::action::app(Message::CameraDone(services::camera_command(verb).await))
    })
}

fn run<F: Future<Output = zbus::Result<()>> + Send + 'static>(
    what: &'static str,
    future: F,
) -> app::Task<Message> {
    Task::future(async move {
        if let Err(err) = future.await {
            tracing::warn!("cannot {what}: {err}");
        }
        cosmic::action::app(Message::None)
    })
}

/// A VPN on or off. One that won't come on, often for a password only
/// COSMIC Settings asks for, opens its page there.
fn connect_vpn(vpn: services::Vpn) -> app::Task<Message> {
    let turning_on = vpn.active.is_none();
    Task::future(async move {
        match services::connect_vpn(vpn).await {
            Err(err) if turning_on => {
                tracing::warn!("cannot turn the VPN on: {err}");
                cosmic::action::app(Message::Settings(Some("vpn")))
            }
            Err(err) => {
                tracing::warn!("cannot turn the VPN off: {err}");
                cosmic::action::app(Message::None)
            }
            Ok(()) => cosmic::action::app(Message::None),
        }
    })
}

fn popup_limits(max_height: f32) -> Limits {
    Limits::NONE
        .min_width(WIDTH)
        // Wider while editing, with its panel at the left.
        .max_width(WIDTH + crate::central::board::SIDEBAR + pages::PANEL_GAP)
        .min_height(1.0)
        .max_height(max_height)
}

/// Whether a press right after COSMIC closed the popup is the click that
/// closed it, and so not one to open it again.
fn press_just_closed(closed_at: Option<std::time::Instant>) -> bool {
    closed_at.is_some_and(|at| at.elapsed() < Duration::from_millis(250))
}

/// Whether the applet listens to every press on its windows: on the bar, where
/// the window is its own. Embedded in a corner, the press comes by area.
fn listens_to_window_presses(corner: bool) -> bool {
    !corner
}

/// The applet embedded in a corner applet: its press comes by area, and the
/// view is the button without a window of its own.
pub(crate) fn in_corner(core: app::Core) -> (App, app::Task<Message>) {
    init_with(core, true)
}

fn init_with(mut core: app::Core, corner: bool) -> (App, app::Task<Message>) {
    // See-through, but no blur behind: the theme stays transparent while any
    // kind is set, and only layer surfaces, which there are none of here, blur.
    core.set_auto_blur(cosmic::core::Auto::System.into());
    let tiles = load_tiles();
    // COSMIC_QS_PREVIEW_SELECT=4 picks that item while editing, to see
    // its sizes and handle.
    let selected_tile = std::env::var("COSMIC_QS_PREVIEW_SELECT")
        .ok()
        .and_then(|i| i.parse().ok());
    // COSMIC_QS_PREVIEW_ADDING=1 shows what can be added, while editing.
    let adding = std::env::var("COSMIC_QS_PREVIEW_ADDING").is_ok_and(|v| v == "1");
    let app = App {
        corner,
        core,
        adding,
        selected_tile,
        tiles,
        do_not_disturb: DO_NOT_DISTURB.get(),
        dark_mode: DARK_MODE.get(),
        tiling_config: services::tiling_config(),
        show_percentage: SHOW_PERCENTAGE.get(),
        notifications: notifications::load(),
        dnd_until: services::dnd_until(),
        touchpad: services::touchpad(),
        hot_edge: services::hot_edge(),
        bar_spacing: BarSpacing::load(),
        panel_icons: PanelIcons::load(),
        layout: Layout::load(),
        background: Background::load(),
        primary_display: layout::primary_display(),
        // As last read or set: the daemon is asked later, not now.
        brightness: services::remembered_brightness(),
        ..App::default()
    };
    // COSMIC_QS_PREVIEW=sound opens the popup on that page by itself, to
    // take screenshots without clicking the panel.
    // COSMIC_QS_PREVIEW_OUTPUT=DP-3 keeps it to the panel on that screen.
    let on_output = std::env::var("COSMIC_QS_PREVIEW_OUTPUT").map_or(true, |output| {
        std::env::var("COSMIC_PANEL_OUTPUT").is_ok_and(|panel| panel == output)
    });
    let preview = std::env::var("COSMIC_QS_PREVIEW")
        .ok()
        .filter(|_| on_output)
        .and_then(|name| Page::from_name(&name));
    let task = match preview {
        Some(page) => Task::future(async move {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            cosmic::action::app(Message::Preview(page))
        }),
        None => Task::none(),
    };
    // For the bar's icon, before the popup first opens.
    (app, Task::batch([task, read_camera(), read_brightness(BRIGHTNESS_AFTER)]))
}

impl cosmic::Application for App {
    type Message = Message;
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    const APP_ID: &'static str = crate::central::APP_ID;

    fn init(core: app::Core, _flags: ()) -> (Self, app::Task<Message>) {
        init_with(core, false)
    }

    fn core(&self) -> &app::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut app::Core {
        &mut self.core
    }

    fn style(&self) -> Option<iced::theme::Style> {
        Some(cosmic::applet::style())
    }

    fn update(&mut self, message: Message) -> app::Task<Message> {
        match message {
            Message::TogglePopup => {
                // On the bar, the host shows it; hosted, the host opens it.
                if !self.hosted {
                    return ask_host(crate::central::panels::service::toggle(Which::Central, panel_output()));
                }
                if self.popup.is_some() {
                    return self.close_popup();
                }
            }
            Message::PanelPress(id) => {
                // The click that closed the popup, if COSMIC closed it first,
                // isn't one to open it again; nor one on a background app's icon.
                if Some(id) == self.core.main_window_id()
                    && !press_just_closed(self.closed_at)
                    && !self.tray_just_pressed()
                {
                    return self.update(Message::TogglePopup);
                }
            }
            Message::CornerPress => {
                if !press_just_closed(self.closed_at) && !self.tray_just_pressed() {
                    return self.update(Message::TogglePopup);
                }
            }
            Message::Preview(page) => {
                if !self.hosted {
                    let open = crate::central::panels::service::open(Which::Central, panel_output(), page.name().to_owned());
                    return ask_host(open);
                }
                if self.popup.is_some() {
                    return self.update(Message::ShowPage(page));
                }
            }
            Message::OpenAt(page) => {
                if self.popup.is_some() {
                    return self.update(Message::ShowPage(page));
                }
                return self.update(Message::Preview(page));
            }
            Message::SetBarIcon(item, show) => self.panel_icons.set(item, show),
            Message::MoveBarIcon(item, up) => {
                let items = self.reorderable_bar_items();
                self.panel_icons.move_item(item, up, &items);
            }
            Message::BgDragStart(target) => {
                self.bg_drag = Some(target);
            }
            Message::BgDragOver(target) => match (self.bg_drag, target) {
                (Some(BgDrag::App(from)), BgDrag::App(to)) if from != to => {
                    self.background.move_to(from, to);
                }
                (Some(BgDrag::BarItem(from)), BgDrag::BarItem(to)) if from != to => {
                    let items = self.reorderable_bar_items();
                    self.panel_icons.move_to(from, to, &items);
                }
                _ => {}
            },
            Message::BgDragEnd => {
                self.bg_drag = None;
            }
            Message::Tray(message) => return self.tray(message),
            Message::ToggleFold(section) => {
                self.layout.toggle_fold(section);
                // Opened with no room for it: the other panels on top fold.
                let top = [Section::Notifications, Section::Calendar, Section::Clipboard];
                if top.contains(&section) && !self.layout.is_folded(section) {
                    let editing = self.page == Some(Page::Edit);
                    for other in top.into_iter().filter(|s| *s != section) {
                        if self.top_height() <= self.top_room(editing) {
                            break;
                        }
                        if !self.layout.is_folded(other) {
                            // For now only: the next opening decides again.
                            self.layout.show(other, true);
                        }
                    }
                }
            }
            Message::ResizeDrag(index, dx) => {
                self.hovered = Some(index);
                let Some(placed) = self.tiles.get(index).copied() else {
                    return Task::none();
                };
                let start = match self.resizing {
                    Some((i, fit)) if i == index => fit,
                    _ => {
                        self.resizing = Some((index, placed.fit));
                        placed.fit
                    }
                };
                // The size whose width is nearest where the handle is.
                let target = f32::from(start.span().0) + dx / crate::central::board::column_step();
                let fit = placed
                    .kind
                    .fits()
                    .iter()
                    .copied()
                    .min_by(|a, b| {
                        let far = |fit: &Fit| (f32::from(fit.span().0) - target).abs();
                        far(a).total_cmp(&far(b))
                    })
                    .unwrap_or(placed.fit);
                if fit != placed.fit {
                    self.tiles[index].fit = fit;
                }
            }
            Message::ResizeEnd => {
                if self.resizing.take().is_some() {
                    self.save_tiles();
                }
            }
            Message::AddItem(kind) => {
                // Lines and breaks can repeat, the rest go on once.
                if kind.splits() || !self.tiles.iter().any(|p| p.kind == kind) {
                    let at = self.insert_at(self.add_target, kind);
                    let mut trial = self.tiles.clone();
                    trial.insert(at, Placed::new(kind));
                    if crate::central::tiles::within_limits(&trial) {
                        self.tiles = trial;
                        self.selected_tile = (!kind.splits()).then_some(at);
                        if kind == Kind::Bubble {
                            self.add_target += 1;
                        }
                        self.save_tiles();
                    }
                }
            }
            Message::SetPrimaryDisplay(name) => {
                layout::set_primary_display(&name);
                self.primary_display = Some(name);
            }
            Message::Closed(id) => {
                if self.popup == Some(id) {
                    self.closed_at = Some(std::time::Instant::now());
                    let seen = self.mark_seen();
                    self.leave_edit(false);
                    self.last_closed_page = self.page_name();
                    self.background.close();
                    self.background_editing = false;
                    self.tray_menu_return = None;
                    self.popup = None;
                    self.page = None;
                    return seen;
                }
            }
            Message::ShowPage(page) => {
                if page == Page::Edit && self.page != Some(Page::Edit) {
                    self.before_edit = Some(self.tiles.clone());
                    self.add_target = 0;
                    self.adding = false;
                } else if page != Page::Edit && self.page == Some(Page::Edit) {
                    // Away from editing without saving: as it was.
                    self.leave_edit(false);
                }
                if page != Page::Background {
                    self.background_editing = false;
                }
                // Away from an app's menu: it's told it closed.
                if page != Page::TrayMenu {
                    self.background.close();
                }
                self.page = Some(page);
                self.selected_tile = None;
                return self.load(page);
            }
            Message::AddTarget(bubble) => {
                // The same "+" again closes it.
                self.adding = !(self.adding && self.add_target == bubble);
                self.add_target = bubble;
            }
            Message::CloseAdding => self.adding = false,
            Message::CalendarShift(by) => self.calendar_month = self.calendar_month.shift(by),
            Message::CalendarToday => self.calendar_month = crate::central::calendar::Month::now(),
            Message::ClockTick => self.clock = crate::central::calendar::bar_lines(),
            Message::ClipboardChanged => return load_clipboard(false),
            Message::Clipboard(entries, opened) => {
                if let Some(entries) = entries {
                    self.clipboard = entries;
                    if opened && self.page == Some(Page::Main) {
                        self.fold_for_room();
                    }
                }
            }
            Message::CopyEntry(id) => return clipboard_ask(crate::central::clipboard::Action::Copy(id)),
            Message::PinEntry(id) => return clipboard_ask(crate::central::clipboard::Action::Pin(id)),
            Message::DeleteEntry(id) => return clipboard_ask(crate::central::clipboard::Action::Remove(id)),
            Message::ClearClipboard => return clipboard_ask(crate::central::clipboard::Action::Clear),
            Message::AddStructure(kind, bubble) => {
                let mut trial = self.tiles.clone();
                trial.insert(self.insert_at(bubble, kind), Placed::new(kind));
                if kind.splits() && crate::central::tiles::within_limits(&trial) {
                    self.tiles = trial;
                    self.selected_tile = None;
                    self.save_tiles();
                }
            }
            Message::SaveEdit => {
                self.leave_edit(true);
                self.page = Some(Page::Main);
                self.selected_tile = None;
            }
            Message::CancelEdit => {
                self.leave_edit(false);
                self.page = Some(Page::Main);
                self.selected_tile = None;
            }
            Message::Update(update) => match update {
                Update::Network(network) => {
                    let changed = network != self.network;
                    self.network = network;
                    if changed && self.page == Some(Page::Wifi) {
                        return load_wifi(false, Duration::ZERO);
                    }
                }
                Update::Bluetooth(bluetooth) => self.bluetooth = bluetooth,
                Update::Battery(battery) => self.battery = battery,
                Update::Volume(_) | Update::Audio if self.adjusting() => {}
                Update::Volume(volume) => {
                    let changed = volume.is_some() != self.volume.is_some();
                    self.volume = volume;
                    if changed && self.page == Some(Page::Sound) {
                        return self.load(Page::Sound);
                    }
                }
                Update::Microphone(muted) => self.microphone = muted,
                Update::Audio => {
                    if self.page == Some(Page::Sound) {
                        return load_mixer();
                    }
                }
                Update::Peripherals(peripherals) => self.peripherals = peripherals,
                Update::Brightness(_) if self.adjusting() => {}
                Update::Brightness(brightness) => self.brightness = brightness,
                Update::PowerProfiles(power) => self.power = power,
            },
            Message::SetVolume(percent) => {
                self.adjusting = Some(std::time::Instant::now());
                let level = percent / 100.0;
                if let Some(volume) = self.volume.as_mut() {
                    volume.level = level;
                }
                return Task::future(async move {
                    services::set_volume(level).await;
                    cosmic::action::app(Message::None)
                });
            }
            Message::ToggleMute => {
                // At once, the bar's icon too; PipeWire then says the same.
                if let Some(volume) = self.volume.as_mut() {
                    volume.muted = !volume.muted;
                    self.adjusting = Some(std::time::Instant::now());
                }
                return Task::future(async {
                    services::toggle_mute().await;
                    cosmic::action::app(Message::None)
                });
            }
            Message::ToggleMicrophone => {
                return Task::future(async {
                    services::toggle_microphone().await;
                    cosmic::action::app(Message::None)
                });
            }
            Message::SetBrightness(percent) => {
                if let Some((current, max)) = self.brightness.as_mut() {
                    self.adjusting = Some(std::time::Instant::now());
                    // Never all the way to black.
                    *current = ((percent / 100.0 * *max as f32) as i32).max(*max / 100).max(1);
                    let (value, max) = (*current, *max);
                    return Task::future(async move {
                        services::set_brightness(value, max).await;
                        cosmic::action::app(Message::None)
                    });
                }
            }
            Message::ToggleWifi => {
                if self.network.wifi_enabled {
                    self.wifi_networks = None;
                }
                let enable = !self.network.wifi_enabled;
                let mut tasks = vec![run("toggle Wi-Fi", services::set_wifi(enable))];
                if enable && self.page == Some(Page::Wifi) {
                    tasks.push(load_wifi(true, Duration::from_secs(3)));
                }
                return Task::batch(tasks);
            }
            Message::WifiNetworks(networks) => self.wifi_networks = networks,
            Message::JoinWifi(index) => {
                let Some(networks) = &self.wifi_networks else {
                    return Task::none();
                };
                let Some(network) = networks.networks.get(index).cloned() else {
                    return Task::none();
                };
                let device = networks.device.clone();
                return Task::future(async move {
                    match services::join_wifi(device, network).await {
                        Ok(Joined::NeedsSettings) => {
                            cosmic::action::app(Message::Settings(Some("wireless")))
                        }
                        Ok(Joined::Connecting) => cosmic::action::app(Message::None),
                        Err(err) => {
                            tracing::warn!("cannot join network: {err}");
                            cosmic::action::app(Message::None)
                        }
                    }
                });
            }
            Message::ToggleBluetooth => {
                if let Some(adapter) = self.bluetooth.adapter.clone() {
                    return run(
                        "toggle Bluetooth",
                        services::set_bluetooth(adapter, !self.bluetooth.powered),
                    );
                }
            }
            Message::ConnectBluetooth(device, connect) => {
                return run(
                    "connect Bluetooth device",
                    services::connect_bluetooth(device, connect),
                );
            }
            Message::ToggleVpn => {
                let on: Vec<_> = self
                    .network
                    .vpns
                    .iter()
                    .filter(|vpn| vpn.active.is_some())
                    .cloned()
                    .collect();
                if on.is_empty() {
                    return match self.network.vpns.first() {
                        Some(vpn) => connect_vpn(vpn.clone()),
                        // None set up yet: where they are.
                        None => self.update(Message::Settings(Some("vpn"))),
                    };
                }
                return Task::batch(on.into_iter().map(connect_vpn));
            }
            Message::ConnectVpn(index) => {
                if let Some(vpn) = self.network.vpns.get(index) {
                    return connect_vpn(vpn.clone());
                }
            }
            Message::NextPowerProfile => {
                if let Some(power) = &self.power {
                    let order = ["power-saver", "balanced", "performance"];
                    let available: Vec<&str> = order
                        .into_iter()
                        .filter(|p| power.available.iter().any(|a| a == p))
                        .collect();
                    let current = available.iter().position(|p| *p == power.active);
                    let next = current.map_or(0, |i| (i + 1) % available.len());
                    if let Some(next) = available.get(next) {
                        return run(
                            "change power profile",
                            services::set_power_profile((*next).to_owned()),
                        );
                    }
                }
            }
            Message::SetPowerProfile(profile) => {
                return run("change power profile", services::set_power_profile(profile));
            }
            Message::ToggleDoNotDisturb => {
                let dnd = if self.do_not_disturb { Dnd::Off } else { Dnd::Forever };
                return self.update(Message::SetDoNotDisturb(dnd));
            }
            Message::SetDoNotDisturb(dnd) => {
                use chrono::{Days, Local, NaiveTime};
                self.dnd_until = match dnd {
                    Dnd::Off | Dnd::Forever => None,
                    Dnd::For(seconds) => Some(now() + seconds),
                    Dnd::Tomorrow => Local::now()
                        .date_naive()
                        .checked_add_days(Days::new(1))
                        .and_then(|day| {
                            day.and_time(NaiveTime::from_hms_opt(8, 0, 0)?)
                                .and_local_timezone(Local)
                                .earliest()
                        })
                        .map(|t| t.timestamp()),
                };
                self.do_not_disturb = !matches!(dnd, Dnd::Off);
                DO_NOT_DISTURB.set(self.do_not_disturb);
                services::set_dnd_until(self.dnd_until);
            }
            Message::CheckDoNotDisturb => {
                if self.dnd_until.is_some_and(|until| now() >= until) {
                    return self.update(Message::SetDoNotDisturb(Dnd::Off));
                }
            }
            Message::ToggleDarkMode => {
                self.dark_mode = !self.dark_mode;
                DARK_MODE.set(self.dark_mode);
            }
            Message::ToggleAirplaneMode => {
                let radios_on = self.airplane_mode();
                let mut tasks = vec![run("toggle Wi-Fi", services::set_wifi(radios_on))];
                if let Some(adapter) = self.bluetooth.adapter.clone() {
                    tasks.push(run(
                        "toggle Bluetooth",
                        services::set_bluetooth(adapter, radios_on),
                    ));
                }
                return Task::batch(tasks);
            }
            Message::Output(name, height) => {
                tracing::debug!("output {name:?}: {height:?}");
                let panel = std::env::var("COSMIC_PANEL_OUTPUT").unwrap_or_default();
                // the panel's screen, or any if it's on all of them
                if name.as_deref() == Some(panel.as_str())
                    || (panel.is_empty() || panel == "all") && self.screen_height.is_none()
                {
                    // What cosmic-randr says wins: it knows a screen standing.
                    if !self.screen_height_known {
                        self.screen_height = height;
                    }
                    if let Some(name) = name.filter(|_| !self.screen_height_known) {
                        return Task::future(async move {
                            cosmic::action::app(Message::ScreenHeight(services::screen_height(name).await))
                        });
                    }
                }
            }
            Message::ScreenHeight(height) => {
                if height.is_some() {
                    self.screen_height = height;
                    self.screen_height_known = true;
                }
            }
            Message::ToggleTouchpad => {
                let enabled = self.touchpad != Some(services::Touchpad::Enabled);
                self.touchpad = Some(if enabled {
                    services::Touchpad::Enabled
                } else {
                    services::Touchpad::Disabled
                });
                services::set_touchpad(enabled);
            }
            Message::ToggleHotEdge => {
                self.hot_edge = !self.hot_edge;
                services::set_hot_edge(self.hot_edge);
            }
            Message::OpenHotEdge => {
                spawn("cosmic-ext-hot-edge", &[]);
                return self.close_popup();
            }
            Message::ToggleCamera => {
                // At once, as the other tiles; then as it really is.
                let verb = if self.camera.on { "stop" } else { "start" };
                self.camera = services::Camera {
                    on: !self.camera.on,
                    status: None,
                };
                self.camera_busy += 1;
                return camera_command(verb);
            }
            Message::RestartCamera => {
                self.camera = services::Camera {
                    on: true,
                    status: None,
                };
                self.camera_busy += 1;
                return camera_command("restart");
            }
            Message::ReadCamera => {
                if self.camera_busy == 0 {
                    return read_camera();
                }
            }
            Message::Camera(camera) => {
                if self.camera_busy == 0 {
                    self.camera = camera;
                    }
            }
            Message::CameraDone(camera) => {
                self.camera_busy = self.camera_busy.saturating_sub(1);
                // The last one done tells how it ended up.
                if self.camera_busy == 0 {
                    self.camera = camera;
                    }
            }
            Message::IriunWindow(show) => {
                // At once, as asked; then as the service wrote it down.
                if let Some(status) = &mut self.camera.status {
                    status.pc_window = Some(if show {
                        services::PcWindow::Shown
                    } else {
                        services::PcWindow::Hidden
                    });
                }
                self.camera_busy += 1;
                return Task::future(async move {
                    cosmic::action::app(Message::CameraDone(services::camera_window(show).await))
                });
            }
            Message::ToggleKeepAwake => {
                return match self.inhibit.take() {
                    Some(cookie) => run("allow idling", services::allow_idle(cookie)),
                    None => Task::future(async {
                        let cookie = services::inhibit_idle(strings().keep_awake_reason)
                            .await
                            .inspect_err(|err| tracing::warn!("cannot keep awake: {err}"))
                            .ok();
                        cosmic::action::app(Message::Inhibited(cookie))
                    }),
                };
            }
            Message::Inhibited(cookie) => self.inhibit = cookie,
            Message::SettingsChanged => {
                // What another screen's applet saved; not while this one is
                // in the middle of moving or resizing an item.
                if self.drag.is_none() && self.resizing.is_none() && self.before_edit.is_none() {
                    let tiles = load_tiles();
                    if tiles != self.tiles {
                        self.tiles = tiles;
                        self.selected_tile = None;
                    }
                }
                self.panel_icons = PanelIcons::load();
                self.layout = Layout::load();
                self.bar_spacing = BarSpacing::load();
                self.primary_display = layout::primary_display();
                self.do_not_disturb = DO_NOT_DISTURB.get();
                // Turned off elsewhere: no timer left.
                if !self.do_not_disturb && self.dnd_until.is_some() {
                    self.dnd_until = None;
                    services::set_dnd_until(None);
                }
                self.dark_mode = DARK_MODE.get();
                self.tiling_config = services::tiling_config();
                self.touchpad = services::touchpad();
                self.hot_edge = services::hot_edge();
            }
            Message::ToggleShowPercentage => {
                self.show_percentage = !self.show_percentage;
                SHOW_PERCENTAGE.set(self.show_percentage);
            }
            Message::Sinks(sinks) => self.sinks = sinks,
            Message::SetSink(name) => {
                if let Some((_, default)) = self.sinks.as_mut() {
                    *default = name.clone();
                }
                return Task::future(async move {
                    services::set_sink(name).await;
                    cosmic::action::app(Message::None)
                });
            }
            Message::Displays(mut displays) => {
                // Levels not read this time: as they were.
                for display in &mut displays {
                    if let Some((number, u32::MAX, _)) = display.ddc {
                        display.ddc = self
                            .displays
                            .as_ref()
                            .and_then(|old| old.iter().find(|d| d.name == display.name))
                            .and_then(|d| d.ddc)
                            .filter(|(_, level, _)| *level != u32::MAX)
                            .or(Some((number, 100, 100)));
                    }
                }
                self.displays = Some(displays);
            }
            Message::Workspaces(event) => match event {
                workspaces::Event::Ready(tx) => self.workspaces_tx = Some(tx),
                workspaces::Event::Tiling(tiling) => self.workspace_tiling = Some(tiling),
            },
            Message::ToggleTiling => {
                if let (Some(tx), Some(tiling)) = (&self.workspaces_tx, self.workspace_tiling) {
                    // The workspace's tiling then follows its own setting.
                    services::set_tiling_config("autotile", self.tiling_config.new_workspaces_tile);
                    _ = tx.send(!tiling);
                    self.workspace_tiling = Some(!tiling);
                }
            }
            Message::SetNewWorkspacesTile(tile) => {
                self.tiling_config.new_workspaces_tile = tile;
                services::set_tiling_config("autotile", tile);
            }
            Message::ToggleActiveHint => {
                self.tiling_config.active_hint = !self.tiling_config.active_hint;
                services::set_tiling_config("active_hint", self.tiling_config.active_hint);
            }
            Message::A11y(event) => match event {
                services::A11y::Ready(tx) => self.a11y_tx = Some(tx),
                services::A11y::Event(event) => match event {
                    services::AccessibilityEvent::Magnifier(on) => self.magnifier = Some(on),
                    services::AccessibilityEvent::ScreenFilter { inverted, filter } => {
                        self.screen_filter = Some((inverted, filter));
                    }
                    services::AccessibilityEvent::Closed => {
                        self.a11y_tx = None;
                        self.magnifier = None;
                        self.screen_filter = None;
                    }
                    // Only changes are sent: until one, both are off.
                    services::AccessibilityEvent::Bound(_) => {
                        self.magnifier.get_or_insert(false);
                        self.screen_filter.get_or_insert((false, None));
                    }
                },
            },
            Message::ScreenReader(response) => {
                use cosmic_settings_accessibility_subscription::Response;
                match response {
                    Response::Init(enabled, tx) => {
                        self.screen_reader = Some(enabled);
                        self.screen_reader_tx = Some(tx);
                    }
                    Response::ScreenReader(enabled) => self.screen_reader = Some(enabled),
                    Response::IsEnabled(_) => {}
                    Response::Error(err) => tracing::debug!("screen reader: {err}"),
                }
            }
            Message::ToggleMagnifier => {
                if let (Some(tx), Some(on)) = (&self.a11y_tx, self.magnifier) {
                    _ = tx.send(services::AccessibilityRequest::Magnifier(!on));
                }
            }
            Message::ToggleInvertColors => {
                if let (Some(tx), Some((inverted, filter))) = (&self.a11y_tx, self.screen_filter) {
                    _ = tx.send(services::AccessibilityRequest::ScreenFilter {
                        inverted: !inverted,
                        filter,
                    });
                }
            }
            Message::ToggleScreenReader => {
                if let (Some(tx), Some(on)) = (&self.screen_reader_tx, self.screen_reader) {
                    _ = tx.send(cosmic_settings_accessibility_subscription::Request::ScreenReader(!on));
                    self.screen_reader = Some(!on);
                }
            }
            Message::ToggleHighContrast => {
                let enabled = !self.core.system_theme().cosmic().is_high_contrast;
                services::set_high_contrast(enabled);
            }
            Message::OpenWorkspaces => {
                spawn("cosmic-workspaces", &[]);
                return self.close_popup();
            }
            Message::Media(mut media) => {
                // Browsers leave the length out of what they tell for a moment
                // while seeking, and may say nothing more after: the progress
                // bar went with it until playback paused or resumed.
                for m in media.iter_mut().filter(|m| m.length <= 0) {
                    if let Some(old) = self
                        .media
                        .iter()
                        .find(|old| old.service == m.service && old.title == m.title)
                    {
                        m.length = old.length;
                    }
                }
                // A player starting to play comes forward, as on a phone.
                let started = media.iter().find(|m| {
                    m.playing
                        && !self
                            .media
                            .iter()
                            .any(|old| old.service == m.service && old.playing)
                });
                if let Some(started) = started {
                    self.player = Some(started.service.clone());
                }
                self.media = media;
                self.seeking = None;
            }
            Message::SelectPlayer(service) => {
                self.player = Some(service);
                self.seeking = None;
            }
            Message::ToggleMediaOutputs => {
                self.media_outputs = !self.media_outputs;
                if self.media_outputs {
                    return Task::future(async {
                        cosmic::action::app(Message::Sinks(services::sinks().await))
                    });
                }
            }
            Message::MediaControl(control) => {
                if let Some(media) = self.shown_media() {
                    return run("control the player", media::control(media.service.clone(), control));
                }
            }
            Message::Seek(position) => self.seeking = Some(position),
            Message::SeekDone => {
                let service = self.shown_media().map(|m| m.service.clone());
                let media = self
                    .media
                    .iter_mut()
                    .find(|m| Some(&m.service) == service.as_ref());
                let (Some(position), Some(media)) = (self.seeking.take(), media) else {
                    return Task::none();
                };
                media.position = position as i64;
                media.at = std::time::Instant::now();
                if let Some(track) = media.track.clone() {
                    return run(
                        "seek",
                        media::seek(media.service.clone(), track, position as i64),
                    );
                }
            }
            Message::Tick => {}
            Message::NotificationsChanged => self.notifications = notifications::load(),
            Message::OpenNotification(key) => {
                let (id, restored) = key;
                if restored {
                    return self.remove_notification(key);
                }
                notifications::remove(&mut self.notifications, id, false);
                let close = self.close_popup();
                return Task::batch([close, close_live(id, Some("default".to_owned()))]);
            }
            Message::DismissNotification(key) => {
                let (id, restored) = key;
                if restored {
                    return self.remove_notification(key);
                }
                notifications::remove(&mut self.notifications, id, false);
                return close_live(id, None);
            }
            Message::NotificationAction(id, action) => {
                notifications::remove(&mut self.notifications, id, false);
                return close_live(id, Some(action));
            }
            Message::ToggleGroup(app) => {
                if !self.expanded.remove(&app) {
                    self.expanded.insert(app);
                }
            }
            Message::ClearNotifications => {
                let ids = notifications::clear(&mut self.notifications);
                return Task::batch([
                    notifications_ask(notifications::Action::Clear),
                    Task::future(async move {
                        notifications::dismiss(ids).await;
                        cosmic::action::app(Message::None)
                    }),
                ]);
            }
            Message::SetScale(index, scale) => {
                let Some(display) = self.displays.as_mut().and_then(|d| d.get_mut(index)) else {
                    return Task::none();
                };
                display.scale = scale;
                let display = display.clone();
                return Task::future(async move {
                    services::set_scale(display, scale).await;
                    cosmic::action::app(Message::None)
                })
                .chain(read_displays(services::DdcRead::All));
            }
            Message::SetDdcBrightness(index, percent) => {
                if let Some((_, current, max)) = self
                    .displays
                    .as_mut()
                    .and_then(|d| d.get_mut(index))
                    .and_then(|d| d.ddc.as_mut())
                {
                    *current = (percent / 100.0 * *max as f32).round() as u32;
                }
            }
            Message::ApplyDdcBrightness(index) => {
                // DDC is slow: only once let go.
                if let Some((number, current, max)) = self
                    .displays
                    .as_ref()
                    .and_then(|d| d.get(index))
                    .and_then(|d| d.ddc)
                {
                    return Task::future(async move {
                        services::set_ddc_brightness(number, current, max).await;
                        cosmic::action::app(Message::None)
                    });
                }
            }
            Message::DragStart(index) => {
                self.drag = Some(Drag {
                    index,
                    moved: false,
                });
            }
            Message::HoverEnd(index) => {
                if self.hovered == Some(index) {
                    self.hovered = None;
                }
            }
            Message::DragOver(index) => {
                // Entering an item shows its handle, dragging or not.
                self.hovered = Some(index);
                // The others make way as it passes over them.
                if let Some(drag) = self.drag.as_mut() {
                    if drag.index != index && index < self.tiles.len() {
                        let mut trial = self.tiles.clone();
                        let placed = trial.remove(drag.index);
                        trial.insert(index, placed);
                        // Not where it'd make a fourth bubble or a third line.
                        if crate::central::tiles::within_limits(&trial) {
                            self.tiles = trial;
                            drag.index = index;
                            drag.moved = true;
                            self.selected_tile = None;
                        }
                    }
                }
            }
            Message::DragEnd => {
                if let Some(drag) = self.drag.take() {
                    if drag.moved {
                        self.save_tiles();
                    } else {
                        // A click: choose it to move or resize, or let go of it.
                        self.selected_tile = match self.selected_tile {
                            Some(selected) if selected == drag.index => None,
                            _ => Some(drag.index),
                        };
                    }
                }
            }
            Message::RemoveTile(index) => {
                if index < self.tiles.len() {
                    let mut trial = self.tiles.clone();
                    trial.remove(index);
                    // Taking a bubble's break off joins two bubbles: not if
                    // that makes too many lines in one.
                    if crate::central::tiles::within_limits(&trial) {
                        self.tiles = trial;
                        self.selected_tile = None;
                        self.save_tiles();
                    }
                }
            }
            Message::ResetTiles => {
                self.tiles = default_tiles();
                self.selected_tile = None;
                self.save_tiles();
            }
            Message::SetBarSpacing(spacing) => {
                self.bar_spacing = spacing;
                spacing.save();
            }
            Message::Settings(page) => {
                spawn("cosmic-settings", page.as_slice());
                return self.close_popup();
            }
            Message::Screenshot => {
                spawn("cosmic-screenshot", &[]);
                return self.close_popup();
            }
            Message::Lock => {
                spawn("loginctl", &["lock-session"]);
                return self.close_popup();
            }
            Message::PowerOff => {
                spawn("cosmic-osd", &["shutdown"]);
                return self.close_popup();
            }
            Message::Restart => {
                spawn("cosmic-osd", &["restart"]);
                return self.close_popup();
            }
            Message::LogOut => {
                spawn("cosmic-osd", &["log-out"]);
                return self.close_popup();
            }
            Message::Suspend => {
                spawn("systemctl", &["suspend"]);
                return self.close_popup();
            }
            Message::Mixer(_) if self.adjusting() => {}
            Message::Mixer(mixer) => self.mixer = Some(mixer),
            Message::AdjustDone => {
                self.adjusting = None;
                if self.page == Some(Page::Sound) {
                    return load_mixer();
                }
            }
            Message::SetStreamVolume(app, percent) => {
                self.adjusting = Some(std::time::Instant::now());
                let Some(stream) = self.stream_mut(&app) else {
                    return Task::none();
                };
                stream.level = percent / 100.0;
                let (indices, level) = (stream.indices.clone(), stream.level);
                return Task::future(async move {
                    audio::set_stream_volume(indices, level).await;
                    cosmic::action::app(Message::None)
                });
            }
            Message::ToggleStreamMute(app) => {
                let Some(stream) = self.stream_mut(&app) else {
                    return Task::none();
                };
                stream.muted = !stream.muted;
                let (indices, muted) = (stream.indices.clone(), stream.muted);
                return Task::future(async move {
                    audio::set_stream_mute(indices, muted).await;
                    cosmic::action::app(Message::None)
                });
            }
            Message::SetSource(name) => {
                if let Some(mixer) = self.mixer.as_mut() {
                    mixer.default_source = name.clone();
                }
                return Task::future(async move {
                    audio::set_source(name).await;
                    cosmic::action::app(Message::Mixer(audio::mixer().await))
                });
            }
            Message::SetInputVolume(percent) => {
                self.adjusting = Some(std::time::Instant::now());
                let level = percent / 100.0;
                if let Some(input) = self.mixer.as_mut().and_then(|m| m.input.as_mut()) {
                    input.level = level;
                }
                return Task::future(async move {
                    audio::set_input_volume(level).await;
                    cosmic::action::app(Message::None)
                });
            }
            Message::ToggleInputMute => {
                if let Some(input) = self.mixer.as_mut().and_then(|m| m.input.as_mut()) {
                    input.muted = !input.muted;
                }
                return Task::future(async {
                    services::toggle_microphone().await;
                    cosmic::action::app(Message::None)
                });
            }
            Message::Privacy(privacy) => self.privacy = privacy,
            Message::ScrollVolume(steps) => {
                if let Some(volume) = &self.volume {
                    let percent = (volume.level * 100.0 + steps * 5.0).clamp(0.0, 100.0);
                    return self.update(Message::SetVolume(percent));
                }
            }
            Message::ScrollBrightness(steps) => {
                if let Some((current, max)) = self.brightness {
                    let percent = (current as f32 * 100.0 / max as f32 + steps * 5.0).clamp(0.0, 100.0);
                    return self.update(Message::SetBrightness(percent));
                }
            }
            Message::None => {}
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            services::subscription().map(Message::Update),
            services::settings_subscription().map(|()| Message::SettingsChanged),
            workspaces::subscription().map(Message::Workspaces),
            cosmic_settings_accessibility_subscription::subscription().map(Message::ScreenReader),
            media::subscription().map(Message::Media),
            audio::privacy_subscription().map(Message::Privacy),
            notifications::subscription().map(|()| Message::NotificationsChanged),
            crate::central::clipboard::subscription().map(|()| Message::ClipboardChanged),
        ];
        subscriptions.push(self.lasting_subscription());
        // The screen's size, for the popup's height.
        subscriptions.push(event::listen_with(|event, _, _| {
            use iced::event::wayland::{Event as Wayland, OutputEvent};
            match event {
                iced::Event::PlatformSpecific(iced::event::PlatformSpecific::Wayland(
                    Wayland::Output(
                        OutputEvent::Created(Some(info)) | OutputEvent::InfoUpdate(info),
                        _,
                    ),
                )) => {
                    let height = info.logical_size.map(|(_, h)| h);
                    Some(Message::Output(info.name.clone(), height))
                }
                _ => None,
            }
        }));

        if listens_to_window_presses(self.corner) {
            subscriptions.push(event::listen_with(|event, _, id| match event {
                iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
                    Some(Message::PanelPress(id))
                }
                _ => None,
            }));
        }
        if self.drag.is_some() {
            // Let go anywhere, on a tile or not.
            subscriptions.push(event::listen_with(|event, _, _| match event {
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                    iced::mouse::Button::Left,
                )) => Some(Message::DragEnd),
                _ => None,
            }));
        }
        if self.bg_drag.is_some() {
            subscriptions.push(event::listen_with(|event, _, _| match event {
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                    iced::mouse::Button::Left,
                )) => Some(Message::BgDragEnd),
                _ => None,
            }));
        }
        if self.panel_icons.get(BarItem::Clock) != Show::Never {
            subscriptions.push(crate::central::calendar::minutes().map(|()| Message::ClockTick));
        }
        if self.dnd_until.is_some() {
            subscriptions.push(
                iced::time::every(Duration::from_secs(20)).map(|_| Message::CheckDoNotDisturb),
            );
        }
        // The service writes down how it is every couple of seconds: for its
        // page, and for the tile's subtitle while it's on.
        if self.popup.is_some() && (self.page == Some(Page::Camera) || self.camera.on) {
            subscriptions
                .push(iced::time::every(Duration::from_secs(2)).map(|_| Message::ReadCamera));
        }
        if self.popup.is_some() && self.shown_media().is_some_and(|m| m.playing) {
            subscriptions
                .push(iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick));
        }
        Subscription::batch(subscriptions)
    }

    fn view(&self) -> Element<'_, Message> {
        self.core.applet.autosize_window(self.bar_button(false)).into()
    }

    fn view_window(&self, id: window::Id) -> Element<'_, Message> {
        if self.popup != Some(id) {
            return widget::text("").into();
        }
        crate::central::timing::viewed("central");
        let page = self.page.unwrap_or(Page::Main);
        let content = match page {
            Page::Main => self.main_page(),
            // The editing panel at the left, the panel as it is beside it.
            Page::Edit => widget::row::with_children(vec![
                widget::container(self.edit_side())
                    .width(Length::Fixed(crate::central::board::SIDEBAR))
                    .into(),
                {
                    let board: Element<'_, Message> =
                        widget::container(self.edit_page()).width(Length::Fixed(WIDTH)).into();
                    if self.adding {
                        // What can be added, a card floating over the panel,
                        // the rest dimmed; a click on that closes it.
                        let backdrop = widget::mouse_area(
                            widget::container(widget::space::horizontal())
                                .width(Length::Fill)
                                .height(Length::Fill)
                                .class(theme::Container::custom(|theme| {
                                    let mut shade: iced::Color =
                                        theme.cosmic().background(theme.transparent).base.into();
                                    shade.a = 0.72;
                                    widget::container::Style {
                                        background: Some(iced::Background::Color(shade)),
                                        border: iced::Border {
                                            radius: theme.cosmic().corner_radii.radius_m.into(),
                                            ..Default::default()
                                        },
                                        ..Default::default()
                                    }
                                })),
                        )
                        .on_press(Message::CloseAdding);
                        // Solid, unlike the panels, which the theme may let
                        // show through: nothing behind it reads through.
                        let card = widget::container(
                            widget::container(self.adding_panel())
                                .width(Length::Fixed(WIDTH - 32.0))
                                .class(theme::Container::custom(|theme| {
                                    let cosmic = theme.cosmic();
                                    let mut base: iced::Color =
                                        cosmic.background(theme.transparent).base.into();
                                    base.a = 1.0;
                                    widget::container::Style {
                                        background: Some(iced::Background::Color(base)),
                                        border: iced::Border {
                                            radius: cosmic.corner_radii.radius_m.into(),
                                            width: 1.0,
                                            color: cosmic.accent_color().into(),
                                        },
                                        shadow: iced::Shadow {
                                            color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.35),
                                            offset: iced::Vector::new(0.0, 6.0),
                                            blur_radius: 24.0,
                                        },
                                        ..Default::default()
                                    }
                                })),
                        )
                        .center(Length::Fill);
                        iced::widget::stack(vec![board, backdrop.into(), card.into()])
                            .width(Length::Fixed(WIDTH))
                            .into()
                    } else {
                        board
                    }
                },
            ])
            .spacing(pages::PANEL_GAP)
            // Its foot level with the panel's, by the bar.
            .align_y(Alignment::End)
            .into(),
            Page::Network => pages::panel(self.network_page()),
            Page::Wifi => pages::panel(self.wifi_page()),
            Page::Vpn => pages::panel(self.vpn_page()),
            Page::Bluetooth => pages::panel(self.bluetooth_page()),
            Page::Power => pages::panel(self.power_page()),
            Page::Sound => pages::panel(self.sound_page()),
            Page::Displays => pages::panel(self.displays_page()),
            Page::Tiling => pages::panel(self.tiling_page()),
            Page::DoNotDisturb => pages::panel(self.dnd_page()),
            Page::Session => pages::panel(self.session_page()),
            Page::Camera => pages::panel(self.camera_page()),
            Page::Background => pages::panel(self.background_page()),
            Page::TrayMenu => pages::panel(self.tray_menu_page()),
        };
        // Editing is the main page in place: it fades, the others slide.
        let depth = if matches!(page, Page::Main | Page::Edit | Page::Background) { 0 } else { 1 };
        let content: Element<'_, Message> = if page == Page::Edit {
            content
        } else {
            widget::container(content).width(Length::Fixed(WIDTH)).into()
        };
        // The pages draw their own panels, apart, instead of the applet's
        // popup, all one: as applets' popups are sized.
        // Within a popup's limits still, on the host's surface as big as
        // the screen: as tall as the screen allows, no wider than editing.
        let limits = popup_limits(self.max_height());
        widget::container(Transition::new(page as u64, depth, content))
            .max_width(limits.max().width)
            .max_height(limits.max().height)
            .into()
    }

    fn on_close_requested(&self, id: window::Id) -> Option<Message> {
        Some(Message::Closed(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corner_does_not_listen_to_window_presses() {
        assert!(!listens_to_window_presses(true));
        assert!(listens_to_window_presses(false));
    }

    #[test]
    fn a_press_right_after_closing_does_not_reopen() {
        let now = std::time::Instant::now();
        assert!(press_just_closed(Some(now)));
        assert!(!press_just_closed(None));
        let old = now.checked_sub(Duration::from_millis(400)).unwrap();
        assert!(!press_just_closed(Some(old)));
    }

    #[test]
    fn camera_page_by_name() {
        assert_eq!(Page::from_name("camera"), Some(Page::Camera));
    }

    #[test]
    fn the_accessibility_protocol_only_for_the_tiles_that_show_it() {
        let with = |tile| vec![Placed::new(Kind::Tile(Tile::Wifi)), Placed::new(Kind::Tile(tile))];
        let never = |_: Tile| false;
        // Hosted: the panel's tiles decide, not the bar.
        assert!(wants_a11y(true, &with(Tile::Magnifier), never));
        assert!(wants_a11y(true, &with(Tile::InvertColors), never));
        assert!(!wants_a11y(true, &with(Tile::ScreenReader), |_| true));
        // On the bar: what the bar shows.
        assert!(wants_a11y(false, &[], |tile| tile == Tile::Magnifier));
        assert!(!wants_a11y(false, &with(Tile::Magnifier), never));
    }

    #[test]
    fn every_page_goes_by_its_name_and_back() {
        for page in Page::ALL {
            assert_eq!(Page::from_name(page.name()), Some(page));
        }
    }

    #[test]
    fn the_camera_is_not_on_the_panel_at_first() {
        assert!(!default_tiles().iter().any(|p| p.kind == Kind::Tile(Tile::Camera)));
    }
}
