// SPDX-License-Identifier: GPL-3.0-only

//! Botões de canto, menu de energia e menu de escolha.

use cosmic::iced::{self, Alignment, Background, Border, Color, Length, Point, Shadow, Vector};
use cosmic::{Element, theme, widget};

use super::{tinted_icon, with_hovers};
use crate::fl;
use crate::login_ui::{black, fade, mix, shift, white};
use crate::motion::Look;
use crate::offset::offset;
use crate::power::PowerAction;
use crate::styles::{ChoiceMenu, PowerParts};

const CORNER_SIZE: f32 = 40.0;
const SQUIRCLE_RATIO: f32 = 0.35;
const MENU_WIDTH: f32 = 220.0;
/// Botão ⏻ do Win11 (`shape = None`): ícone de 24 px, padding 10, menu de 200 px.
const WIN11_POWER_ICON: u16 = 24;
const WIN11_POWER_PADDING: u16 = 10;
const WIN11_POWER_SIZE: f32 = WIN11_POWER_ICON as f32 + 2.0 * WIN11_POWER_PADDING as f32;
const WIN11_MENU_WIDTH: f32 = 200.0;
const MENU_GAP: f32 = 8.0;
const MENU_PADDING: u16 = 4;
const MENU_RADIUS: f32 = 12.0;
const MENU_ITEM_HEIGHT: f32 = 36.0;
const MENU_ICON_SIZE: u16 = 16;
const POWER_SVG: &[u8] = include_bytes!("../../../res/power-thin.svg");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonShape {
    Circle,
    Squircle,
}

fn shape_radius(shape: ButtonShape, size: f32) -> f32 {
    match shape {
        ButtonShape::Circle => size / 2.0,
        ButtonShape::Squircle => size * SQUIRCLE_RATIO,
    }
}

/// Estilo do botão de canto: `hover` (0..=1, animado de fora) clareia o fundo em direção a `ink`;
/// com `native_hover` o estado "hovered" do iced vale hover 1. Fundo translúcido ganha a borda de vidro.
fn corner_style(fill: Color, ink: Color, radius: f32, hover: f32, native_hover: bool, alpha: f32) -> theme::Button {
    let tinted = |t: f32| {
        let target = Color { a: (fill.a + 0.10).min(1.0), ..mix(fill, ink, 0.15) };
        mix(fill, target, t)
    };
    let style = move |background: Color| {
        let mut s = widget::button::Style::new();
        s.background = Some(Background::Color(fade(background, alpha)));
        s.border_radius = radius.into();
        if fill.a < 0.9 {
            s.border_width = 1.0;
            s.border_color = fade(white(0.28), alpha);
        }
        s.icon_color = Some(fade(ink, alpha));
        s.text_color = Some(fade(ink, alpha));
        s
    };
    let rest = tinted(hover);
    let hovered = if native_hover { tinted(1.0) } else { rest };
    theme::Button::Custom {
        active: Box::new(move |_, _| style(rest)),
        disabled: Box::new(move |_| style(fill)),
        hovered: Box::new(move |_, _| style(hovered)),
        pressed: Box::new(move |_, _| style(mix(fill, ink, 0.25))),
    }
}

fn round_button<'a, M: Clone + 'static>(
    content: impl Into<Element<'a, M>>,
    style: theme::Button,
    size: f32,
    on_press: M,
) -> widget::Button<'a, M> {
    widget::button::custom(content)
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .padding(((size - size * 0.5) / 2.0) as u16)
        .class(style)
        .on_press(on_press)
}

/// Cores do menu: sobre ícone claro, escuro fixo; sobre ícone escuro, a própria tinta do botão
/// (quase opaca). Devolve (fundo, texto).
fn menu_palette(fill: Color, ink: Color) -> (Color, Color) {
    let luma = 0.2126 * ink.r + 0.7152 * ink.g + 0.0722 * ink.b;
    if luma > 0.5 {
        (Color { r: 0.17, g: 0.17, b: 0.17, a: 0.96 }, ink)
    } else {
        (Color { a: 0.96, ..fill }, ink)
    }
}

fn menu_item<'a, M: Clone + 'static>(
    leading: Element<'a, M>,
    label: String,
    on_press: M,
    ink: Color,
    alpha: f32,
) -> Element<'a, M> {
    let style = move |background: Option<Color>| {
        let mut s = widget::button::Style::new();
        s.background = background.map(|c| Background::Color(fade(c, alpha)));
        s.border_radius = (MENU_RADIUS - f32::from(MENU_PADDING)).into();
        s.icon_color = Some(fade(ink, alpha));
        s.text_color = Some(fade(ink, alpha));
        s
    };
    let content = widget::row![leading, widget::text(label).size(14).class(theme::Text::Color(fade(ink, alpha)))]
        .spacing(12)
        .align_y(Alignment::Center)
        .height(Length::Fill);
    widget::button::custom(content)
        .width(Length::Fill)
        .height(Length::Fixed(MENU_ITEM_HEIGHT))
        .padding([0, 12])
        .class(theme::Button::Custom {
            active: Box::new(move |_, _| style(None)),
            disabled: Box::new(move |_| style(None)),
            hovered: Box::new(move |_, _| style(Some(Color { a: 0.08, ..ink }))),
            pressed: Box::new(move |_, _| style(Some(Color { a: 0.05, ..ink }))),
        })
        .on_press(on_press)
        .into()
}

fn menu_surface<'a, M: 'a>(items: Vec<Element<'a, M>>, width: f32, background: Color, ink: Color, look: Look) -> Element<'a, M> {
    let alpha = look.alpha;
    let menu = widget::container(widget::Column::with_children(items))
        .padding(MENU_PADDING)
        .width(Length::Fixed(width))
        .class(theme::Container::custom(move |_| widget::container::Style {
            icon_color: Some(fade(ink, alpha)),
            text_color: Some(fade(ink, alpha)),
            background: Some(Background::Color(fade(background, alpha))),
            border: Border { radius: MENU_RADIUS.into(), width: 1.0, color: fade(white(0.10), alpha) },
            shadow: Shadow { color: fade(black(0.25), alpha), offset: Vector::new(0.0, 6.0), blur_radius: 18.0 },
            ..Default::default()
        }));
    offset(menu, shift(look)).into()
}

fn power_glyph<'a, M: 'a>(size: u16, color: Color) -> Element<'a, M> {
    widget::svg(widget::svg::Handle::from_memory(POWER_SVG))
        .width(Length::Fixed(f32::from(size)))
        .height(Length::Fixed(f32::from(size)))
        .class(theme::Svg::custom(move |_| iced::widget::svg::Style { color: Some(color) }))
        .into()
}

/// Botão de energia: `shape = None` é o botão ⏻ redondo do Win11 (mesmo tamanho e menu de antes,
/// mas nas cores recebidas, não em branco fixo); com forma, um botão de canto. Ambos abrem o menu
/// (Suspender, Reiniciar, Desligar) acima dele.
pub fn power<'a, M: Clone + 'static>(
    p: PowerParts<M>,
    fill: Color,
    icon_color: Color,
    shape: Option<ButtonShape>,
) -> Element<'a, M> {
    let PowerParts { open, on_toggle, on_action, fx } = p;
    let alpha = fx.look.alpha;
    // `None`: o botão ⏻ redondo do Win11 (24 px de ícone, padding 10, menu de 200 px).
    let (button, size, menu_width) = match shape {
        Some(shape) => {
            let style = corner_style(fill, icon_color, shape_radius(shape, CORNER_SIZE), fx.hover, false, alpha);
            let glyph = power_glyph(CORNER_SIZE as u16 / 2, fade(icon_color, alpha));
            (round_button(glyph, style, CORNER_SIZE, on_toggle.clone()), CORNER_SIZE, MENU_WIDTH)
        }
        None => {
            let style = corner_style(fill, icon_color, WIN11_POWER_SIZE / 2.0, fx.hover, false, alpha);
            let button = widget::button::custom(power_glyph(WIN11_POWER_ICON, fade(icon_color, alpha)))
                .padding(WIN11_POWER_PADDING)
                .class(style)
                .on_press(on_toggle.clone());
            (button, WIN11_POWER_SIZE, WIN11_MENU_WIDTH)
        }
    };
    let button = widget::tooltip(with_hovers(button, fx.hovers), widget::text(fl!("power")), widget::tooltip::Position::Top);
    let mut popover = widget::popover(button)
        .position(widget::popover::Position::Point(Point::new(size - menu_width, -MENU_GAP)))
        .on_close(on_toggle);
    if open {
        let (background, ink) = menu_palette(fill, icon_color);
        let items = PowerAction::ALL
            .into_iter()
            .map(|action| {
                let leading = match action {
                    PowerAction::Shutdown => power_glyph(MENU_ICON_SIZE, fade(ink, fx.menu.alpha)),
                    _ => tinted_icon(action.icon_name(), MENU_ICON_SIZE, fade(ink, fx.menu.alpha)),
                };
                menu_item(leading, action.label(), on_action(action), ink, fx.menu.alpha)
            })
            .collect();
        popover = popover.popup(menu_surface(items, menu_width, background, ink, fx.menu));
    }
    offset(popover, shift(fx.look)).into()
}

/// Menu de escolha (sessão, layout do teclado). `shape = None` abre por um texto (o rótulo);
/// com forma, por um botão de canto com `icon`. Mesmo padrão do menu de energia: `popover` com
/// `Position::Point` e `on_close`; o menu cresce para cima, alinhado à esquerda do botão.
pub fn choice<'a, M: Clone + 'static>(
    menu: ChoiceMenu<M>,
    fill: Color,
    text: Color,
    shape: Option<ButtonShape>,
    icon: &'static str,
    look: Look,
) -> Element<'a, M> {
    let ChoiceMenu { open, on_toggle, label, items } = menu;
    let alpha = look.alpha;
    let trigger: Element<'a, M> = match shape {
        Some(shape) => {
            let style = corner_style(fill, text, shape_radius(shape, CORNER_SIZE), 0.0, true, alpha);
            let glyph = widget::icon::from_name(icon).size(CORNER_SIZE as u16 / 2).icon();
            widget::tooltip(
                round_button(glyph, style, CORNER_SIZE, on_toggle.clone()),
                widget::text(label.clone()),
                widget::tooltip::Position::Top,
            )
            .into()
        }
        None => {
            let style = move |background: Option<Color>| {
                let mut s = widget::button::Style::new();
                s.background = background.map(|c| Background::Color(fade(c, alpha)));
                s.border_radius = 6.0.into();
                s.icon_color = Some(fade(text, alpha));
                s.text_color = Some(fade(text, alpha));
                s
            };
            widget::button::custom(widget::text(label.clone()).size(12).class(theme::Text::Color(fade(text, alpha))))
                .padding([4, 8])
                .class(theme::Button::Custom {
                    active: Box::new(move |_, _| style(None)),
                    disabled: Box::new(move |_| style(None)),
                    hovered: Box::new(move |_, _| style(Some(Color { a: 0.12, ..text }))),
                    pressed: Box::new(move |_, _| style(Some(Color { a: 0.2, ..text }))),
                })
                .on_press(on_toggle.clone())
                .into()
        }
    };
    let mut popover = widget::popover(trigger)
        .position(widget::popover::Position::Point(Point::new(0.0, -MENU_GAP)))
        .on_close(on_toggle);
    if open {
        let (background, ink) = menu_palette(fill, text);
        let rows = items
            .into_iter()
            .map(|(label, checked, message)| {
                let mark: Element<'a, M> = if checked {
                    tinted_icon("object-select-symbolic", MENU_ICON_SIZE, fade(ink, alpha))
                } else {
                    widget::space::horizontal().width(Length::Fixed(f32::from(MENU_ICON_SIZE))).into()
                };
                menu_item(mark, label, message, ink, alpha)
            })
            .collect();
        popover = popover.popup(menu_surface(rows, MENU_WIDTH, background, ink, look));
    }
    offset(popover, shift(look)).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_palette_follows_ink_and_radius_follows_shape() {
        let fill = Color { r: 0.95, g: 0.86, b: 1.0, a: 1.0 };
        assert_eq!(menu_palette(fill, Color::WHITE).0.r, 0.17);
        assert_eq!(menu_palette(fill, black(0.9)).0, Color { a: 0.96, ..fill });
        assert_eq!(shape_radius(ButtonShape::Circle, 40.0), 20.0);
        assert_eq!(shape_radius(ButtonShape::Squircle, 40.0), 14.0);
    }
}
