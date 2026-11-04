// Copyright 2023 System76 <info@system76.com>, 2026 Aenes0
// SPDX-License-Identifier: GPL-3.0-only

//! Drawing of the bar (chosen icons + ^) and of the box (the rest, in a grid).

use cosmic::{
    Element,
    applet::cosmic_panel_config::PanelAnchor,
    iced::{Length, window},
    widget::{DndDestination, container, mouse_area},
};

use super::{
    app::{App, Msg},
    drag::{self, Cell, Origin},
    status_menu,
};
use crate::central::tray::dnd::TrayDrag;

/// Icons per row in the box.
pub(super) const BOX_COLUMNS: usize = 5;
/// Padding (px) around the grid of the box.
pub(super) const BOX_PADDING: i32 = 8;

/// The cell of the `index`-th icon of the box, relative to the box surface:
/// where the menu of a box icon anchors.
pub(super) fn box_cell_anchor(
    index: usize,
    columns: usize,
    button_len: f32,
    padding: i32,
) -> cosmic::iced::Rectangle<i32> {
    let columns = columns.max(1);
    let len = button_len as i32;
    cosmic::iced::Rectangle {
        x: padding + (index % columns) as i32 * len,
        y: padding + (index / columns) as i32 * len,
        width: len,
        height: len,
    }
}

/// The bar: the visible icons in the chosen order (with the gap of a drag),
/// then the ^ if anything is hidden or a drag from the bar is on.
pub(super) fn view(app: &App) -> Element<'_, Msg> {
    app.core.applet.autosize_window(bar(app)).into()
}

/// The same bar without autosize: for the host applet to embed in the corner.
pub(super) fn corner_view(app: &App) -> Element<'_, Msg> {
    bar(app)
}

fn bar(app: &App) -> Element<'_, Msg> {
    let applet = &app.core.applet;
    let split = app.split();
    let main_id = app.core.main_window_id();
    let bar = app.bar_row(&split);

    let cells = bar.cells.iter().filter_map(|cell| match *cell {
        Cell::Item(id) => {
            let menu = app.menus.get(&id)?;
            let button = mouse_area(menu_icon_button(applet, menu).on_press(Msg::Activate(id)))
                .on_right_press(Msg::TogglePopup(id))
                .on_enter(Msg::Hovered(Some(id)))
                .on_exit(Msg::Hovered(None));
            Some(drag::source(button, id, menu, Origin::Bar, main_id))
        }
        Cell::Gap => Some(drag::gap(app.button_len())),
    });
    let ghosts = bar.ghosts.iter().filter_map(|&id| {
        let menu = app.menus.get(&id)?;
        Some(drag::ghost(id, menu, main_id))
    });
    let cells: Vec<Element<'_, Msg>> = cells.chain(ghosts).collect();
    let queue = DndDestination::for_data::<TrayDrag>(line(app, cells), |data, _| {
        Msg::DropOnBar(data)
    })
    .drag_id(drag::DND_BAR)
    .on_enter(|x, y, _mimes| Msg::BarEnter(x, y))
    .on_motion(Msg::BarMotion)
    .on_leave(|| Msg::BarLeave);

    // The ^ is a sibling of the bar's target, never inside it: a drop on the ^
    // must not also be a drop on the bar.
    let mut children: Vec<Element<'_, Msg>> = vec![queue.into()];
    if split.show_arrow {
        let arrow = mouse_area(
            applet
                .icon_button(match applet.anchor {
                    PanelAnchor::Bottom => "go-up-symbolic",
                    PanelAnchor::Left => "go-next-symbolic",
                    PanelAnchor::Right => "go-previous-symbolic",
                    PanelAnchor::Top => "go-down-symbolic",
                })
                .on_press_down(Msg::ToggleOverflow),
        )
        .on_enter(Msg::HoveredOverflow);
        children.push(
            DndDestination::for_data::<TrayDrag>(arrow, |data, _| Msg::DropOnArrow(data))
                .drag_id(drag::DND_ARROW)
                .into(),
        );
    }

    line(app, children)
}

/// A row, or a column on a vertical panel. No Length::Fill here: inside
/// autosize_window it asks for an infinite size.
fn line<'a>(app: &App, children: Vec<Element<'a, Msg>>) -> Element<'a, Msg> {
    if app.is_vertical() {
        cosmic::iced::widget::column(children).into()
    } else {
        cosmic::iced::widget::row(children).into()
    }
}

/// The box: hidden icons in rows of up to five, each with its name as a tooltip.
pub(super) fn view_box(app: &App, box_id: window::Id) -> Element<'_, Msg> {
    let applet = &app.core.applet;
    let split = app.split();

    let rows = split.boxed.chunks(BOX_COLUMNS).map(|chunk| {
        let icons = chunk.iter().filter_map(|&id| {
            let menu = app.menus.get(&id)?;
            let button = mouse_area(menu_icon_button(applet, menu).on_press(Msg::ActivateFromBox(id)))
                .on_right_press(Msg::MenuFromBox(id))
                .on_enter(Msg::Hovered(Some(id)))
                .on_exit(Msg::Hovered(None));
            let button = drag::source(button, id, menu, Origin::Box, Some(box_id));
            Some(match menu.identity.label() {
                Some(label) => applet
                    .applet_tooltip(button, label.to_owned(), false, Msg::Surface, Some(box_id))
                    .into(),
                None => Element::from(button),
            })
        });
        Element::from(cosmic::iced::widget::row(icons))
    });

    applet
        .popup_container(container(cosmic::iced::widget::column(rows)).padding(BOX_PADDING as f32))
        .into()
}

pub(super) fn menu_icon_button<'a>(
    applet: &'a cosmic::applet::Context,
    menu: &'a status_menu::State,
) -> cosmic::widget::Button<'a, Msg> {
    let icon = menu.icon_handle().clone();

    let suggested = applet.suggested_size(true);
    let padding = applet.suggested_padding(true).1;
    let symbolic = icon.symbolic;

    cosmic::widget::button::custom(
        cosmic::widget::layer_container(
            cosmic::widget::icon(icon)
                .class(if symbolic {
                    cosmic::theme::Svg::Custom(std::rc::Rc::new(|theme| {
                        cosmic::iced::widget::svg::Style {
                            color: Some(theme.cosmic().background(theme.transparent).on.into()),
                        }
                    }))
                } else {
                    cosmic::theme::Svg::default()
                })
                .width(Length::Fixed(suggested.0 as f32))
                .height(Length::Fixed(suggested.1 as f32)),
        )
        .center(Length::Fill),
    )
    .width(Length::Fixed((suggested.0 + 2 * padding) as f32))
    .height(Length::Fixed((suggested.1 + 2 * padding) as f32))
    .class(cosmic::theme::Button::AppletIcon)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_cell_anchor_places_the_index_in_the_grid() {
        let r = box_cell_anchor(7, 5, 30.0, 8);
        assert_eq!((r.x, r.y, r.width, r.height), (68, 38, 30, 30));
    }

    #[test]
    fn box_cell_anchor_first_cell_is_at_the_padding() {
        let r = box_cell_anchor(0, 5, 30.0, 8);
        assert_eq!((r.x, r.y), (8, 8));
        let last_of_row = box_cell_anchor(4, 5, 30.0, 8);
        assert_eq!((last_of_row.x, last_of_row.y), (128, 8));
    }
}
