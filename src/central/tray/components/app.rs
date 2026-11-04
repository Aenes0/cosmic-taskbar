// Copyright 2023 System76 <info@system76.com>, 2026 Aenes0
// SPDX-License-Identifier: GPL-3.0-only

use cosmic::{
    Task, app,
    applet::{
        cosmic_panel_config::PanelAnchor,
        token::subscription::{TokenRequest, TokenUpdate, activation_token_subscription},
    },
    cctk::sctk::reexports::calloop,
    cosmic_config::{self, CosmicConfigEntry},
    iced::{
        self, Event::Mouse, Subscription, event,
        keyboard::{self, key::{Key, Named}},
        mouse::{self, ScrollDelta},
        platform_specific::shell::commands::popup::destroy_popup,
        runtime::platform_specific::wayland::popup::SctkPopupSettings,
        window,
    },
    scroll::{DiscreteScrollDelta, DiscreteScrollState},
    surface,
};
use std::{
    collections::{BTreeMap, HashSet},
    time::{Duration, Instant},
};

use crate::central::tray::{
    components::{
        drag::{self, Origin},
        popups::{Cmd, Parent, Popups, Want},
        status_menu, view,
    },
    config::{CONFIG_ID, TrayConfig},
    dnd::TrayDrag,
    layout,
    subscriptions::{
        status_notifier_item::{StatusNotifierItem, StatusNotifierItemProxy},
        status_notifier_watcher,
    },
};

/// A press on the ^ this soon after the compositor closed the box is the same
/// click that closed it (the grab dismisses, then delivers the press): no reopen.
const DISMISS_CLICK_WINDOW: Duration = Duration::from_millis(300);
/// How often the state machine is nudged while a popup is still pending.
const POPUP_TICK: Duration = Duration::from_millis(100);

/// Whether a press at `now` is the click that the compositor used, at `at`, to
/// dismiss a popup: it must not open it again.
fn dismiss_blocks(at: Option<Instant>, now: Instant) -> bool {
    at.is_some_and(|t| now.saturating_duration_since(t) < DISMISS_CLICK_WINDOW)
}

/// Whether a destroy asked by a widget (libcosmic's tooltips, on leaving an
/// icon) goes to the compositor: only for a surface seen open. libcosmic
/// sends it for its tooltip even when that never opened, and destroying a
/// surface that does not exist drops the one popup waiting to be made (the
/// box, a menu) without a word (`event_loop/state.rs:1883`).
fn forward_destroy(open: &mut HashSet<window::Id>, id: window::Id) -> bool {
    open.remove(&id)
}

#[derive(Clone, Debug)]
pub enum Msg {
    None,
    Activate(usize),
    Closed(window::Id),
    /// The compositor confirmed the popup `id` exists.
    PopupOpened(window::Id),
    /// Nudge for the popup state machine while a request or a close is pending.
    PopupTick,
    Escape,
    // XXX don't use index (unique window id? or I guess that's created and destroyed)
    StatusMenu((usize, status_menu::Msg)),
    StatusNotifier(status_notifier_watcher::Event),
    TogglePopup(usize),
    Hovered(Option<usize>),
    WheelScrolled(ScrollDelta),
    Surface(surface::Action<Msg>),
    ToggleOverflow,
    HoveredOverflow,
    Token(TokenUpdate),
    /// menu_id, Id, Title read from the item's D-Bus properties.
    Identity(usize, String, String),
    Config(TrayConfig),
    /// Left click on an icon in the box: closes the box, then activates.
    ActivateFromBox(usize),
    /// Right click on an icon in the box: closes the box, then opens its menu.
    MenuFromBox(usize),
    /// A drag source (bar or box) started (`on_start`) or ended (`on_finish`/`on_cancel`).
    DragStart(usize, Origin),
    DragEnd,
    /// The drop target over the bar: pointer in the applet surface.
    BarEnter(f64, f64),
    BarMotion(f64, f64),
    BarLeave,
    /// Dropped on the bar / on the ^; `None` when the data is not ours.
    DropOnBar(Option<TrayDrag>),
    DropOnArrow(Option<TrayDrag>),
}

#[derive(Default)]
pub(crate) struct App {
    pub(super) core: app::Core,
    connection: Option<zbus::Connection>,
    pub(super) menus: BTreeMap<usize, status_menu::State>,
    max_menu_id: usize,
    /// Every decision about the box and the menus; the app only executes its commands.
    popups: Popups,
    /// The item whose menu we told over D-Bus is open.
    menu_notified: Option<usize>,
    pub(super) hovered_menu: Option<usize>,
    /// When the compositor last closed the box (click outside, grab).
    box_dismissed_at: Option<Instant>,
    /// Same for a menu: the item and when.
    menu_dismissed: Option<(usize, Instant)>,
    /// Surfaces seen open that are not the box or a menu (libcosmic's tooltips).
    others_open: HashSet<window::Id>,
    scroll_states: BTreeMap<usize, DiscreteScrollState>,
    token_tx: Option<calloop::channel::Sender<TokenRequest>>,
    pub(super) config: TrayConfig,
    config_handle: Option<cosmic_config::Config>,
    /// A drag started here (bar or box).
    pub(super) drag: Option<drag::Drag>,
    /// Gap under the pointer while a drag is over the bar.
    pub(super) preview_slot: Option<usize>,
    /// Embedded in a bigger applet ("corner"): the host owns the window size.
    corner: bool,
    /// Length (px) along the major axis taken by the others in the corner.
    reserved: f32,
}

/// The major-axis bound left for the tray after the host's `reserved` px
/// (never below 0).
fn bounded_len(bounds: Option<f32>, reserved: f32) -> Option<f32> {
    bounds.map(|len| (len - reserved.max(0.0)).max(0.0))
}

/// Only the standalone tray owns its window; in the corner the host does.
fn resizes_window(corner: bool) -> bool {
    !corner
}

impl App {
    fn next_menu_id(&mut self) -> usize {
        self.max_menu_id += 1;
        self.max_menu_id
    }

    pub(super) fn is_vertical(&self) -> bool {
        matches!(
            self.core.applet.anchor,
            PanelAnchor::Left | PanelAnchor::Right
        )
    }

    /// Length of one button along the panel; always > 0.
    pub(crate) fn button_len(&self) -> f32 {
        let len = f32::from(self.core.applet.suggested_size(true).0)
            + 2.0 * f32::from(self.core.applet.suggested_padding(true).1);
        len.max(1.0)
    }

    /// A drag started on the bar: the ^ shows up as a target.
    pub(super) fn dragging_from_bar(&self) -> bool {
        self.drag.as_ref().is_some_and(|d| d.origin == Origin::Bar)
    }

    /// Who goes on the bar and who goes in the box, right now.
    pub(super) fn split(&self) -> layout::Split {
        let items: Vec<layout::Present<'_>> = self
            .menus
            .iter()
            .map(|(id, menu)| (*id, menu.identity.key()))
            .collect();
        // With bounds for both axes the panel put us in its own overflow:
        // as upstream, only the capacity changes.
        let major_axis_len = self
            .core
            .applet
            .suggested_bounds
            .as_ref()
            .map(|c| if self.is_vertical() { c.height } else { c.width })
            .filter(|len| *len > 0.0);
        let major_axis_len = bounded_len(major_axis_len, self.reserved);
        layout::split(
            &items,
            &self.config.visible,
            layout::capacity(major_axis_len, self.button_len()),
            self.dragging_from_bar(),
        )
    }

    /// Slot along the bar where the menu of `id` is anchored: its own button
    /// if on the bar, otherwise the ^.
    fn slot_of(&self, id: usize) -> usize {
        let split = self.split();
        split
            .bar
            .iter()
            .position(|&b| b == id)
            .unwrap_or(split.bar.len())
    }

    fn popup_settings(
        &self,
        parent: window::Id,
        popup_id: window::Id,
        slot: usize,
    ) -> SctkPopupSettings {
        let mut settings = self
            .core
            .applet
            .get_popup_settings(parent, popup_id, None, None, None);
        let offset = (slot as f32 * self.button_len()) as i32;
        if self.is_vertical() {
            settings.positioner.anchor_rect.y = offset;
        } else {
            settings.positioner.anchor_rect.x = offset;
        }
        settings
    }

    /// Fits the applet to the bar as drawn: icons, the gap of a drag, the ^.
    pub(super) fn resize_window(&self) -> app::Task<Msg> {
        if !resizes_window(self.corner) {
            return Task::none();
        }
        let Some(main_id) = self.core.main_window_id() else {
            return Task::none();
        };
        let split = self.split();
        let n = self.bar_row(&split).cells.len() + usize::from(split.show_arrow);
        let len = self.button_len();
        let major = (len * n as f32).max(1.0);
        let size = if self.is_vertical() {
            iced::Size::new(len, major)
        } else {
            iced::Size::new(major, len)
        };
        window::resize(main_id, size)
    }

    /// After items, identities or the config change: resize, and close the
    /// box if nothing is left in it.
    fn relayout(&mut self) -> app::Task<Msg> {
        let mut cmds = vec![self.resize_window()];
        if self.popups.box_id().is_some() && self.split().boxed.is_empty() {
            cmds.push(self.close_box());
        }
        Task::batch(cmds)
    }

    /// Updates the visible list, writes it and resizes; no-op if unchanged.
    pub(super) fn set_visible(&mut self, visible: Vec<String>) -> app::Task<Msg> {
        if self.config.visible == visible {
            return Task::none();
        }
        match self.config_handle.as_ref() {
            Some(handle) => {
                if let Err(why) = self.config.set_visible(handle, visible) {
                    tracing::error!(why = why.to_string(), "Failed to write the visible list");
                }
            }
            None => self.config.visible = visible,
        }
        self.relayout()
    }

    /// Closes the box if it is open (its menu goes with it).
    pub(super) fn close_box(&mut self) -> app::Task<Msg> {
        if !self.popups.is_box_open() {
            return Task::none();
        }
        let cmds = self.popups.toggle_box(Instant::now(), window::Id::unique());
        self.run(cmds)
    }

    fn open_menu_item(&self) -> Option<usize> {
        self.popups.menu_item().map(|(item, _)| item)
    }

    /// Tells the item over D-Bus when its menu enters or leaves the state
    /// (requested counts as open, as before).
    fn sync_menu_events(&mut self) {
        let now_open = self.open_menu_item();
        if now_open == self.menu_notified {
            return;
        }
        if let Some(menu) = self.menu_notified.and_then(|id| self.menus.get(&id)) {
            menu.closed();
        }
        if let Some(menu) = now_open.and_then(|id| self.menus.get(&id)) {
            menu.opened();
        }
        self.menu_notified = now_open;
    }

    /// Executes what the state machine decided.
    fn run(&mut self, cmds: Vec<Cmd>) -> app::Task<Msg> {
        self.sync_menu_events();
        let mut tasks = Vec::with_capacity(cmds.len());
        for cmd in cmds {
            tracing::debug!(?cmd, "popup command");
            match cmd {
                Cmd::Destroy(id) => tasks.push(destroy_popup(id)),
                Cmd::Open { want, id, parent } => {
                    tasks.push(cosmic::surface::surface_task(
                        cosmic::surface::action::app_popup(
                            |_| Default::default(),
                            move |app: &mut Self| app.settings_for(want, id, parent),
                            None,
                        ),
                    ));
                }
            }
        }
        Task::batch(tasks)
    }

    /// Positioning of a requested popup. Pure with respect to the state: the
    /// machine alone decides what is open.
    fn settings_for(&self, want: Want, id: window::Id, parent: Parent) -> SctkPopupSettings {
        let main = self
            .core
            .main_window_id()
            .unwrap_or_else(window::Id::unique);
        match (want, parent) {
            (Want::Box, _) => {
                let mut settings = self.popup_settings(main, id, self.split().bar.len());
                settings.close_with_children = false;
                settings
            }
            (Want::Menu { item, in_box: true }, Parent::Box(box_id)) => {
                let mut settings = self
                    .core
                    .applet
                    .get_popup_settings(box_id, id, None, None, None);
                let index = self
                    .split()
                    .boxed
                    .iter()
                    .position(|&b| b == item)
                    .unwrap_or(0);
                settings.positioner.anchor_rect = view::box_cell_anchor(
                    index,
                    view::BOX_COLUMNS,
                    self.button_len(),
                    view::BOX_PADDING,
                );
                settings
            }
            (Want::Menu { item, .. }, _) => self.popup_settings(main, id, self.slot_of(item)),
        }
    }

    fn build(core: app::Core, corner: bool) -> (Self, app::Task<Msg>) {
        let mut core = core;
        // See-through, but no blur behind: the theme stays transparent while any
        // kind is set, and only layer surfaces, which there are none of here, blur.
        core.set_auto_blur(cosmic::core::Auto::System.into());
        let (config_handle, config) =
            match cosmic_config::Config::new(CONFIG_ID, TrayConfig::VERSION) {
                Ok(handle) => {
                    let config = TrayConfig::get_entry(&handle).unwrap_or_else(|(errors, c)| {
                        for why in errors.into_iter().filter(cosmic_config::Error::is_err) {
                            tracing::error!(why = why.to_string(), "Error reading config");
                        }
                        c
                    });
                    (Some(handle), config)
                }
                Err(why) => {
                    tracing::error!(why = why.to_string(), "Error opening config");
                    (None, TrayConfig::default())
                }
            };
        (
            Self {
                core,
                config,
                config_handle,
                corner,
                ..Self::default()
            },
            Task::none(),
        )
    }

    /// Same as `init`, but embedded in a bigger applet: it does not size the window.
    pub(crate) fn in_corner(core: app::Core) -> (Self, app::Task<Msg>) {
        Self::build(core, true)
    }

    /// The bar as a plain element, for the host applet to place (no autosize).
    pub(crate) fn corner_view(&self) -> cosmic::Element<'_, Msg> {
        view::corner_view(self)
    }

    /// Length (px) the host uses along the major axis; the tray gets the rest.
    pub(crate) fn set_reserved(&mut self, px: f32) {
        self.reserved = px.max(0.0);
    }
}

impl cosmic::Application for App {
    type Message = Msg;
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    const APP_ID: &'static str = crate::central::tray::APP_ID;

    fn init(core: app::Core, _flags: ()) -> (Self, app::Task<Msg>) {
        Self::build(core, false)
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

    fn update(&mut self, message: Msg) -> app::Task<Msg> {
        match message {
            Msg::None => Task::none(),
            Msg::Activate(id) => {
                if let Some(token_tx) = self.token_tx.as_ref() {
                    let _ = token_tx.send(TokenRequest {
                        app_id: Self::APP_ID.to_string(),
                        exec: format!("activate:{}", id),
                    });
                } else if let Some(menu) = self.menus.get(&id) {
                    return activate(id, &menu.item, None);
                }
                Task::none()
            }
            Msg::ActivateFromBox(id) => {
                let close = self.close_box();
                Task::batch([close, self.update(Msg::Activate(id))])
            }
            // The menu opens as a child of the box, which stays.
            Msg::MenuFromBox(id) => {
                if !self.menus.contains_key(&id) {
                    return Task::none();
                }
                let cmds = self.popups.menu(id, true, Instant::now(), window::Id::unique());
                self.run(cmds)
            }
            Msg::PopupOpened(id) => {
                tracing::debug!(?id, "popup opened");
                let ours = self.popups.box_id() == Some(id)
                    || self.popups.menu_item().is_some_and(|(_, m)| m == id);
                if !ours {
                    self.others_open.insert(id);
                }
                let cmds = self.popups.opened(id, Instant::now());
                self.run(cmds)
            }
            Msg::PopupTick => {
                let cmds = self.popups.tick(Instant::now(), window::Id::unique());
                self.run(cmds)
            }
            Msg::Escape => {
                let cmds = self.popups.escape(Instant::now());
                self.run(cmds)
            }
            Msg::Closed(surface) => {
                self.others_open.remove(&surface);
                let now = Instant::now();
                // Only what we did not destroy is still "open" here: the
                // compositor closed it (click outside, grab).
                let box_by_compositor = self.popups.box_id() == Some(surface);
                let menu_by_compositor = self
                    .popups
                    .menu_item()
                    .filter(|(_, id)| *id == surface)
                    .map(|(item, _)| item);
                tracing::debug!(?surface, box_by_compositor, ?menu_by_compositor, "popup closed");
                let cmds = self.popups.closed(surface, now, window::Id::unique());
                let mut tasks = vec![self.run(cmds)];
                if let Some(item) = menu_by_compositor {
                    self.menu_dismissed = Some((item, now));
                }
                if box_by_compositor {
                    self.hovered_menu = None;
                    self.box_dismissed_at = Some(now);
                    tasks.push(self.box_closed());
                }
                Task::batch(tasks)
            }
            Msg::StatusMenu((id, msg)) => match self.menus.get_mut(&id) {
                Some(state) => state
                    .update(msg, id, self.token_tx.as_ref())
                    .map(move |msg| cosmic::action::app(Msg::StatusMenu((id, msg)))),
                None => Task::none(),
            },
            Msg::StatusNotifier(event) => match event {
                status_notifier_watcher::Event::Connected(connection) => {
                    self.connection = Some(connection);
                    Task::none()
                }
                status_notifier_watcher::Event::Registered(name) => {
                    let (state, cmd) = status_menu::State::new(name);
                    let proxy = state.item.item_proxy().clone();
                    let existing = self
                        .menus
                        .iter()
                        .find(|(_, prev_state)| prev_state.name() == state.name())
                        .map(|(id, _)| *id);
                    // A re-registered name gets a fresh State (identity unknown
                    // again), so its identity is read again too.
                    let id = existing.unwrap_or_else(|| self.next_menu_id());
                    self.menus.insert(id, state);
                    app::Task::batch([
                        self.relayout(),
                        read_identity(id, proxy),
                        cmd.map(move |msg| cosmic::action::app(Msg::StatusMenu((id, msg)))),
                    ])
                }
                status_notifier_watcher::Event::Unregistered(name) => {
                    let mut cmds = Vec::new();
                    if let Some((id, _)) = self.menus.iter().find(|(_id, menu)| menu.name() == name)
                    {
                        let id = *id;
                        self.menus.remove(&id);
                        self.scroll_states.remove(&id);
                        if self.hovered_menu == Some(id) {
                            self.hovered_menu = None;
                        }
                        // Its drag source is gone with it: `DragEnd` will not
                        // come, and a stale drag would force the ^ and keep
                        // its key off the bar.
                        if self.drag.as_ref().is_some_and(|d| d.menu_id == id) {
                            self.drag = None;
                            self.preview_slot = None;
                        }
                        if self.open_menu_item() == Some(id) {
                            let close = self.popups.escape(Instant::now());
                            cmds.push(self.run(close));
                        }
                    }
                    cmds.push(self.relayout());
                    Task::batch(cmds)
                }
                status_notifier_watcher::Event::Error(err) => {
                    tracing::error!("Status notifier error: {err}");
                    Task::none()
                }
            },
            Msg::Identity(id, sni_id, title) => {
                let Some(menu) = self.menus.get_mut(&id) else {
                    return Task::none();
                };
                menu.identity.learn(&sni_id, &title);
                self.relayout()
            }
            Msg::Config(config) => {
                if self.config == config {
                    return Task::none();
                }
                self.config = config;
                self.relayout()
            }
            Msg::TogglePopup(id) => {
                if !self.menus.contains_key(&id) {
                    return Task::none();
                }
                let now = Instant::now();
                let reopens = self.open_menu_item() != Some(id)
                    && self
                        .menu_dismissed
                        .is_some_and(|(item, at)| item == id && dismiss_blocks(Some(at), now));
                self.menu_dismissed = None;
                if reopens {
                    return Task::none();
                }
                let cmds = self.popups.menu(id, false, now, window::Id::unique());
                self.run(cmds)
            }
            Msg::Token(u) => match u {
                TokenUpdate::Init(tx) => {
                    self.token_tx = Some(tx);
                    Task::none()
                }
                TokenUpdate::Finished => {
                    self.token_tx = None;
                    Task::none()
                }
                TokenUpdate::ActivationToken { token, exec: id } => {
                    if let Some(id_str) = id.strip_prefix("activate:") {
                        if let Ok(real_id) = id_str.parse::<usize>()
                            && let Some(menu) = self.menus.get(&real_id)
                        {
                            return activate(real_id, &menu.item, token.clone());
                        }
                        return Task::none();
                    }
                    if let Some(((state, id), token)) = str::parse(&id)
                        .ok()
                        .and_then(|id: usize| self.menus.get_mut(&id).map(|m| (m, id)))
                        .zip(token)
                    {
                        return state
                            .update(
                                status_menu::Msg::ClickToken(token),
                                id,
                                self.token_tx.as_ref(),
                            )
                            .map(move |msg| cosmic::action::app(Msg::StatusMenu((id, msg))));
                    }
                    Task::none()
                }
            },
            Msg::Hovered(id) => {
                self.hovered_menu = id;
                // With a menu open, hovering another icon of the bar switches
                // to its menu; icons of the box do not.
                match (id, self.open_menu_item()) {
                    (Some(id), Some(open)) if open != id && self.split().bar.contains(&id) => {
                        let cmds = self.popups.menu(id, false, Instant::now(), window::Id::unique());
                        self.run(cmds)
                    }
                    _ => Task::none(),
                }
            }
            Msg::WheelScrolled(delta) => {
                let Some(id) = self.hovered_menu else {
                    return Task::none();
                };
                let discrete_delta = self.scroll_states.entry(id).or_default().update(delta);
                let Some((delta, orientation)) = discrete_scroll_delta(discrete_delta) else {
                    return Task::none();
                };
                let Some(menu) = self.menus.get(&id) else {
                    return Task::none();
                };
                scroll(id, menu.item.item_proxy().clone(), delta, orientation)
            }
            Msg::Surface(surface::Action::DestroyPopup(id)) => {
                if forward_destroy(&mut self.others_open, id) {
                    return cosmic::surface::surface_task(surface::Action::DestroyPopup(id));
                }
                tracing::debug!(?id, "destroy of a surface never open, dropped");
                Task::none()
            }
            Msg::Surface(a) => cosmic::surface::surface_task(a),
            Msg::ToggleOverflow => {
                let now = Instant::now();
                let opening = self.popups.box_id().is_none();
                let blocked = opening && dismiss_blocks(self.box_dismissed_at, now);
                self.box_dismissed_at = None;
                if blocked || (opening && self.split().boxed.is_empty()) {
                    return Task::none();
                }
                let cmds = self.popups.toggle_box(now, window::Id::unique());
                self.run(cmds)
            }
            message @ (Msg::DragStart(..)
            | Msg::DragEnd
            | Msg::BarEnter(..)
            | Msg::BarMotion(..)
            | Msg::BarLeave
            | Msg::DropOnBar(_)
            | Msg::DropOnArrow(_)) => self.update_drag(message),
            Msg::HoveredOverflow => {
                self.hovered_menu = None;
                // With a menu open, hovering the ^ switches to the box.
                if self.popups.box_id().is_some()
                    || self.open_menu_item().is_none()
                    || self.split().boxed.is_empty()
                {
                    return Task::none();
                }
                let cmds = self.popups.toggle_box(Instant::now(), window::Id::unique());
                self.run(cmds)
            }
        }
    }

    fn subscription(&self) -> Subscription<Msg> {
        let mut subscriptions = Vec::new();

        subscriptions.push(status_notifier_watcher::subscription().map(Msg::StatusNotifier));

        for (id, menu) in &self.menus {
            let is_open = self.open_menu_item() == Some(*id);
            subscriptions.push(menu.subscription(is_open).with(*id).map(Msg::StatusMenu));
        }
        subscriptions.push(activation_token_subscription(0).map(Msg::Token));
        subscriptions.push(event::listen_with(|e, status, id| match (e, status) {
            (Mouse(mouse::Event::WheelScrolled { delta }), event::Status::Ignored) => {
                Some(Msg::WheelScrolled(delta))
            }
            (iced::Event::Window(window::Event::Opened { .. }), _) => Some(Msg::PopupOpened(id)),
            (
                iced::Event::Keyboard(keyboard::Event::KeyPressed {
                    key: Key::Named(Named::Escape),
                    ..
                }),
                _,
            ) => Some(Msg::Escape),
            _ => None,
        }));
        if self.popups.pending() {
            subscriptions.push(iced::time::every(POPUP_TICK).map(|_| Msg::PopupTick));
        }
        subscriptions.push(self.core.watch_config::<TrayConfig>(CONFIG_ID).map(|u| {
            for why in u.errors.into_iter().filter(cosmic_config::Error::is_err) {
                tracing::error!(why = why.to_string(), "Error watching config");
            }
            Msg::Config(u.config)
        }));

        iced::Subscription::batch(subscriptions)
    }

    fn view(&self) -> cosmic::Element<'_, Msg> {
        view::view(self)
    }

    fn view_window(&self, surface: window::Id) -> cosmic::Element<'_, Msg> {
        if self.popups.box_id() == Some(surface) {
            return view::view_box(self, surface);
        }

        let theme = self.core.system_theme();
        let cosmic = theme.cosmic();
        let corners = cosmic.corner_radii;
        let pad = corners.radius_m[0];
        let open = self
            .popups
            .menu_item()
            .filter(|(_, id)| *id == surface)
            .and_then(|(item, _)| Some((item, self.menus.get(&item)?)));
        match open {
            Some((id, menu)) => self
                .core
                .applet
                .popup_container(
                    cosmic::widget::container(
                        menu.popup_view().map(move |msg| Msg::StatusMenu((id, msg))),
                    )
                    .padding([pad, 0.]),
                )
                .into(),
            None => iced::widget::text("").into(),
        }
    }

    fn on_close_requested(&self, id: window::Id) -> Option<Msg> {
        Some(Msg::Closed(id))
    }
}

/// Reads Id and Title once, together, so a Title cannot fix the key before the Id.
fn read_identity(id: usize, proxy: StatusNotifierItemProxy<'static>) -> app::Task<Msg> {
    Task::future(async move {
        let sni_id = proxy.id().await.unwrap_or_else(|why| {
            tracing::warn!("Could not read Id of item {id}: {why}");
            String::new()
        });
        let title = proxy.title().await.unwrap_or_else(|why| {
            tracing::warn!("Could not read Title of item {id}: {why}");
            String::new()
        });
        cosmic::action::app(Msg::Identity(id, sni_id, title))
    })
}

fn activate(
    id: usize,
    item: &StatusNotifierItem,
    activation_token: Option<String>,
) -> Task<cosmic::Action<Msg>> {
    if item.is_menu() {
        return Task::done(cosmic::action::app(Msg::TogglePopup(id)));
    }
    let item_proxy = item.item_proxy().clone();
    Task::future(async move {
        if let Some(t) = activation_token {
            match item_proxy.provide_xdg_activation_token(t).await {
                Ok(_) => {
                    tracing::debug!("Token provided successfully to {}", id)
                }
                Err(e) => tracing::error!("Failed to provide token to {}: {}", id, e),
            }
        }
        match item_proxy.activate(0, 0).await {
            Ok(_) => cosmic::action::app(Msg::None),
            Err(err) => {
                tracing::error!("Activate failed: {}", err);
                cosmic::action::app(Msg::TogglePopup(id))
            }
        }
    })
}

fn scroll(
    id: usize,
    item_proxy: crate::central::tray::subscriptions::status_notifier_item::StatusNotifierItemProxy<'static>,
    delta: i32,
    orientation: &'static str,
) -> Task<cosmic::Action<Msg>> {
    Task::future(async move {
        match item_proxy.scroll(delta, orientation).await {
            Ok(_) => cosmic::action::app(Msg::None),
            Err(err) => {
                tracing::error!("Scroll failed for {}: {}", id, err);
                cosmic::action::app(Msg::None)
            }
        }
    })
}

fn discrete_scroll_delta(delta: DiscreteScrollDelta) -> Option<(i32, &'static str)> {
    if delta.y != 0 {
        Some((delta.y as i32, "vertical"))
    } else if delta.x != 0 {
        Some((delta.x as i32, "horizontal"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tooltip_never_open_is_not_destroyed() {
        let mut open = HashSet::new();
        let tooltip = window::Id::unique();
        assert!(!forward_destroy(&mut open, tooltip), "never seen open: dropped");
        open.insert(tooltip);
        assert!(forward_destroy(&mut open, tooltip), "seen open: goes");
        assert!(!forward_destroy(&mut open, tooltip), "once");
    }

    #[test]
    fn a_press_right_after_the_dismissal_is_the_same_click() {
        let t = Instant::now();
        assert!(dismiss_blocks(Some(t), t + Duration::from_millis(50)));
        assert!(!dismiss_blocks(Some(t), t + DISMISS_CLICK_WINDOW));
        assert!(!dismiss_blocks(None, t));
    }

    #[test]
    fn reserved_is_taken_from_the_major_axis() {
        let bounds = bounded_len(Some(300.0), 90.0);
        assert_eq!(bounds, Some(210.0));
        assert_eq!(layout::capacity(bounds, 30.0), Some(7));
    }

    #[test]
    fn reserved_never_goes_below_zero_and_none_stays_none() {
        assert_eq!(bounded_len(Some(50.0), 90.0), Some(0.0));
        assert_eq!(bounded_len(None, 90.0), None);
        assert_eq!(bounded_len(Some(50.0), -5.0), Some(50.0));
    }

    #[test]
    fn corner_does_not_resize_the_window() {
        assert!(!resizes_window(true));
        assert!(resizes_window(false));
    }

    #[test]
    fn discrete_scroll_prefers_vertical_delta() {
        assert_eq!(
            discrete_scroll_delta(cosmic::scroll::DiscreteScrollDelta { x: 4, y: -2 }),
            Some((-2, "vertical"))
        );
    }

    #[test]
    fn discrete_scroll_uses_horizontal_delta_when_vertical_is_zero() {
        assert_eq!(
            discrete_scroll_delta(cosmic::scroll::DiscreteScrollDelta { x: 3, y: 0 }),
            Some((3, "horizontal"))
        );
    }

    #[test]
    fn discrete_scroll_ignores_zero_delta() {
        assert_eq!(
            discrete_scroll_delta(cosmic::scroll::DiscreteScrollDelta { x: 0, y: 0 }),
            None
        );
    }

    #[test]
    fn pixel_scroll_accumulates_before_emitting_discrete_delta() {
        let mut state = cosmic::scroll::DiscreteScrollState::default();

        let first = state.update(ScrollDelta::Pixels { x: 0.0, y: 12.0 });
        assert_eq!(discrete_scroll_delta(first), None);

        let second = state.update(ScrollDelta::Pixels { x: 0.0, y: 12.0 });
        assert_eq!(discrete_scroll_delta(second), Some((1, "vertical")));
    }
}
