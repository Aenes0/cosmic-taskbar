// SPDX-License-Identifier: GPL-3.0-only

//! Isolated Alt+Tab / Super+Tab window switcher (`io.github.Aenes0.CosmicSwitcher`).
//!
//! Runs as its own process (`cosmic-taskbar switcher`), connects directly to
//! `cosmic-comp` over Wayland `layer-shell` on Vulkan GPU (`wgpu`), and owns
//! its own D-Bus name so it never collides with system `cosmic-workspaces`.

mod icons;
mod service;
mod state;
mod view;
mod wayland;

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use cosmic::{
    Element, Task, app,
    cctk::sctk::reexports::{calloop, client::protocol::wl_output::WlOutput},
    iced::{
        self, Limits, Subscription, event,
        keyboard,
        mouse::ScrollDelta,
        platform_specific::{
            runtime::wayland::layer_surface::{IcedOutput, SctkLayerSurfaceSettings},
            shell::commands::layer_surface::{
                Anchor, KeyboardInteractivity, Layer, destroy_layer_surface, get_layer_surface,
            },
        },
        window,
    },
    widget,
};

use crate::shared::preview_card::Picture;

use icons::IconResolver;
use service::DbusEvent;
use state::{
    CycleSource, GRID_GAP, GridSpec, HEADER_H, HELD_CHECK_MS, KeyAction, LABEL_H, Mru, Outcome,
    PANEL_DELAY_MS, PANEL_PADDING, SwitcherState, key_action, layout, output_named, should_dedup,
};
use wayland::{CAPTURE_ACTIVE, WaylandCmd, WaylandEvent, WinInfo};

pub const APP_ID: &str = "io.github.Aenes0.CosmicSwitcher";

/// Safety net so an Alt+Tab grab can never remain stuck if a modifier release is missed.
const SWITCHER_SAFETY_TIMEOUT: Duration = Duration::from_secs(10);

/// Entry point for `cosmic-taskbar switcher` / `--switcher`.
pub fn run() -> cosmic::iced::Result {
    // Resident service: two tokio workers are plenty.
    if std::env::var_os("TOKIO_WORKER_THREADS").is_none() {
        // SAFETY: called before any threads start.
        unsafe { std::env::set_var("TOKIO_WORKER_THREADS", "2") };
    }
    crate::shared::logging::init(
        "warn,cosmic_taskbar=info,usvg=error",
        &["cosmic::applet", "iced_core::clipboard", "cosmic_config", "sctk"],
    );
    let _ = tracing_log::LogTracer::init();

    let args: Vec<String> = std::env::args().skip(2).collect();
    if service::forward_cli(&args) {
        return Ok(());
    }

    // Do NOT override ICED_BACKEND: switcher connects directly to cosmic-comp
    // via layer-shell and renders on Vulkan GPU (`wgpu`).
    crate::shared::journal::own_output("cosmic-switcher");

    let settings = cosmic::app::Settings::default()
        .no_main_window(true)
        .exit_on_close(false)
        .transparent(true);
    cosmic::app::run::<SwitcherApp>(settings, ())
}

#[derive(Clone, Debug)]
pub enum Msg {
    Dbus(DbusEvent),
    Wayland(WaylandEvent),
    Output(String, WlOutput, f32, f32),
    OutputGone(WlOutput),
    KeyAction(KeyAction, bool),
    ModifiersChanged { alt: bool, logo: bool },
    OnScroll(ScrollDelta),
    ShowPanel(u64),
    HeldCheck(u64),
    SafetyTimeout(u64),
    Activate(String),
    CloseWindow(String),
    Hover(Option<String>),
    Cancel,
    Beat,
    Trim,
    Ignore,
}

struct Output {
    name: String,
    handle: WlOutput,
    width: f32,
    height: f32,
}

pub struct SwitcherApp {
    core: app::Core,
    outputs: Vec<Output>,
    surface: Option<window::Id>,
    active_output: Option<String>,
    wayland_tx: Option<calloop::channel::Sender<WaylandCmd>>,
    pub(crate) windows: HashMap<String, WinInfo>,
    window_order: Vec<String>,
    mru: Mru<String>,
    pub(crate) switcher: Option<SwitcherState<String>>,
    gen_id: u64,
    last_cycle: Option<(Instant, CycleSource)>,
    pub(crate) thumbs: HashMap<String, Picture>,
    pub(crate) icons: IconResolver,
    pub(crate) hovered: Option<String>,
}

impl SwitcherApp {
    /// Available inner width and height for the grid on the active output.
    pub(crate) fn switcher_avail(&self) -> (f32, f32) {
        let (out_w, out_h) = self
            .active_output
            .as_deref()
            .and_then(|name| self.outputs.iter().find(|o| o.name == name))
            .or_else(|| self.outputs.first())
            .map_or((1920.0, 1080.0), |o| (o.width, o.height));
        let avail_w = (out_w * 0.86 - 2.0 * PANEL_PADDING).max(320.0);
        let avail_h = (out_h * 0.78 - 2.0 * PANEL_PADDING - HEADER_H - 14.0).max(220.0);
        (avail_w, avail_h)
    }

    pub(crate) fn switcher_grid(&self) -> GridSpec {
        let (avail_w, avail_h) = self.switcher_avail();
        let count = self.switcher.as_ref().map_or(0, |s| s.order().len());
        layout(avail_w, avail_h, count, GRID_GAP, LABEL_H)
    }

    fn focused_output_name(&self) -> Option<String> {
        let ordered = self.mru.ordered(&self.window_order);
        ordered
            .first()
            .and_then(|id| self.windows.get(id))
            .and_then(|w| w.outputs.first().cloned())
            .or_else(|| self.outputs.first().map(|o| o.name.clone()))
    }

    fn resolve_output(&self, requested: &str) -> (Option<String>, IcedOutput) {
        let known: Vec<(String, WlOutput)> = self
            .outputs
            .iter()
            .map(|o| (o.name.clone(), o.handle.clone()))
            .collect();
        if let Some(handle) = output_named(requested, &known) {
            return (Some(requested.to_owned()), IcedOutput::Output(handle));
        }
        if let Some(focused) = self.focused_output_name()
            && let Some(handle) = output_named(&focused, &known)
        {
            return (Some(focused), IcedOutput::Output(handle));
        }
        (None, IcedOutput::Active)
    }

    fn open_switcher(
        &mut self,
        reverse: bool,
        requested_output: &str,
        sticky: bool,
    ) -> app::Task<Msg> {
        // Toggle closed if already open in sticky overview mode.
        if sticky && self.switcher.as_ref().is_some_and(SwitcherState::is_sticky) {
            return self.hide();
        }
        // If already open in Alt+Tab mode, another shortcut press advances selection.
        if !sticky && self.switcher.is_some() {
            return self.cycle(if reverse { -1 } else { 1 }, CycleSource::Dbus);
        }

        let mut tasks: Vec<app::Task<Msg>> = Vec::new();
        if self.surface.is_some() {
            tasks.push(self.hide());
        }

        let ordered = self.mru.ordered(&self.window_order);
        let state = SwitcherState::open(ordered.clone(), reverse, sticky);
        self.switcher = Some(state);
        self.gen_id += 1;
        let gen_id = self.gen_id;
        self.hovered = None;
        self.last_cycle = None;

        // Pre-resolve icons before view() runs.
        self.icons
            .ensure_all(self.windows.values().map(|w| w.app_id.as_str()));

        // Start live thumbnail captures right away so they are ready when the panel shows.
        CAPTURE_ACTIVE.store(true, Ordering::Relaxed);
        if let Some(tx) = &self.wayland_tx {
            let _ = tx.send(WaylandCmd::Capture(ordered));
        }

        let (out_name, iced_output) = self.resolve_output(requested_output);
        self.active_output = out_name;

        let id = window::Id::unique();
        self.surface = Some(id);
        tasks.push(get_layer_surface(SctkLayerSurfaceSettings {
            id,
            layer: Layer::Overlay,
            keyboard_interactivity: KeyboardInteractivity::Exclusive,
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            output: iced_output,
            namespace: "cosmic-switcher".into(),
            size: Some((None, None)),
            exclusive_zone: -1,
            size_limits: Limits::NONE.min_width(1.0).min_height(1.0),
            ..Default::default()
        }));

        if !sticky {
            tasks.push(Task::perform(
                tokio::time::sleep(Duration::from_millis(PANEL_DELAY_MS)),
                move |()| cosmic::Action::App(Msg::ShowPanel(gen_id)),
            ));
            tasks.push(Task::perform(
                tokio::time::sleep(Duration::from_millis(HELD_CHECK_MS)),
                move |()| cosmic::Action::App(Msg::HeldCheck(gen_id)),
            ));
            tasks.push(Task::perform(
                tokio::time::sleep(SWITCHER_SAFETY_TIMEOUT),
                move |()| cosmic::Action::App(Msg::SafetyTimeout(gen_id)),
            ));
        }

        Task::batch(tasks)
    }

    fn cycle(&mut self, delta: i32, source: CycleSource) -> app::Task<Msg> {
        let now = Instant::now();
        if should_dedup(self.last_cycle, now, source) {
            return Task::none();
        }
        let Some(state) = self.switcher.as_mut() else {
            return Task::none();
        };
        state.cycle(delta);
        // Pressing Tab a second time shows the panel immediately without waiting.
        if !state.panel_visible() {
            state.show_panel();
        }
        self.last_cycle = Some((now, source));

        let index = state.selected_index();
        let total = state.order().len();
        let grid = self.switcher_grid();
        if grid.scrolls && total > 0 {
            let rows = total.div_ceil(grid.cols);
            let fraction = if rows > 1 {
                (index / grid.cols) as f32 / (rows - 1) as f32
            } else {
                0.0
            };
            cosmic::iced::runtime::widget::operation::snap_to(
                view::SWITCHER_SCROLL_ID.clone(),
                iced::widget::scrollable::RelativeOffset {
                    x: None,
                    y: Some(fraction),
                },
            )
        } else {
            Task::none()
        }
    }

    fn confirm(&mut self) -> app::Task<Msg> {
        if let Some(state) = self.switcher.take()
            && let Outcome::Activate(id) = state.release()
        {
            self.activate_window(id);
        }
        self.hide()
    }

    fn activate_window(&mut self, id: String) {
        self.mru.touch(id.clone());
        if let Some(tx) = &self.wayland_tx {
            let _ = tx.send(WaylandCmd::Activate(id));
        }
    }

    fn hide(&mut self) -> app::Task<Msg> {
        self.switcher = None;
        self.active_output = None;
        self.hovered = None;
        self.last_cycle = None;
        CAPTURE_ACTIVE.store(false, Ordering::Relaxed);
        self.thumbs.clear();

        let Some(id) = self.surface.take() else {
            return Task::none();
        };
        let trim = Task::perform(tokio::time::sleep(Duration::from_millis(500)), |()| {
            cosmic::Action::App(Msg::Trim)
        });
        Task::batch([destroy_layer_surface(id), trim])
    }

    fn update_windows(&mut self, list: Vec<WinInfo>) {
        let prev_active: HashMap<String, bool> = self
            .windows
            .iter()
            .map(|(id, w)| (id.clone(), w.active))
            .collect();

        let mut next_map = HashMap::with_capacity(list.len());
        let mut present_ids = Vec::with_capacity(list.len());

        for win in list {
            let was_active = prev_active.get(&win.id).copied().unwrap_or(false);
            if win.active && !was_active {
                self.mru.touch(win.id.clone());
            }
            present_ids.push(win.id.clone());
            next_map.insert(win.id.clone(), win);
        }

        // Remove closed windows from MRU, active switcher session, and thumbnail cache.
        self.mru.retain_present(&present_ids);
        self.thumbs.retain(|id, _| next_map.contains_key(id));
        if let Some(state) = self.switcher.as_mut() {
            let closed: Vec<String> = state
                .order()
                .iter()
                .filter(|id| !next_map.contains_key(*id))
                .cloned()
                .collect();
            for id in closed {
                state.remove(&id);
            }
        }

        // Keep stable insertion order for unseen windows.
        self.window_order.retain(|id| next_map.contains_key(id));
        for id in &present_ids {
            if !self.window_order.contains(id) {
                self.window_order.push(id.clone());
            }
        }

        self.windows = next_map;
        if self.switcher.is_some() {
            self.icons
                .ensure_all(self.windows.values().map(|w| w.app_id.as_str()));
        }
    }
}

impl cosmic::Application for SwitcherApp {
    type Message = Msg;
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    const APP_ID: &'static str = APP_ID;

    fn init(mut core: app::Core, _flags: ()) -> (Self, app::Task<Msg>) {
        // Transparent fullscreen overlay: never auto-blur the entire desktop.
        core.set_auto_blur(Default::default());
        (
            Self {
                core,
                outputs: Vec::new(),
                surface: None,
                active_output: None,
                wayland_tx: None,
                windows: HashMap::new(),
                window_order: Vec::new(),
                mru: Mru::new(),
                switcher: None,
                gen_id: 0,
                last_cycle: None,
                thumbs: HashMap::new(),
                icons: IconResolver::new(),
                hovered: None,
            },
            Task::none(),
        )
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
            Msg::Dbus(evt) => match evt {
                DbusEvent::ShowSwitcher { reverse } => {
                    return self.open_switcher(reverse, "", false);
                }
                DbusEvent::ShowSwitcherOn { reverse, output } => {
                    return self.open_switcher(reverse, &output, false);
                }
                DbusEvent::ShowOverviewOn { output } => {
                    return self.open_switcher(false, &output, true);
                }
                DbusEvent::Cycle { delta } => {
                    return self.cycle(delta, CycleSource::Dbus);
                }
                DbusEvent::Release => return self.confirm(),
                DbusEvent::Hide => return self.hide(),
                DbusEvent::Ready => service::sd_ready(),
                DbusEvent::Quit => std::process::exit(0),
            },
            Msg::Wayland(evt) => match evt {
                WaylandEvent::Ready(tx) => self.wayland_tx = Some(tx),
                WaylandEvent::Windows(list) => self.update_windows(list),
                WaylandEvent::Thumb(id, picture) => {
                    if self.switcher.is_some() {
                        match picture {
                            Some(pic) => {
                                self.thumbs.insert(id, pic);
                            }
                            None => {
                                self.thumbs.remove(&id);
                            }
                        }
                    }
                }
            },
            Msg::Output(name, handle, width, height) => {
                match self.outputs.iter_mut().find(|o| o.handle == handle) {
                    Some(existing) => {
                        existing.name = name;
                        if width > 0.0 {
                            existing.width = width;
                        }
                        if height > 0.0 {
                            existing.height = height;
                        }
                    }
                    None => self.outputs.push(Output {
                        name,
                        handle,
                        width: if width > 0.0 { width } else { 1920.0 },
                        height: if height > 0.0 { height } else { 1080.0 },
                    }),
                }
            }
            Msg::OutputGone(handle) => {
                self.outputs.retain(|o| o.handle != handle);
            }
            Msg::KeyAction(action, modifier_held) => {
                if self.switcher.is_none() {
                    return Task::none();
                }
                if modifier_held && let Some(state) = self.switcher.as_mut() {
                    state.note_modifier_held();
                }
                match action {
                    KeyAction::Cancel => return self.hide(),
                    KeyAction::Confirm => return self.confirm(),
                    KeyAction::Cycle(delta) => {
                        return self.cycle(delta, CycleSource::Keyboard);
                    }
                    KeyAction::Row(rows) => {
                        let cols = self.switcher_grid().cols as i32;
                        return self.cycle(rows * cols, CycleSource::Keyboard);
                    }
                }
            }
            Msg::ModifiersChanged { alt, logo } => {
                if (alt || logo) && let Some(state) = self.switcher.as_mut() {
                    state.note_modifier_held();
                }
                if let Some(state) = self.switcher.as_ref()
                    && !state.is_sticky()
                    && !alt
                    && !logo
                    && std::env::var_os("CS_NO_HELD_CHECK").is_none()
                {
                    return self.confirm();
                }
            }
            Msg::OnScroll(delta) => {
                let y = match delta {
                    ScrollDelta::Lines { y, .. } => y,
                    ScrollDelta::Pixels { y, .. } => y / 40.0,
                };
                if y > 0.0 {
                    return self.cycle(-1, CycleSource::Keyboard);
                } else if y < 0.0 {
                    return self.cycle(1, CycleSource::Keyboard);
                }
            }
            Msg::ShowPanel(gen_id) => {
                if gen_id == self.gen_id
                    && let Some(state) = self.switcher.as_mut()
                {
                    state.show_panel();
                }
            }
            Msg::HeldCheck(gen_id) => {
                if std::env::var_os("CS_NO_HELD_CHECK").is_some() {
                    return Task::none();
                }
                if gen_id == self.gen_id
                    && self
                        .switcher
                        .as_ref()
                        .is_some_and(SwitcherState::modifier_already_released)
                {
                    tracing::debug!("switcher: modifier never observed held, confirming switch");
                    return self.confirm();
                }
            }
            Msg::SafetyTimeout(gen_id) => {
                if gen_id == self.gen_id
                    && self
                        .switcher
                        .as_ref()
                        .is_some_and(|s| !s.is_sticky())
                    && std::env::var_os("CS_NO_HELD_CHECK").is_none()
                {
                    tracing::warn!("switcher: safety timeout elapsed, closing");
                    return self.hide();
                }
            }
            Msg::Activate(id) => {
                self.activate_window(id);
                return self.hide();
            }
            Msg::CloseWindow(id) => {
                if let Some(tx) = &self.wayland_tx {
                    let _ = tx.send(WaylandCmd::Close(id));
                }
            }
            Msg::Hover(id) => self.hovered = id,
            Msg::Cancel => return self.hide(),
            Msg::Beat => service::sd_beat(),
            Msg::Trim => {
                if self.surface.is_none() {
                    // SAFETY: hands freed thumbnail memory back to the OS.
                    unsafe { libc::malloc_trim(0) };
                }
            }
            Msg::Ignore => {}
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Msg> {
        let mut subs = vec![
            service::subscription().map(Msg::Dbus),
            wayland::subscription().map(Msg::Wayland),
            event::listen_with(|evt, _, _| {
                use iced::event::wayland::{Event as Wayland, OutputEvent};
                match evt {
                    iced::Event::PlatformSpecific(
                        iced::event::PlatformSpecific::Wayland(Wayland::Output(
                            OutputEvent::Created(Some(info)) | OutputEvent::InfoUpdate(info),
                            output,
                        )),
                    ) => {
                        let (w, h) = info
                            .logical_size
                            .map_or((0.0, 0.0), |(w, h)| (w as f32, h as f32));
                        info.name.map(|name| Msg::Output(name, output, w, h))
                    }
                    iced::Event::PlatformSpecific(
                        iced::event::PlatformSpecific::Wayland(Wayland::Output(
                            OutputEvent::Removed,
                            output,
                        )),
                    ) => Some(Msg::OutputGone(output)),
                    _ => None,
                }
            }),
        ];

        if let Some(period) = service::watchdog_period() {
            subs.push(iced::time::every(period).map(|_| Msg::Beat));
        }

        if self.surface.is_some() {
            subs.push(event::listen_with(|evt, _, _| match evt {
                iced::Event::Keyboard(keyboard::Event::KeyPressed {
                    key, modifiers, ..
                }) => key_action(&key, modifiers)
                    .map(|act| Msg::KeyAction(act, modifiers.alt() || modifiers.logo())),
                iced::Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                    Some(Msg::ModifiersChanged {
                        alt: modifiers.alt(),
                        logo: modifiers.logo(),
                    })
                }
                _ => None,
            }));
        }

        Subscription::batch(subs)
    }

    fn view(&self) -> Element<'_, Msg> {
        widget::text("").into()
    }

    fn view_window(&self, id: window::Id) -> Element<'_, Msg> {
        if self.surface != Some(id) {
            return widget::text("").into();
        }
        view::view(self)
    }

    fn on_close_requested(&self, id: window::Id) -> Option<Msg> {
        (self.surface == Some(id)).then_some(Msg::Cancel)
    }
}
