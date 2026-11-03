//! The window preview's card: a grid of window cards over an open-app icon.

use super::*;
use super::preview::{self, CARD_H, CARD_W, GAP, PAD};

/// Room left around a window's picture inside its cell of the grid.
const CELL_INSET: f32 = 4.0;

/// The box a window's picture fits in, inside a `cell_w` x `cell_h` cell.
fn picture_box(cell_w: f32, cell_h: f32) -> (f32, f32) {
    ((cell_w - 2.0 * CELL_INSET).max(1.0), (cell_h - 2.0 * CELL_INSET).max(1.0))
}

/// The title to show: an empty or blank one reads "untitled".
fn card_title(title: &str) -> &str {
    if title.trim().is_empty() { texts().untitled } else { title }
}

/// `items` in rows of `cols` (0 counts as 1).
fn rows_of<T: Clone>(items: &[T], cols: usize) -> Vec<Vec<T>> {
    items.chunks(cols.max(1)).map(<[T]>::to_vec).collect()
}

/// True when the grid is taller than the room the card has.
fn needs_scroll(natural_h: f32, max_h: f32) -> bool {
    natural_h > max_h
}

impl StartMenu {
    /// The preview of the open app `key`, already placed over `anchor` inside
    /// `bounds`; `None` when the app has no windows.
    pub(super) fn preview_card(
        &self,
        key: &str,
        anchor: iced::Rectangle,
        bounds: iced::Rectangle,
        p: Palette,
    ) -> Option<Element<'_, Message>> {
        let group = self.group_of(key).filter(|g| !g.windows.is_empty())?;
        let windows: Vec<&Window> = group.windows.iter().filter_map(|&i| self.windows.get(i)).collect();
        if windows.is_empty() {
            return None;
        }
        let n = windows.len();
        let rect = preview::card_rect(anchor, n, bounds);
        let (cols, rows, _) = preview::grid(n, bounds.size());
        let mut grid = widget::column::with_capacity(rows).spacing(GAP);
        for line in rows_of(&windows, cols) {
            let mut row = widget::row::with_capacity(line.len()).spacing(GAP);
            for window in line {
                row = row.push(self.window_card(window));
            }
            grid = grid.push(row);
        }
        let natural = 2.0 * PAD + rows as f32 * CARD_H + rows.saturating_sub(1) as f32 * GAP;
        let grid = widget::container(grid).padding(PAD);
        let body: Element<Message> = if needs_scroll(natural, rect.height) {
            widget::scrollable(grid).height(Length::Fixed(rect.height)).into()
        } else {
            grid.into()
        };
        let card = widget::container(body)
            .width(Length::Fixed(rect.width))
            .height(Length::Fixed(rect.height))
            .class(theme::Container::custom(move |_| widget::container::Style {
                background: Some(Background::Color(p.popup)),
                text_color: Some(p.text),
                icon_color: Some(p.text),
                border: iced::Border { radius: 8.0.into(), width: 1.0, color: p.border },
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 16.0,
                },
                ..Default::default()
            }));
        // Presses on the card must not reach the backdrop, which would hide the menu.
        let card = widget::mouse_area(card)
            .on_press(Message::Nothing)
            .on_right_press(Message::Nothing)
            .interaction(iced::mouse::Interaction::Idle);
        Some(iced::widget::pin(card).x(rect.x).y(rect.y).width(Length::Fill).height(Length::Fill).into())
    }

    /// One window, as the taskbar previews it, centred in its cell of the grid.
    fn window_card(&self, window: &Window) -> Element<'_, Message> {
        let id = window.id.clone();
        let hot = self.preview_hot.as_deref() == Some(id.as_str());
        let (max_w, max_h) = picture_box(CARD_W, CARD_H);
        let card = shared_card::card(shared_card::CardParams {
            // No picture (minimized, or not taken yet): the app's icon, big.
            picture: self.thumbs.get(&id).cloned(),
            app_icon: self.window_icon(window),
            title: card_title(&window.title).to_string(),
            hovered: hot,
            fade: 1.0,
            lift: if hot { 1.0 } else { 0.0 },
            max_w,
            max_h,
            on_press: Message::Activate(id.clone()),
            on_close: Message::CloseWindow(id.clone()),
            on_middle: Message::CloseWindow(id.clone()),
            on_enter: Message::PreviewHot(Some(id)),
            on_exit: Message::PreviewHot(None),
        });
        widget::container(card)
            .width(Length::Fixed(CARD_W))
            .height(Length::Fixed(CARD_H))
            .center_x(Length::Fixed(CARD_W))
            .center_y(Length::Fixed(CARD_H))
            .into()
    }
}

#[cfg(test)]
mod tests;
