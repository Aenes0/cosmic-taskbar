// SPDX-License-Identifier: GPL-3.0-only

//! A window's preview card, as the taskbar and the Start menu both show it:
//! the window's picture with round corners and no face, the app's icon on its
//! lower right corner, and, under the pointer, a round close button at the
//! upper right and the window's title over the picture's foot.
//! Every picture fits its box by its own shape. At rest it is a little
//! smaller, and grows into the box under the pointer.

use cosmic::{
    Apply, Element,
    iced::{
        self, Alignment, Background, Border, Length,
        advanced::text::{Ellipsize, EllipsizeHeightLimit},
        widget::{mouse_area, space::horizontal as horizontal_space},
    },
    theme::{self, Button, Container},
    widget::{Image, button, container, icon, image::Handle, text},
};

/// A window's picture and its size in pixels.
pub type Picture = (Handle, u32, u32);

/// How much the picture grows under the pointer.
pub const LIFT: f32 = 0.03;
pub const RADIUS: f32 = 10.0;
pub const BADGE: f32 = 30.0;

/// The label's normal size, and the two smaller ones it falls back to.
const LABEL_SIZES: [f32; 3] = [14.0, 12.0, 11.0];
/// How much of its size an average character takes of width.
const LABEL_CHAR_EM: f32 = 0.56;

/// The picture's size, fitted by the window's own shape (`dims`) into
/// `max_w` x `max_h`; with no picture, a smaller box for the app's icon.
pub fn picture_size(dims: Option<(u32, u32)>, max_w: f32, max_h: f32) -> (f32, f32) {
    match dims {
        Some((w, h)) if w > 0 && h > 0 => {
            let scale = (max_w / w as f32).min(max_h / h as f32);
            (
                (w as f32 * scale).round().max(1.0),
                (h as f32 * scale).round().max(1.0),
            )
        }
        _ => (max_w * 0.66, max_h * 0.83),
    }
}

/// The label's font size and number of lines for `title` in `width` px: one
/// line at the normal size if it fits, else two lines, and two lines two and
/// then three points smaller before it is cut.
pub fn label_layout(title: &str, width: f32) -> (f32, u16) {
    let chars = title.chars().count() as f32;
    let per_line = |size: f32| (width / (LABEL_CHAR_EM * size)).floor();
    let [normal, smaller, smallest] = LABEL_SIZES;
    if chars <= per_line(normal) {
        (normal, 1)
    } else if chars <= 2.0 * per_line(normal) - 2.0 {
        // Two lines lose a little to where the words break.
        (normal, 2)
    } else if chars <= 2.0 * per_line(smaller) - 2.0 {
        (smaller, 2)
    } else {
        (smallest, 2)
    }
}

/// What a card shows and what it says when used.
pub struct CardParams<M> {
    pub picture: Option<Picture>,
    pub app_icon: icon::Handle,
    pub title: String,
    pub hovered: bool,
    /// How far (0 to 1) the card has faded in.
    pub fade: f32,
    /// How far the card has grown under the pointer.
    pub lift: f32,
    /// The box the picture fits in.
    pub max_w: f32,
    pub max_h: f32,
    pub on_press: M,
    pub on_close: M,
    pub on_middle: M,
    pub on_enter: M,
    pub on_exit: M,
}

/// One window's card, `max_w` x `max_h` or smaller, by the picture's shape.
pub fn card<'a, M: Clone + 'static>(p: CardParams<M>) -> Element<'a, M> {
    let t = p.fade;
    let fade = move |mut c: iced::Color| {
        c.a *= t;
        c
    };
    let has_picture = p.picture.is_some();
    let grow = 1.0 + LIFT * p.lift.clamp(0.0, 1.2);
    let rest = 1.0 / (1.0 + LIFT);

    let (pw, ph) = picture_size(p.picture.as_ref().map(|(_, w, h)| (*w, *h)), p.max_w, p.max_h);
    let (box_w, box_h) = (pw, ph);
    let (dw, dh) = ((pw * rest * grow).round(), (ph * rest * grow).round());

    let picture: Element<_> = match p.picture {
        Some((handle, _, _)) => Image::new(handle)
            .width(Length::Fixed(dw))
            .height(Length::Fixed(dh))
            .content_fit(iced::ContentFit::Fill)
            .border_radius(RADIUS)
            .opacity(t)
            .into(),
        // A minimized window has no picture: its app's icon, big, instead.
        None => icon::icon(p.app_icon.clone()).size(48).into(),
    };
    let frame = container(picture)
        .center(Length::Fixed(dw))
        .height(Length::Fixed(dh))
        .class(Container::custom(move |theme| {
            let cosmic = theme.cosmic();
            container::Style {
                background: (!has_picture).then(|| {
                    Background::Color(fade(iced::Color {
                        a: 0.10,
                        ..cosmic.on_bg_color().into()
                    }))
                }),
                border: Border {
                    radius: RADIUS.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        }));

    // The app's icon on the picture's lower right corner, as on macOS; it has
    // no alpha, so it shows once the card is mostly in.
    let badge: Element<_> = if t > 0.6 && !p.hovered {
        container(icon::icon(p.app_icon).size(BADGE as u16))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::End)
            .align_y(Alignment::End)
            .into()
    } else {
        horizontal_space().width(Length::Shrink).into()
    };
    // A round close button at the upper right, only while the pointer is on the card.
    let close: Element<_> = if p.hovered {
        container(
            button::custom(icon::from_name("window-close-symbolic").size(12))
                .class(Button::Destructive)
                .on_press(p.on_close)
                .padding(4),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(6)
        .align_x(Alignment::End)
        .align_y(Alignment::Start)
        .into()
    } else {
        horizontal_space().width(Length::Shrink).into()
    };
    // The window's title, over the picture's foot, only while the pointer is
    // on it (it takes the app icon's place there): nothing is reserved for it.
    let caption: Element<_> = if p.hovered && !p.title.trim().is_empty() {
        let (size, lines) = label_layout(&p.title, pw - 2.0 * 6.0 - 2.0 * 8.0);
        let ink = theme::active().cosmic().on_bg_color();
        container(
            container(
                text(p.title)
                    .size(size)
                    .line_height(1.2)
                    .class(theme::Text::Color(ink.into()))
                    .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(lines as usize))),
            )
            .padding([4, 8])
            .class(Container::custom(|theme: &cosmic::Theme| {
                let mut base: iced::Color = theme.cosmic().background(theme.transparent).base.into();
                base.a = 1.0;
                container::Style {
                    background: Some(Background::Color(base)),
                    border: Border {
                        radius: 8.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            })),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(6)
        .align_x(Alignment::Start)
        .align_y(Alignment::End)
        .into()
    } else {
        horizontal_space().width(Length::Shrink).into()
    };
    let centered = container(frame)
        .width(Length::Fixed(box_w))
        .height(Length::Fixed(box_h))
        .center_x(Length::Fixed(box_w))
        .center_y(Length::Fixed(box_h));
    let card = container(iced::widget::stack![centered, close, badge, caption])
        .width(Length::Fixed(box_w))
        .height(Length::Fixed(box_h));

    card.apply(button::custom)
        .on_press(p.on_press)
        .class(card_style())
        .padding(0)
        .apply(mouse_area)
        .on_enter(p.on_enter)
        .on_middle_press(p.on_middle)
        .on_exit(p.on_exit)
        .apply(Element::from)
}

/// A preview card has no button face: the picture itself is the card.
pub fn card_style() -> cosmic::theme::Button {
    let flat = |theme: &cosmic::Theme, base: button::Style| button::Style {
        background: None,
        border_radius: theme.cosmic().radius_m().into(),
        outline_width: 0.0,
        ..base
    };
    Button::Custom {
        active: Box::new(move |focused, theme| {
            flat(theme, button::Catalog::active(theme, focused, false, &Button::AppletMenu))
        }),
        hovered: Box::new(move |focused, theme| {
            flat(theme, button::Catalog::hovered(theme, focused, false, &Button::AppletMenu))
        }),
        disabled: Box::new(move |theme| {
            flat(theme, button::Catalog::disabled(theme, &Button::AppletMenu))
        }),
        pressed: Box::new(move |focused, theme| {
            flat(theme, button::Catalog::pressed(theme, focused, false, &Button::AppletMenu))
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The taskbar's box.
    const MAX_W: f32 = 300.0;
    const MAX_H: f32 = 150.0;

    fn size(dims: Option<(u32, u32)>) -> (f32, f32) {
        picture_size(dims, MAX_W, MAX_H)
    }

    #[test]
    fn a_portrait_card_is_as_wide_as_its_picture_with_nothing_empty_at_the_sides() {
        let (pw, ph) = size(Some((1080, 1876)));
        assert!(pw < 100.0, "pw={pw}");
        assert_eq!(ph, MAX_H);
    }

    #[test]
    fn a_wide_window_keeps_its_own_width_up_to_the_maximum() {
        assert_eq!(size(Some((3440, 1396))).0, MAX_W);
        let (pw, ph) = size(Some((1920, 1080)));
        assert!((pw / ph - 16.0 / 9.0).abs() < 0.02, "{pw}x{ph}");
    }

    #[test]
    fn no_picture_still_has_a_card() {
        let (pw, ph) = size(None);
        assert!(pw > 0.0 && pw < MAX_W && ph > 0.0 && ph < MAX_H, "{pw}x{ph}");
    }

    #[test]
    fn a_picture_fits_any_box_it_is_given() {
        let (pw, ph) = picture_size(Some((1920, 1080)), 216.0, 156.0);
        assert_eq!(pw, 216.0);
        assert!(ph <= 156.0, "ph={ph}");
    }

    #[test]
    fn a_short_title_is_one_line_at_the_normal_size() {
        assert_eq!(label_layout("Terminal", 300.0), (14.0, 1));
    }

    #[test]
    fn a_longer_title_wraps_to_two_lines_before_it_shrinks() {
        let title = "Visualização em gráfico - Planilha de custos";
        assert_eq!(label_layout(title, 190.0), (14.0, 2));
    }

    #[test]
    fn a_long_title_shrinks_two_points_then_three_then_is_cut() {
        let long = "Visualização em gráfico das vendas por região 2026";
        assert_eq!(label_layout(long, 190.0), (12.0, 2));
        let very_long = "Visualização em gráfico das vendas por região e por mês - Planilha".repeat(2);
        assert_eq!(label_layout(&very_long, 190.0), (11.0, 2));
    }

    #[test]
    fn the_label_layout_survives_an_empty_title_and_no_room() {
        assert_eq!(label_layout("", 190.0), (14.0, 1));
        assert_eq!(label_layout("abc", 0.0).1, 2);
    }

    #[test]
    fn a_degenerate_picture_does_not_divide_by_zero() {
        let (pw, ph) = size(Some((0, 0)));
        assert!(pw > 0.0 && ph > 0.0);
    }
}
