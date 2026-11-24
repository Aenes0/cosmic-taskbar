// SPDX-License-Identifier: GPL-3.0-only

//! GPU-rendered layer-shell view for the isolated Alt+Tab / Super+Tab switcher.

use std::sync::LazyLock;

use cosmic::{
    Apply, Element,
    iced::{
        self, Alignment, Background, Border, Color, ContentFit, Length, Padding, Shadow, Vector,
        advanced::text::{Ellipsize, EllipsizeHeightLimit},
        widget::{column, mouse_area, row, space::horizontal as horizontal_space},
    },
    theme::{self, Button, Container},
    widget::{self, Image, button, container, icon, text},
};

use super::state::{GRID_GAP, GridSpec, LABEL_H, PANEL_PADDING};
use super::{Msg, SwitcherApp};

/// Scrollable ID used to keep the keyboard-selected row in view.
pub static SWITCHER_SCROLL_ID: LazyLock<widget::Id> =
    LazyLock::new(|| widget::Id::new("cosmic-switcher-grid"));

const CARD_PAD: f32 = 8.0;
const CARD_RADIUS: f32 = crate::shared::tokens::POPUP_RADIUS;
const THUMB_RADIUS: f32 = 8.0;
const PANEL_RADIUS: f32 = 18.0;

/// Renders the switcher surface for the active output.
pub fn view<'a>(app: &'a SwitcherApp) -> Element<'a, Msg> {
    let Some(state) = app.switcher.as_ref().filter(|s| s.panel_visible()) else {
        // Transparent placeholder during PANEL_DELAY_MS: grabs keyboard with zero flash.
        return container(horizontal_space())
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
    };

    let spec = app.switcher_grid();
    let total = state.order().len();

    let title = if state.is_sticky() {
        "Visão geral das janelas"
    } else {
        "Alternar janelas"
    };

    let count_badge = container(text(format!("{total}")).size(12.0))
        .padding([2.0, 8.0])
        .class(Container::custom(|theme: &cosmic::Theme| {
            let c = theme.cosmic();
            let mut bg: Color = c.on_bg_color().into();
            bg.a = 0.12;
            container::Style {
                background: Some(Background::Color(bg)),
                text_color: Some(c.on_bg_color().into()),
                border: Border {
                    radius: 999.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        }));

    let mut dim_color: Color = theme::active().cosmic().on_bg_color().into();
    dim_color.a = 0.6;
    let hint = text("Tab navegar · Enter abrir · Esc fechar")
        .size(12.0)
        .class(theme::Text::Color(dim_color));

    let header = row![
        text(title).size(14.0),
        count_badge,
        horizontal_space().width(Length::Fill),
        hint,
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let body: Element<'a, Msg> = if total == 0 {
        container(text("Nenhuma janela aberta").size(15.0))
            .padding([36, 48])
            .center_x(Length::Fill)
            .into()
    } else {
        let mut rows_vec: Vec<Element<'a, Msg>> = Vec::new();
        let mut current_row: Vec<Element<'a, Msg>> = Vec::new();

        for (idx, id) in state.order().iter().enumerate() {
            let Some(win) = app.windows.get(id) else {
                continue;
            };
            let selected = idx == state.selected_index();
            let hovered = app.hovered.as_deref() == Some(id.as_str());
            current_row.push(window_card(app, win, selected, hovered, &spec));
            if current_row.len() == spec.cols {
                rows_vec.push(
                    row(std::mem::take(&mut current_row))
                        .spacing(GRID_GAP)
                        .align_y(Alignment::Center)
                        .into(),
                );
            }
        }
        if !current_row.is_empty() {
            rows_vec.push(
                row(current_row)
                    .spacing(GRID_GAP)
                    .align_y(Alignment::Center)
                    .into(),
            );
        }

        let grid = column(rows_vec).spacing(GRID_GAP).align_x(Alignment::Start);
        if spec.scrolls {
            let (_, avail_h) = app.switcher_avail();
            widget::scrollable(grid)
                .id(SWITCHER_SCROLL_ID.clone())
                .height(Length::Fixed(avail_h))
                .into()
        } else {
            grid.into()
        }
    };

    let card_w = spec.preview_w + 2.0 * CARD_PAD;
    let shown_cols = total.clamp(1, spec.cols) as f32;
    let panel_inner_w = if total == 0 {
        360.0
    } else {
        shown_cols * card_w + (shown_cols - 1.0) * GRID_GAP
    };

    let panel = container(
        column![header, body]
            .spacing(14)
            .width(Length::Fixed(panel_inner_w)),
    )
    .padding(PANEL_PADDING)
    .class(Container::custom(|theme: &cosmic::Theme| {
        let c = theme.cosmic();
        let mut bg: Color = c.background(false).base.into();
        bg.a = if c.is_dark { 0.92 } else { 0.95 };
        let mut border_col: Color = c.on_bg_color().into();
        border_col.a = if c.is_dark { 0.14 } else { 0.12 };
        container::Style {
            background: Some(Background::Color(bg)),
            text_color: Some(c.on_bg_color().into()),
            border: Border {
                radius: PANEL_RADIUS.into(),
                width: crate::shared::tokens::HAIRLINE_WIDTH,
                color: border_col,
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.45),
                offset: Vector::new(0.0, 12.0),
                blur_radius: 32.0,
            },
            ..Default::default()
        }
    }));

    // Clicks inside the panel do not dismiss it; scroll wheel cycles selection.
    let panel_interactive = mouse_area(panel)
        .on_press(Msg::Ignore)
        .on_scroll(Msg::OnScroll);

    let centered = container(panel_interactive)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill);

    // Clicks on the backdrop outside the panel cancel and close.
    let backdrop = mouse_area(
        container(horizontal_space())
            .width(Length::Fill)
            .height(Length::Fill)
            .class(Container::custom(|_| container::Style {
                background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.24))),
                ..Default::default()
            })),
    )
    .on_press(Msg::Cancel)
    .on_right_press(Msg::Cancel)
    .on_scroll(Msg::OnScroll);

    iced::widget::stack![backdrop, centered].into()
}

fn window_card<'a>(
    app: &'a SwitcherApp,
    win: &'a super::wayland::WinInfo,
    selected: bool,
    hovered: bool,
    spec: &GridSpec,
) -> Element<'a, Msg> {
    let app_icon = app.icons.get(&win.app_id);
    let thumb = app.thumbs.get(&win.id);

    let preview_inner: Element<'a, Msg> = match thumb {
        Some((handle, _, _)) => Image::new(handle.clone())
            .width(Length::Fixed(spec.preview_w))
            .height(Length::Fixed(spec.preview_h))
            .content_fit(ContentFit::Contain)
            .border_radius(THUMB_RADIUS)
            .into(),
        None => container(icon::icon(app_icon.clone()).size(56))
            .center_x(Length::Fixed(spec.preview_w))
            .center_y(Length::Fixed(spec.preview_h))
            .into(),
    };

    let preview_box = container(preview_inner)
        .width(Length::Fixed(spec.preview_w))
        .height(Length::Fixed(spec.preview_h))
        .center_x(Length::Fixed(spec.preview_w))
        .center_y(Length::Fixed(spec.preview_h))
        .class(Container::custom(|theme: &cosmic::Theme| {
            let mut bg: Color = theme.cosmic().on_bg_color().into();
            bg.a = if theme.cosmic().is_dark { 0.06 } else { 0.05 };
            container::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    radius: THUMB_RADIUS.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        }));

    let display_title = if win.title.trim().is_empty() {
        win.app_id.clone()
    } else {
        win.title.clone()
    };

    let title_text = text(display_title)
        .size(13.0)
        .width(Length::Fill)
        .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)));

    let close_el: Element<'a, Msg> = if hovered || selected {
        button::custom(icon::from_name("window-close-symbolic").size(12))
            .class(Button::Destructive)
            .padding(4)
            .on_press(Msg::CloseWindow(win.id.clone()))
            .into()
    } else {
        horizontal_space().width(Length::Fixed(20.0)).into()
    };

    let footer = row![icon::icon(app_icon).size(20), title_text, close_el]
        .spacing(8)
        .align_y(Alignment::Center)
        .width(Length::Fixed(spec.preview_w))
        .height(Length::Fixed(LABEL_H));

    let content = column![preview_box, footer]
        .spacing(6)
        .align_x(Alignment::Center);

    let id_press = win.id.clone();
    let id_middle = win.id.clone();
    let id_enter = win.id.clone();

    content
        .apply(button::custom)
        .padding(Padding::from(CARD_PAD))
        .class(card_button_style(selected))
        .on_press(Msg::Activate(id_press))
        .apply(mouse_area)
        .on_enter(Msg::Hover(Some(id_enter)))
        .on_exit(Msg::Hover(None))
        .on_middle_press(Msg::CloseWindow(id_middle))
        .into()
}

fn card_button_style(selected: bool) -> Button {
    let make = move |theme: &cosmic::Theme, hovered: bool| {
        let c = theme.cosmic();
        let accent: Color = c.accent_color().into();
        let mut bg: Color = if selected {
            accent
        } else {
            c.on_bg_color().into()
        };
        bg.a = match (selected, hovered) {
            (true, true) => 0.22,
            (true, false) => 0.16,
            (false, true) => 0.08,
            (false, false) => 0.02,
        };
        let mut border_color: Color = if selected {
            accent
        } else {
            c.on_bg_color().into()
        };
        if !selected {
            border_color.a = if hovered { 0.22 } else { 0.06 };
        }
        button::Style {
            background: Some(Background::Color(bg)),
            border_radius: CARD_RADIUS.into(),
            border_width: if selected { 2.5 } else { 1.0 },
            border_color,
            text_color: Some(c.on_bg_color().into()),
            icon_color: Some(c.on_bg_color().into()),
            outline_width: 0.0,
            ..button::Style::new()
        }
    };
    Button::Custom {
        active: Box::new(move |_, theme| make(theme, false)),
        hovered: Box::new(move |_, theme| make(theme, true)),
        pressed: Box::new(move |_, theme| make(theme, true)),
        disabled: Box::new(move |theme| make(theme, false)),
    }
}
