// SPDX-License-Identifier: GPL-3.0-only

//! Primitivas visuais compartilhadas pelos estilos: vidro, cartão, campo (retângulo do Windows 11
//! ou pílula), botões de canto, menus, texto com sombra, véu, ícones de status, widget de vidro e
//! barra de início. Recebem cores prontas (a `Palette` decide); `alpha` é o fade da peça inteira.


use std::borrow::Cow;

use cosmic::iced::alignment::Horizontal;
use cosmic::iced::{self, Alignment, Background, Border, Color, Font, Length, Shadow, Vector};
use cosmic::{Element, theme, widget};

use crate::login_ui::{Hovers, black, fade};
use crate::offset::offset;
use crate::style_config::StyleConfig;

mod field;
mod menu;
mod status;

pub use field::{FieldColors, FieldShape, field};
pub use menu::{ButtonShape, choice, power};
pub use status::status;

const TILE_HEIGHT: f32 = 44.0;
const TILE_RADIUS: f32 = 16.0;
const HOME_BAR_HEIGHT: f32 = 4.0;

/// Opacidade e raio do cartão; `None` quando `card_opacity <= 0.01` (sem cartão, em qualquer
/// estilo e sem mínimos). O raio é sempre `card_radius`.
pub fn card_spec(cfg: &StyleConfig) -> Option<(f32, f32)> {
    (cfg.card_opacity > 0.01).then_some((cfg.card_opacity, cfg.card_radius))
}

/// Cartão atrás do bloco de login: `fill` com a opacidade da config (e o fade `alpha`), borda de
/// 1 px. Sem cartão passa o conteúdo direto.
pub fn card<'a, M: 'a>(
    content: Element<'a, M>,
    cfg: &StyleConfig,
    fill: Color,
    border: Color,
    alpha: f32,
) -> Element<'a, M> {
    let Some((opacity, radius)) = card_spec(cfg) else {
        return content;
    };
    widget::container(content)
        .padding([24, 28])
        .class(theme::Container::custom(move |_| widget::container::Style {
            background: Some(Background::Color(fade(fill, opacity * alpha))),
            border: Border { radius: radius.into(), width: 1.0, color: fade(border, alpha) },
            ..Default::default()
        }))
        .into()
}

/// Vidro: tinta `fill`, borda de 1 px e sombra suave. O desfoque vem do fundo (backdrop).
pub fn glass<'a, M: 'a>(
    content: Element<'a, M>,
    fill: Color,
    border: Color,
    shadow: Color,
    radius: f32,
    alpha: f32,
) -> widget::Container<'a, M, cosmic::Theme, cosmic::Renderer> {
    widget::container(content).class(theme::Container::custom(move |_| widget::container::Style {
        background: Some(Background::Color(fade(fill, alpha))),
        border: Border { radius: radius.into(), width: 1.0, color: fade(border, alpha) },
        shadow: Shadow { color: fade(shadow, alpha), offset: Vector::new(0.0, 4.0), blur_radius: 16.0 },
        ..Default::default()
    }))
}

fn with_hovers<'a, M: Clone + 'static>(element: impl Into<Element<'a, M>>, hovers: Option<Hovers<M>>) -> Element<'a, M> {
    let area = widget::mouse_area(element);
    match hovers {
        Some(Hovers { enter, exit }) => area.on_enter(enter).on_exit(exit).into(),
        None => area.into(),
    }
}

fn tinted_icon<'a, M: 'a>(name: &'static str, size: u16, color: Color) -> Element<'a, M> {
    widget::icon::from_name(name)
        .size(size)
        .icon()
        .class(theme::Svg::custom(move |_| iced::widget::svg::Style { color: Some(color) }))
        .into()
}

/// Campo de senha (ou de usuário) com a seta de enviar dentro, na `width` pedida.
// ---------------------------------------------------------------------------------------------
// Texto, véu, ícones, widget e barra de início
// ---------------------------------------------------------------------------------------------

/// Uma linha de texto com sombra suave em duas camadas (a de 1 px ocupa o layout, a de 2 px só
/// desenha); `shadow` transparente = texto simples. As cores passam por `fade(_, alpha)`.
pub fn text_line<'a, M: 'a>(
    s: Cow<'a, str>,
    size: f32,
    font: Font,
    color: Color,
    shadow: Color,
    alpha: f32,
    align: Horizontal,
) -> Element<'a, M> {
    let make = |c: Color| widget::text(s.clone()).size(size).font(font).align_x(align).class(theme::Text::Color(c));
    if shadow.a <= 0.005 {
        return make(fade(color, alpha)).into();
    }
    let far = Color { a: shadow.a * 0.45, ..shadow };
    iced::widget::stack![
        widget::container(make(fade(shadow, alpha))).padding([1, 0, 0, 0]),
        offset(make(fade(far, alpha)), Vector::new(0.0, 2.0)),
        make(fade(color, alpha)),
    ]
    .into()
}

/// O véu só existe com alfa acima de 0,005.
pub fn veil_needed(c: Color) -> bool {
    c.a > 0.005
}

/// Gradiente vertical suave atrás do conteúdo (transparente nas bordas, `veil` no miolo), para
/// dar contraste ao texto sobre papel de parede claro. A árvore é sempre a mesma (um container),
/// então o véu aparecer depois da análise do fundo não reseta o foco do campo; sem véu o
/// container não pinta nada.
pub fn veil<'a, M: 'a>(content: Element<'a, M>, veil: Color, radius: f32) -> Element<'a, M> {
    let background = veil_needed(veil).then(|| {
        let clear = Color { a: 0.0, ..veil };
        Background::Gradient(
            iced::gradient::Linear::new(std::f32::consts::PI)
                .add_stop(0.0, clear)
                .add_stop(0.3, veil)
                .add_stop(0.7, veil)
                .add_stop(1.0, clear)
                .into(),
        )
    });
    widget::container(content)
        .class(theme::Container::custom(move |_| widget::container::Style {
            background,
            border: Border { radius: radius.into(), ..Default::default() },
            ..Default::default()
        }))
        .into()
}

/// Linha de ícones de status (rede, bateria...); vazio = nada.
pub fn icon_row<'a, M: 'a>(icons: Vec<&'static str>, color: Color, size: u16, alpha: f32) -> Element<'a, M> {
    let color = fade(color, alpha);
    let children: Vec<Element<'a, M>> = icons.into_iter().map(|name| tinted_icon(name, size, color)).collect();
    widget::Row::with_children(children)
        .spacing(f32::from(size) / 3.0)
        .align_y(Alignment::Center)
        .into()
}

/// Widget de vidro do iPadOS: ícone, valor em destaque e rótulo.
pub fn tile<'a, M: 'a>(
    value: String,
    label: String,
    icon: &'static str,
    fill: Color,
    border: Color,
    text: Color,
    alpha: f32,
) -> Element<'a, M> {
    let ink = fade(text, alpha);
    let row = widget::row![
        tinted_icon(icon, 18, ink),
        widget::text(value).size(17).font(cosmic::font::bold()).class(theme::Text::Color(ink)),
        widget::text(label).size(12).class(theme::Text::Color(fade(text, alpha * 0.75))),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    glass(row.into(), fill, border, black(0.12), TILE_RADIUS, alpha)
        .padding([0, 12])
        .width(Length::Fill)
        .height(Length::Fixed(TILE_HEIGHT))
        .align_y(Alignment::Center)
        .into()
}

/// Barra de início (a pílula fina do iOS e do iPadOS).
pub fn home_bar<'a, M: 'a>(color: Color, width: f32, alpha: f32) -> Element<'a, M> {
    widget::container(widget::space::horizontal())
        .width(Length::Fixed(width))
        .height(Length::Fixed(HOME_BAR_HEIGHT))
        .class(theme::Container::custom(move |_| widget::container::Style {
            background: Some(Background::Color(fade(color, alpha))),
            border: Border { radius: (HOME_BAR_HEIGHT / 2.0).into(), ..Default::default() },
            ..Default::default()
        }))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::styles::STYLE_NAMES;

    #[test]
    fn card_zero_opacity_means_no_card() {
        for name in STYLE_NAMES {
            let mut cfg = StyleConfig { style: name.to_string(), card_opacity: 0.0, ..StyleConfig::default() };
            assert_eq!(card_spec(&cfg), None, "{name}");
            cfg.card_opacity = 0.3;
            cfg.card_radius = 4.0;
            assert_eq!(card_spec(&cfg), Some((0.3, 4.0)), "{name}");
        }
    }

    #[test]
    fn veil_zero_alpha_is_passthrough() {
        assert!(!veil_needed(Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }));
        assert!(!veil_needed(Color { r: 1.0, g: 1.0, b: 1.0, a: 0.005 }));
        assert!(veil_needed(Color { r: 0.0, g: 0.0, b: 0.0, a: 0.3 }));
    }
}
