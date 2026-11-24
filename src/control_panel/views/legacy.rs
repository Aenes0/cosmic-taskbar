use cosmic::{
    Element,
    iced::{self, Alignment, Background, Color, Length},
    theme,
    widget::{self, button},
};
use crate::central::{
    monitor::config::{Panel as MonitorPanel, PollComponent as MonitorPollComponent},
    tiles::{chip_class, icon},
    widgets::card,
};
use crate::control_panel::Message;

pub(super) fn dimmed(theme: &cosmic::Theme) -> iced::widget::text::Style {
    let mut color = theme.cosmic().background(theme.transparent).on;
    color.alpha *= 0.65;
    iced::widget::text::Style {
        color: Some(color.into()),
        ..Default::default()
    }
}

pub(super) fn card_box<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    widget::container(content)
        .padding(18)
        .width(Length::Fill)
        .class(theme::Container::custom(|theme| {
            let c = card(theme.cosmic());
            let mut border_col: Color = theme.cosmic().background(theme.transparent).divider.into();
            border_col.a *= 0.55;
            widget::container::Style {
                background: Some(Background::Color(c.base.into())),
                text_color: Some(c.on.into()),
                icon_color: Some(c.on.into()),
                border: iced::Border {
                    radius: crate::shared::tokens::POPUP_RADIUS.into(),
                    width: crate::shared::tokens::HAIRLINE_WIDTH,
                    color: border_col,
                },
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.14),
                    offset: iced::Vector::new(0.0, 2.0),
                    blur_radius: 10.0,
                },
                ..Default::default()
            }
        }))
        .into()
}

pub(super) fn kv_row<'a>(label: &'a str, value: String) -> Element<'a, Message> {
    widget::row::with_children(vec![
        widget::text::body(label)
            .class(theme::Text::Custom(dimmed))
            .width(Length::Fill)
            .into(),
        widget::text::body(value).into(),
    ])
    .spacing(12)
    .align_y(Alignment::Center)
    .into()
}

pub(super) fn status_badge<'a>(active: bool, on_text: &'a str, off_text: &'a str) -> Element<'a, Message> {
    let label = if active { on_text } else { off_text };
    widget::container(widget::text::caption(label))
        .padding([3, 10])
        .class(theme::Container::custom(move |theme| {
            let bg = if active {
                Color::from_rgba(0.22, 0.72, 0.42, 0.22)
            } else {
                let mut d: Color = theme.cosmic().background(theme.transparent).divider.into();
                d.a = 0.35;
                d
            };
            let fg: Color = if active {
                Color::from_rgb(0.35, 0.88, 0.55)
            } else {
                theme.cosmic().background(theme.transparent).on.into()
            };
            widget::container::Style {
                background: Some(Background::Color(bg)),
                text_color: Some(fg),
                border: iced::Border {
                    radius: 12.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        }))
        .into()
}

pub(super) fn action_btn<'a>(label: &'a str, msg: Message, primary: bool) -> Element<'a, Message> {
    button::custom(widget::text::body(label))
        .padding([7, 14])
        .class(chip_class(primary))
        .on_press(msg)
        .into()
}

pub(super) fn small_chip<'a>(label: &'a str, active: bool, msg: Message) -> Element<'a, Message> {
    button::custom(widget::text::caption(label))
        .padding([5, 10])
        .class(chip_class(active))
        .on_press(msg)
        .into()
}

pub(super) fn toggle_row<'a>(label: &'a str, value: bool, msg: Message) -> Element<'a, Message> {
    widget::row::with_children(vec![
        widget::text::body(label).width(Length::Fill).into(),
        widget::toggler(value).on_toggle(move |_| msg.clone()).into(),
    ])
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}

#[derive(Clone, Copy)]
pub(super) enum TileTone {
    /// Always visible / Expanded (vibrant emerald/teal)
    Teal,
    /// Compact (cool sky blue)
    Blue,
    /// Active-only / Minimal (warm amber)
    Amber,
    /// Never / Hidden (dim neutral gray)
    Muted,
}

impl TileTone {
    fn colors(self) -> (Color, Color, Color) {
        // (card_bg, border, pill_fg)
        match self {
            TileTone::Teal => (
                Color::from_rgba(0.16, 0.72, 0.56, 0.16),
                Color::from_rgba(0.24, 0.82, 0.65, 0.48),
                Color::from_rgb(0.32, 0.90, 0.72),
            ),
            TileTone::Blue => (
                Color::from_rgba(0.24, 0.58, 0.96, 0.16),
                Color::from_rgba(0.34, 0.66, 0.98, 0.48),
                Color::from_rgb(0.46, 0.76, 1.0),
            ),
            TileTone::Amber => (
                Color::from_rgba(0.95, 0.70, 0.18, 0.15),
                Color::from_rgba(0.96, 0.74, 0.26, 0.46),
                Color::from_rgb(0.98, 0.80, 0.36),
            ),
            TileTone::Muted => (
                Color::from_rgba(0.50, 0.50, 0.55, 0.08),
                Color::from_rgba(0.50, 0.50, 0.55, 0.22),
                Color::from_rgba(0.75, 0.75, 0.80, 0.72),
            ),
        }
    }
}

pub(super) fn tone_pill<'a>(label: &'a str, tone: TileTone) -> Element<'a, Message> {
    let (bg, _, fg) = tone.colors();
    let pill_bg = Color { a: (bg.a * 1.45).min(0.35), ..bg };
    widget::container(widget::text::caption(label).size(11))
        .padding([2, 8])
        .class(theme::Container::custom(move |_| widget::container::Style {
            background: Some(Background::Color(pill_bg)),
            text_color: Some(fg),
            border: iced::Border {
                radius: 999.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }))
        .into()
}

pub(super) fn draggable_grid_tile<'a>(
    icon_name: &'static str,
    title: &'a str,
    status_label: &'a str,
    tone: TileTone,
    is_dragging_this: bool,
    any_dragging: bool,
    on_cycle: Message,
    on_drag_start: Message,
    on_drag_over: Message,
    footer: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let (bg, border_col, _) = tone.colors();
    let grip = widget::mouse_area(
        widget::container(widget::icon::icon(icon("view-app-grid-symbolic")).size(12))
            .padding([3, 4]),
    )
    .on_press(on_drag_start)
    .interaction(if is_dragging_this {
        iced::mouse::Interaction::Grabbing
    } else {
        iced::mouse::Interaction::Grab
    });

    let top_bar = widget::row::with_children(vec![
        grip.into(),
        widget::space::horizontal().width(Length::Fill).into(),
        tone_pill(status_label, tone),
    ])
    .align_y(Alignment::Center);

    let body_btn = button::custom(
        widget::column::with_children(vec![
            widget::icon::icon(icon(icon_name)).size(24).into(),
            widget::text::caption(title)
                .font(cosmic::font::semibold())
                .align_x(Alignment::Center)
                .into(),
        ])
        .spacing(6)
        .align_x(Alignment::Center)
        .width(Length::Fill),
    )
    .padding([6, 4])
    .width(Length::Fill)
    .class(theme::Button::Text)
    .on_press(on_cycle);

    let mut tile_rows: Vec<Element<'a, Message>> = vec![top_bar.into(), body_btn.into()];
    if let Some(foot) = footer {
        tile_rows.push(foot);
    }
    let tile_content = widget::column::with_children(tile_rows)
        .spacing(6)
        .width(Length::Fill);

    let card_el = widget::container(tile_content)
        .padding([8, 10])
        .width(Length::Fill)
        .class(theme::Container::custom(move |theme| {
            let mut text_col: Color = theme.cosmic().background(theme.transparent).on.into();
            if matches!(tone, TileTone::Muted) {
                text_col.a *= 0.62;
            }
            let (b_width, b_color) = if is_dragging_this {
                (2.0, theme.cosmic().accent_color().into())
            } else {
                (1.0, border_col)
            };
            widget::container::Style {
                background: Some(Background::Color(bg)),
                text_color: Some(text_col),
                icon_color: Some(text_col),
                border: iced::Border {
                    radius: crate::shared::tokens::POPUP_RADIUS.into(),
                    width: b_width,
                    color: b_color,
                },
                ..Default::default()
            }
        }));

    let mut area = widget::mouse_area(card_el).on_enter(on_drag_over);
    if any_dragging {
        area = area.interaction(iced::mouse::Interaction::Grabbing);
    }
    area.into()
}

pub(super) fn tile_grid<'a>(tiles: Vec<Element<'a, Message>>, cols: usize) -> Element<'a, Message> {
    let mut rows: Vec<Element<'a, Message>> = Vec::new();
    let mut iter = tiles.into_iter();
    loop {
        let mut line: Vec<Element<'a, Message>> = Vec::with_capacity(cols);
        for _ in 0..cols {
            if let Some(el) = iter.next() {
                line.push(el);
            }
        }
        if line.is_empty() {
            break;
        }
        while line.len() < cols {
            line.push(widget::space::horizontal().width(Length::Fill).into());
        }
        rows.push(
            widget::row::with_children(line)
                .spacing(10)
                .width(Length::Fill)
                .into(),
        );
    }
    widget::column::with_children(rows)
        .spacing(10)
        .width(Length::Fill)
        .into()
}

pub(super) fn monitor_panel_icon(panel: MonitorPanel) -> &'static str {
    match panel {
        MonitorPanel::Energy => "battery-good-symbolic",
        MonitorPanel::Cpu => "computer-symbolic",
        MonitorPanel::Gpu => "video-display-symbolic",
        MonitorPanel::Memory => "media-memory-symbolic",
        MonitorPanel::Disks => "drive-harddisk-symbolic",
        MonitorPanel::Network => "network-wired-symbolic",
        MonitorPanel::Sensors => "weather-clear-symbolic",
        MonitorPanel::Processes => "utilities-system-monitor-symbolic",
        MonitorPanel::System => "emblem-system-symbolic",
    }
}

pub(super) fn monitor_poll_for(panel: MonitorPanel) -> Option<MonitorPollComponent> {
    match panel {
        MonitorPanel::Cpu => Some(MonitorPollComponent::Cpu),
        MonitorPanel::Gpu => Some(MonitorPollComponent::Gpu),
        MonitorPanel::Memory => Some(MonitorPollComponent::Ram),
        MonitorPanel::Disks => Some(MonitorPollComponent::Disk),
        MonitorPanel::Network => Some(MonitorPollComponent::Net),
        MonitorPanel::Processes => Some(MonitorPollComponent::Procs),
        MonitorPanel::Energy | MonitorPanel::Sensors | MonitorPanel::System => None,
    }
}
