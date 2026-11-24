// SPDX-License-Identifier: GPL-3.0-only

//! Layer-shell host for the taskbar's window previews and right-click context
//! menus (`cosmic-taskbar --taskbar-popups`). Connects directly to cosmic-comp
//! on Vulkan GPU — zero cosmic-panel xdg_popup proxy, zero grab conflicts.

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use cosmic::{
    Element, Task, app,
    cctk::sctk::reexports::{
        calloop,
        client::protocol::wl_output::WlOutput,
    },
    cosmic_config::{Config, CosmicConfigEntry},
    desktop::{IconSourceExt, fde},
    iced::{
        self, Alignment, Background, Border, Length, Limits, Padding, Subscription,
        advanced::text::{Ellipsize, EllipsizeHeightLimit},
        event,
        futures::{self, SinkExt, channel::mpsc},
        keyboard::{self, Key, key::Named},
        platform_specific::{
            runtime::wayland::layer_surface::{IcedMargin, IcedOutput, SctkLayerSurfaceSettings},
            shell::commands::layer_surface::{
                Anchor, KeyboardInteractivity, Layer, destroy_layer_surface, get_layer_surface,
                set_margin,
            },
        },
        stream,
        widget::{column, mouse_area, row, space::horizontal as horizontal_space},
        window,
    },
    theme::{self, Button, Container},
    widget::{button, container, divider, icon, menu, text},
};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixListener;

use crate::bar::config::{APP_ID as LIST_ID, AppListConfig};
use crate::bar::texts::texts;
use crate::shared::anim::Spring;
use crate::shared::preview_card::{self, CardParams, Picture};

use super::proto::{
    AnchorEdge, MENU_WIDTH, MenuAction, PREVIEW_MAX_H, PREVIEW_MAX_W, PREVIEW_PADDING,
    PREVIEW_SPACING, PopupCommand, clamp_margin, estimate_preview_major, socket_path,
};
use super::wayland::{self, PREVIEW_OPEN, WaylandCmd, WaylandEvent, WinInfo};

pub const APP_ID: &str = "io.github.Aenes0.CosmicTaskbar.Popups";

const PANEL_GAP: i32 = 8;
const SCREEN_PAD: f32 = 8.0;
const HIDE_PREVIEW_GRACE: Duration = Duration::from_millis(240);
const MENU_SETTLING: Duration = crate::shared::tokens::FOCUS_SETTLING;
const ANIM_FRAME: Duration = Duration::from_millis(16);
const PREVIEW_REFRESH: Duration = Duration::from_millis(1000);

#[derive(Debug, Clone)]
pub enum HostMsg {
    Command(PopupCommand),
    Wayland(WaylandEvent),
    Output(String, WlOutput, f32, f32),
    OutputGone(WlOutput),
    PreviewHovered(bool),
    CardHovered(Option<String>),
    HidePreviewTick(u64),
    RefreshPreview,
    AnimTick,
    ActivateWindow(String),
    CloseWindow(String),
    CloseAllWindows(Vec<String>),
    Exec {
        exec: String,
        app_id: String,
        terminal: bool,
    },
    TogglePin {
        original_app_id: String,
        currently_pinned: bool,
    },
    Unfocused(window::Id),
    Close,
}

struct Output {
    name: String,
    output: WlOutput,
    width: f32,
    height: f32,
}

#[derive(Debug, Clone)]
struct PreviewData {
    app_id: String,
    icon_name: String,
    window_ids: Vec<String>,
}

#[derive(Debug, Clone)]
struct MenuData {
    app_id: String,
    original_app_id: String,
    app_name: String,
    icon_name: String,
    exec: Option<String>,
    terminal: bool,
    is_pinned: bool,
    actions: Vec<MenuAction>,
    window_ids: Vec<String>,
}

#[derive(Debug, Clone)]
enum ActiveKind {
    Preview(PreviewData),
    Menu(MenuData),
}

struct ActivePopup {
    id: window::Id,
    opened_at: Instant,
    output: String,
    anchor: AnchorEdge,
    kind: ActiveKind,
}

pub struct TaskbarPopups {
    core: app::Core,
    outputs: Vec<Output>,
    active: Option<ActivePopup>,
    pending: Option<PopupCommand>,
    windows: HashMap<String, WinInfo>,
    thumbs: HashMap<String, Picture>,
    wayland_tx: Option<calloop::channel::Sender<WaylandCmd>>,
    preview_hovered: bool,
    hovered_card: Option<String>,
    hide_gen: u64,
    fade: Spring,
    fade_last: Option<Instant>,
    lifts: HashMap<String, Spring>,
}

pub fn run() -> cosmic::iced::Result {
    let settings = cosmic::app::Settings::default()
        .no_main_window(true)
        .exit_on_close(false)
        .transparent(true);
    cosmic::app::run::<TaskbarPopups>(settings, ())
}

fn socket_subscription() -> Subscription<HostMsg> {
    struct SocketSub;
    Subscription::run_with(std::any::TypeId::of::<SocketSub>(), |_| {
        stream::channel(32, |output: mpsc::Sender<HostMsg>| async move {
            let path = socket_path();
            let _ = std::fs::remove_file(&path);
            let listener = match UnixListener::bind(&path) {
                Ok(l) => l,
                Err(err) => {
                    tracing::error!("cannot bind {}: {err}", path.display());
                    futures::future::pending::<()>().await;
                    return;
                }
            };
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let mut tx = output.clone();
                        tokio::spawn(async move {
                            let mut reader = BufReader::new(stream).lines();
                            while let Ok(Some(line)) = reader.next_line().await {
                                if let Ok(cmd) = serde_json::from_str::<PopupCommand>(&line) {
                                    let _ = tx.send(HostMsg::Command(cmd)).await;
                                }
                            }
                        });
                    }
                    Err(err) => {
                        tracing::warn!("socket accept error: {err}");
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                }
            }
        })
    })
}

impl TaskbarPopups {
    fn output_info(&self, name: &str) -> Option<&Output> {
        self.outputs.iter().find(|o| o.name == name)
    }

    fn is_menu_open(&self) -> bool {
        matches!(self.active, Some(ActivePopup { kind: ActiveKind::Menu(_), .. }))
    }

    fn is_preview_open(&self) -> bool {
        matches!(self.active, Some(ActivePopup { kind: ActiveKind::Preview(_), .. }))
    }

    fn close_active(&mut self) -> app::Task<HostMsg> {
        self.preview_hovered = false;
        self.hovered_card = None;
        self.lifts.clear();
        PREVIEW_OPEN.store(false, Ordering::Relaxed);
        if let Some(active) = self.active.take() {
            destroy_layer_surface(active.id)
        } else {
            Task::none()
        }
    }

    fn request_captures(&self, ids: &[String]) {
        if let Some(tx) = &self.wayland_tx {
            let _ = tx.send(WaylandCmd::Capture(ids.to_vec()));
        }
    }

    fn compute_placement(
        &self,
        output_name: &str,
        anchor: AnchorEdge,
        icon_center: f32,
        popup_major: f32,
    ) -> (IcedOutput, Anchor, IcedMargin) {
        let info = self.output_info(output_name);
        let output = info.map_or(IcedOutput::Active, |o| IcedOutput::Output(o.output.clone()));
        let screen_major = info.map_or(1920.0, |o| {
            if anchor.is_horizontal() { o.width } else { o.height }
        });
        let lead = clamp_margin(icon_center, popup_major, screen_major, SCREEN_PAD);
        let (wl_anchor, margin) = match anchor {
            AnchorEdge::Bottom => (
                Anchor::BOTTOM | Anchor::LEFT,
                IcedMargin { bottom: PANEL_GAP, left: lead, top: 0, right: 0 },
            ),
            AnchorEdge::Top => (
                Anchor::TOP | Anchor::LEFT,
                IcedMargin { top: PANEL_GAP, left: lead, bottom: 0, right: 0 },
            ),
            AnchorEdge::Left => (
                Anchor::LEFT | Anchor::TOP,
                IcedMargin { left: PANEL_GAP, top: lead, right: 0, bottom: 0 },
            ),
            AnchorEdge::Right => (
                Anchor::RIGHT | Anchor::TOP,
                IcedMargin { right: PANEL_GAP, top: lead, left: 0, bottom: 0 },
            ),
        };
        (output, wl_anchor, margin)
    }

    fn build_layer_surface(
        &self,
        id: window::Id,
        output_name: &str,
        anchor: AnchorEdge,
        icon_center: f32,
        popup_major: f32,
        keyboard: KeyboardInteractivity,
    ) -> app::Task<HostMsg> {
        let (output, wl_anchor, margin) =
            self.compute_placement(output_name, anchor, icon_center, popup_major);
        get_layer_surface(SctkLayerSurfaceSettings {
            id,
            layer: Layer::Top,
            keyboard_interactivity: keyboard,
            anchor: wl_anchor,
            output,
            namespace: "cosmic-taskbar-popups".into(),
            margin,
            size: None,
            exclusive_zone: 0,
            size_limits: Limits::NONE.min_width(1.0).min_height(1.0),
            ..Default::default()
        })
    }

    fn handle_command(&mut self, cmd: PopupCommand) -> app::Task<HostMsg> {
        if self.outputs.is_empty() && !matches!(cmd, PopupCommand::HideAll | PopupCommand::HidePreview { .. }) {
            self.pending = Some(cmd);
            return Task::none();
        }
        match cmd {
            PopupCommand::ShowPreview {
                output,
                anchor,
                icon_center,
                app_id,
                icon_name,
                window_ids,
            } => {
                // Right-click menu always wins over hover previews.
                if self.is_menu_open() || window_ids.is_empty() {
                    return Task::none();
                }
                self.hide_gen += 1;
                let est = estimate_preview_major(window_ids.len(), anchor.is_horizontal());
                // Reuse the existing preview layer surface when gliding between apps
                // on the same output/edge: zero flicker, zero Vulkan swapchain recreate.
                if let Some(ActivePopup {
                    id: existing_id,
                    output: cur_out,
                    anchor: cur_anchor,
                    kind: ActiveKind::Preview(cur),
                    ..
                }) = &mut self.active
                {
                    if *cur_out == output && *cur_anchor == anchor {
                        let same_app = cur.app_id == app_id;
                        let surface_id = *existing_id;
                        cur.app_id = app_id;
                        cur.icon_name = icon_name;
                        cur.window_ids = window_ids.clone();
                        if !same_app {
                            self.hovered_card = None;
                            self.lifts.clear();
                        }
                        self.request_captures(&window_ids);
                        let (_, _, margin) =
                            self.compute_placement(&output, anchor, icon_center, est);
                        return set_margin(
                            surface_id,
                            margin.top,
                            margin.right,
                            margin.bottom,
                            margin.left,
                        );
                    }
                }
                let close = self.close_active();
                let new_id = window::Id::unique();
                let surface = self.build_layer_surface(
                    new_id,
                    &output,
                    anchor,
                    icon_center,
                    est,
                    KeyboardInteractivity::None,
                );
                self.fade = Spring::new(if crate::shared::anim::motion_scale() > 0.0 {
                    0.0
                } else {
                    1.0
                });
                self.fade_last = None;
                PREVIEW_OPEN.store(true, Ordering::Relaxed);
                self.request_captures(&window_ids);
                self.active = Some(ActivePopup {
                    id: new_id,
                    opened_at: Instant::now(),
                    output,
                    anchor,
                    kind: ActiveKind::Preview(PreviewData {
                        app_id,
                        icon_name,
                        window_ids,
                    }),
                });
                Task::batch([close, surface])
            }
            PopupCommand::HidePreview { output } => {
                let matches_preview = matches!(
                    &self.active,
                    Some(ActivePopup {
                        output: cur_out,
                        kind: ActiveKind::Preview(_),
                        ..
                    }) if *cur_out == output
                );
                if !matches_preview || self.preview_hovered {
                    return Task::none();
                }
                self.hide_gen += 1;
                let gen_id = self.hide_gen;
                Task::perform(tokio::time::sleep(HIDE_PREVIEW_GRACE), move |()| {
                    cosmic::Action::App(HostMsg::HidePreviewTick(gen_id))
                })
            }
            PopupCommand::ShowMenu {
                output,
                anchor,
                icon_center,
                app_id,
                original_app_id,
                app_name,
                icon_name,
                exec,
                terminal,
                is_pinned,
                actions,
                window_ids,
            } => {
                self.hide_gen += 1;
                // Clicking right button on the same app while its menu is open toggles it closed.
                if let Some(ActivePopup {
                    output: cur_out,
                    kind: ActiveKind::Menu(cur),
                    ..
                }) = &self.active
                {
                    if *cur_out == output && cur.app_id == app_id {
                        return self.close_active();
                    }
                }
                let close = self.close_active();
                let new_id = window::Id::unique();
                let surface = self.build_layer_surface(
                    new_id,
                    &output,
                    anchor,
                    icon_center,
                    MENU_WIDTH,
                    KeyboardInteractivity::OnDemand,
                );
                self.active = Some(ActivePopup {
                    id: new_id,
                    opened_at: Instant::now(),
                    output,
                    anchor,
                    kind: ActiveKind::Menu(MenuData {
                        app_id,
                        original_app_id,
                        app_name,
                        icon_name,
                        exec,
                        terminal,
                        is_pinned,
                        actions,
                        window_ids,
                    }),
                });
                let dismiss = Task::future(async {
                    crate::shared::overlay::dismiss_others(crate::shared::overlay::Overlay::Taskbar).await;
                    cosmic::Action::None
                });
                Task::batch([close, surface, dismiss])
            }
            PopupCommand::HideAll => {
                self.hide_gen += 1;
                self.close_active()
            }
        }
    }

    fn preview_animating(&self) -> bool {
        if !self.is_preview_open() {
            return false;
        }
        !self.fade.settled_loose(1.0)
            || self.lifts.iter().any(|(id, s)| {
                let target = if self.hovered_card.as_deref() == Some(id.as_str()) {
                    1.0
                } else {
                    0.0
                };
                !s.settled_loose(target)
            })
    }

    fn view_preview<'a>(&'a self, data: &'a PreviewData, horizontal: bool) -> Element<'a, HostMsg> {
        let app_icon = fde::IconSource::from_unknown(data.icon_name.as_str()).as_cosmic_icon();
        let fade = self.fade.pos.clamp(0.0, 1.0);
        let cards: Vec<Element<'a, HostMsg>> = data
            .window_ids
            .iter()
            .map(|win_id| {
                let info = self.windows.get(win_id);
                let title = info.map(|w| w.title.clone()).unwrap_or_default();
                let picture = self.thumbs.get(win_id).cloned();
                let hovered = self.hovered_card.as_deref() == Some(win_id.as_str());
                let lift = self.lifts.get(win_id).map_or(0.0, |s| s.pos);
                preview_card::card(CardParams {
                    picture,
                    app_icon: app_icon.clone(),
                    title,
                    hovered,
                    fade,
                    lift,
                    max_w: PREVIEW_MAX_W,
                    max_h: PREVIEW_MAX_H,
                    on_press: HostMsg::ActivateWindow(win_id.clone()),
                    on_close: HostMsg::CloseWindow(win_id.clone()),
                    on_middle: HostMsg::CloseWindow(win_id.clone()),
                    on_enter: HostMsg::CardHovered(Some(win_id.clone())),
                    on_exit: HostMsg::CardHovered(None),
                })
            })
            .collect();

        let body: Element<'a, HostMsg> = if horizontal {
            row(cards)
                .spacing(PREVIEW_SPACING)
                .align_y(Alignment::Center)
                .into()
        } else {
            column(cards)
                .spacing(PREVIEW_SPACING)
                .align_x(Alignment::Center)
                .into()
        };

        mouse_area(container(body).padding(PREVIEW_PADDING))
            .on_enter(HostMsg::PreviewHovered(true))
            .on_exit(HostMsg::PreviewHovered(false))
            .into()
    }

    fn view_menu<'a>(&'a self, data: &'a MenuData) -> Element<'a, HostMsg> {
        fn menu_button<'a>(content: impl Into<Element<'a, HostMsg>>) -> cosmic::widget::Button<'a, HostMsg> {
            let space_xxs = theme::spacing().space_xxs;
            let space_m = theme::spacing().space_m;
            button::custom(content)
                .height(20 + 2 * space_xxs)
                .class(Button::MenuItem)
                .padding(Padding::from([space_xxs as f32, space_m as f32]))
                .width(Length::Fill)
        }
        fn with_icon<'a>(
            ico: Option<icon::Handle>,
            label: String,
        ) -> Element<'a, HostMsg> {
            let label = text::body(label)
                .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)));
            match ico {
                Some(ico) => row![icon::icon(ico).size(16), label]
                    .spacing(8)
                    .align_y(Alignment::Center)
                    .into(),
                None => row![horizontal_space().width(Length::Fixed(16.0)), label]
                    .spacing(8)
                    .align_y(Alignment::Center)
                    .into(),
            }
        }

        let t = texts();
        let app_icon = fde::IconSource::from_unknown(data.icon_name.as_str()).as_cosmic_icon();
        let mut content = menu::menu_column::MenuColumn::with_capacity(8).align_x(Alignment::Center);

        if !data.actions.is_empty() {
            for action in &data.actions {
                let action_icon = action
                    .icon
                    .as_deref()
                    .map(|i| fde::IconSource::from_unknown(i).as_cosmic_icon());
                content = content.push(
                    menu_button(with_icon(action_icon, action.name.clone())).on_press(
                        HostMsg::Exec {
                            exec: action.exec.clone(),
                            app_id: data.app_id.clone(),
                            terminal: data.terminal,
                        },
                    ),
                );
            }
            content = content.push(divider::horizontal::light());
        }

        if let Some(exec) = &data.exec {
            content = content.push(
                menu_button(with_icon(Some(app_icon), data.app_name.clone())).on_press(
                    HostMsg::Exec {
                        exec: exec.clone(),
                        app_id: data.app_id.clone(),
                        terminal: data.terminal,
                    },
                ),
            );
        }

        content = content.push(
            menu_button(with_icon(
                Some(icon::from_name("view-pin-symbolic").into()),
                if data.is_pinned { t.unpin } else { t.pin }.to_string(),
            ))
            .on_press(HostMsg::TogglePin {
                original_app_id: data.original_app_id.clone(),
                currently_pinned: data.is_pinned,
            }),
        );

        if !data.window_ids.is_empty() {
            let win_ids = data.window_ids.clone();
            let label = if win_ids.len() == 1 {
                t.close_window
            } else {
                t.close_all
            };
            content = content.push(
                menu_button(with_icon(
                    Some(icon::from_name("window-close-symbolic").into()),
                    label.to_string(),
                ))
                .on_press(HostMsg::CloseAllWindows(win_ids)),
            );
        }

        container(content)
            .width(Length::Fixed(MENU_WIDTH))
            .padding(6)
            .class(Container::custom(|theme: &cosmic::Theme| {
                let c = theme.cosmic();
                let bg = c.background(theme.transparent);
                container::Style {
                    background: Some(Background::Color(bg.base.into())),
                    text_color: Some(bg.on.into()),
                    border: Border {
                        radius: c.corner_radii.radius_m.into(),
                        width: 1.0,
                        color: bg.divider.into(),
                    },
                    shadow: crate::shared::tokens::menu_shadow(),
                    ..Default::default()
                }
            }))
            .into()
    }
}

impl cosmic::Application for TaskbarPopups {
    type Message = HostMsg;
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    const APP_ID: &'static str = APP_ID;

    fn init(mut core: app::Core, _flags: ()) -> (Self, app::Task<HostMsg>) {
        core.set_app_type(cosmic::core::AppType::Applet);
        core.set_auto_blur(cosmic::core::Auto::Popup.into());
        (
            Self {
                core,
                outputs: Vec::new(),
                active: None,
                pending: None,
                windows: HashMap::new(),
                thumbs: HashMap::new(),
                wayland_tx: None,
                preview_hovered: false,
                hovered_card: None,
                hide_gen: 0,
                fade: Spring::new(1.0),
                fade_last: None,
                lifts: HashMap::new(),
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

    fn update(&mut self, message: HostMsg) -> app::Task<HostMsg> {
        match message {
            HostMsg::Command(cmd) => return self.handle_command(cmd),
            HostMsg::Wayland(ev) => match ev {
                WaylandEvent::Ready(tx) => self.wayland_tx = Some(tx),
                WaylandEvent::Windows(map) => {
                    self.thumbs.retain(|id, _| map.contains_key(id));
                    self.windows = map;
                    // If all windows of the previewed app closed, hide the preview.
                    let should_close = match &mut self.active {
                        Some(ActivePopup {
                            kind: ActiveKind::Preview(data),
                            ..
                        }) => {
                            data.window_ids.retain(|id| self.windows.contains_key(id));
                            data.window_ids.is_empty()
                        }
                        _ => false,
                    };
                    if should_close {
                        return self.close_active();
                    }
                }
                WaylandEvent::Thumb(id, pic) => {
                    if let Some(pic) = pic {
                        self.thumbs.insert(id, pic);
                    }
                }
            },
            HostMsg::Output(name, output, width, height) => {
                match self.outputs.iter_mut().find(|o| o.output == output) {
                    Some(known) => {
                        known.name = name;
                        if width > 0.0 {
                            known.width = width;
                        }
                        if height > 0.0 {
                            known.height = height;
                        }
                    }
                    None => self.outputs.push(Output {
                        name,
                        output,
                        width,
                        height,
                    }),
                }
                if let Some(cmd) = self.pending.take() {
                    return self.handle_command(cmd);
                }
            }
            HostMsg::OutputGone(output) => {
                self.outputs.retain(|o| o.output != output);
            }
            HostMsg::PreviewHovered(hovered) => {
                self.preview_hovered = hovered;
                self.hide_gen += 1;
                if !hovered && self.is_preview_open() {
                    self.hovered_card = None;
                    let gen_id = self.hide_gen;
                    return Task::perform(tokio::time::sleep(HIDE_PREVIEW_GRACE), move |()| {
                        cosmic::Action::App(HostMsg::HidePreviewTick(gen_id))
                    });
                }
            }
            HostMsg::CardHovered(id) => {
                self.hovered_card = id.clone();
                if let Some(id) = id {
                    self.lifts.entry(id).or_insert_with(|| Spring::new(0.0));
                }
            }
            HostMsg::HidePreviewTick(gen_id) => {
                if gen_id == self.hide_gen && !self.preview_hovered && self.is_preview_open() {
                    return self.close_active();
                }
            }
            HostMsg::RefreshPreview => {
                if let Some(ActivePopup {
                    kind: ActiveKind::Preview(data),
                    ..
                }) = &self.active
                {
                    self.request_captures(&data.window_ids);
                }
            }
            HostMsg::AnimTick => {
                let now = Instant::now();
                let dt = self.fade_last.map_or(0.0, |t| (now - t).as_secs_f32());
                self.fade_last = Some(now);
                self.fade.step(1.0, dt, 320.0, 28.0);
                if self.fade.settled_loose(1.0) {
                    self.fade = Spring::new(1.0);
                }
                for (id, spring) in &mut self.lifts {
                    let target = if self.hovered_card.as_deref() == Some(id.as_str()) {
                        1.0
                    } else {
                        0.0
                    };
                    spring.step(target, dt, 380.0, 24.0);
                    if spring.settled_loose(target) {
                        *spring = Spring::new(target);
                    }
                }
            }
            HostMsg::ActivateWindow(id) => {
                if let Some(tx) = &self.wayland_tx {
                    let _ = tx.send(WaylandCmd::ToggleWindow(id));
                }
                return self.close_active();
            }
            HostMsg::CloseWindow(id) => {
                if let Some(tx) = &self.wayland_tx {
                    let _ = tx.send(WaylandCmd::Close(id));
                }
            }
            HostMsg::CloseAllWindows(ids) => {
                if let Some(tx) = &self.wayland_tx {
                    let _ = tx.send(WaylandCmd::CloseMany(ids));
                }
                return self.close_active();
            }
            HostMsg::Exec {
                exec,
                app_id,
                terminal,
            } => {
                tokio::spawn(async move {
                    cosmic::desktop::spawn_desktop_exec(
                        exec,
                        Vec::<(String, String)>::new(),
                        Some(app_id.as_str()),
                        terminal,
                    )
                    .await;
                });
                return self.close_active();
            }
            HostMsg::TogglePin {
                original_app_id,
                currently_pinned,
            } => {
                if let Ok(cfg_handle) = Config::new(LIST_ID, AppListConfig::VERSION) {
                    let mut cfg = AppListConfig::get_entry(&cfg_handle).unwrap_or_default();
                    if currently_pinned {
                        cfg.remove_pinned(&original_app_id, &cfg_handle);
                    } else {
                        cfg.add_pinned(original_app_id, &cfg_handle);
                    }
                }
                return self.close_active();
            }
            HostMsg::Unfocused(id) => {
                if let Some(active) = &self.active {
                    if active.id == id
                        && matches!(active.kind, ActiveKind::Menu(_))
                        && active.opened_at.elapsed() > MENU_SETTLING
                    {
                        return self.close_active();
                    }
                }
            }
            HostMsg::Close => return self.close_active(),
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<HostMsg> {
        let mut subs = vec![
            socket_subscription(),
            wayland::subscription().map(HostMsg::Wayland),
            event::listen_with(|event, _, _| {
                use iced::event::wayland::{Event as Wayland, OutputEvent};
                match event {
                    iced::Event::PlatformSpecific(iced::event::PlatformSpecific::Wayland(
                        Wayland::Output(
                            OutputEvent::Created(Some(info)) | OutputEvent::InfoUpdate(info),
                            output,
                        ),
                    )) => {
                        let (w, h) = info
                            .logical_size
                            .map_or((1920.0, 1080.0), |(w, h)| (w as f32, h as f32));
                        info.name.map(|name| HostMsg::Output(name, output, w, h))
                    }
                    iced::Event::PlatformSpecific(iced::event::PlatformSpecific::Wayland(
                        Wayland::Output(OutputEvent::Removed, output),
                    )) => Some(HostMsg::OutputGone(output)),
                    _ => None,
                }
            }),
        ];

        if self.active.is_some() {
            subs.push(event::listen_with(|event, _, window| {
                use iced::event::wayland::{Event as Wayland, LayerEvent};
                match event {
                    iced::Event::Keyboard(keyboard::Event::KeyPressed {
                        key: Key::Named(Named::Escape),
                        ..
                    }) => Some(HostMsg::Close),
                    iced::Event::PlatformSpecific(iced::event::PlatformSpecific::Wayland(
                        Wayland::Layer(LayerEvent::Unfocused, ..),
                    )) => Some(HostMsg::Unfocused(window)),
                    _ => None,
                }
            }));
        }

        if self.is_preview_open() {
            subs.push(iced::time::every(PREVIEW_REFRESH).map(|_| HostMsg::RefreshPreview));
        }
        if self.preview_animating() {
            subs.push(iced::time::every(ANIM_FRAME).map(|_| HostMsg::AnimTick));
        }

        Subscription::batch(subs)
    }

    fn view(&self) -> Element<'_, HostMsg> {
        text::body("").into()
    }

    fn view_window(&self, id: window::Id) -> Element<'_, HostMsg> {
        let Some(active) = self.active.as_ref().filter(|a| a.id == id) else {
            return text::body("").into();
        };
        let content = match &active.kind {
            ActiveKind::Preview(data) => self.view_preview(data, active.anchor.is_horizontal()),
            ActiveKind::Menu(data) => self.view_menu(data),
        };
        cosmic::widget::autosize::autosize(
            content,
            cosmic::widget::Id::new("taskbar-popups-autosize"),
        )
        .into()
    }
}
