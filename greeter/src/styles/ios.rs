// SPDX-License-Identifier: GPL-3.0-only

//! Estilo `ios` (iOS 26, antigo `editorial`): barra de status (hora à esquerda, ícones à direita),
//! data acima de um relógio gigante ExtraBold em "degradê de vidro", campo de vidro em pílula,
//! dois botões redondos de vidro nos cantos de baixo (sessão/teclado à esquerda, energia à
//! direita) e a barrinha inferior do iPhone.

use std::borrow::Cow;

use cosmic::iced::alignment::Horizontal;
use cosmic::iced::widget::text::LineHeight;
use cosmic::iced::{self, Alignment, Color, Font, Length, Vector};
use cosmic::{Element, theme, widget};

use super::palette::{FontRole, backdrop_luminance, min_alpha_for_contrast, palette, style_font};
use super::parts::{self, ButtonShape, FieldColors, FieldShape};
use super::ScreenInput;
use crate::backdrop::Region;
use crate::login_ui::{FIELD_WIDTH, fade, shift};
use crate::offset::offset;
use crate::sysinfo::network_icon_name;

/// Onde ficam a data e o relógio gigante (normalizado), para medir o fundo atrás deles.
pub const CLOCK_REGION: Region = Region { x0: 0.12, y0: 0.06, x1: 0.88, y1: 0.50 };
/// Onde ficam o nome e o campo de senha (normalizado).
pub const FIELD_REGION: Region = Region { x0: 0.33, y0: 0.58, x1: 0.67, y1: 0.82 };

/// Relógio gigante = fonte da hora do monitor × este fator (o protótipo usa ~37 % da altura).
const GIANT_FACTOR: f32 = 3.0;
const GIANT_MAX: f32 = 300.0;
const FIELD_WIDTH_IOS: f32 = FIELD_WIDTH + 20.0;
const HOME_BAR_WIDTH: f32 = 150.0;
const HOME_BAR_ALPHA: f32 = 0.85;
/// Fatias do "degradê de vidro": cada uma é uma cópia do texto recortada por altura; o alfa
/// acumulado vai de `GLASS_BOTTOM` (embaixo) a ~0,96 (em cima) em degraus de ~4 %, sem listras.
const GLASS_SLICES: usize = 9;
const GLASS_BOTTOM: f32 = 0.55;
const GLASS_TOP: f32 = 0.96;
/// Fração da altura coberta pela primeira e pela última fatia (o glifo ocupa ~0,17 a ~0,88).
const GLASS_FIRST_CUT: f32 = 0.90;
const GLASS_LAST_CUT: f32 = 0.25;
/// Véu atrás de `content` (gradiente vertical de `parts::veil`, que garante o contraste); `padding`
/// é a folga entre o conteúdo e o miolo cheio. Sem véu (`color.a` ~ 0) não pinta nada.
fn soft_veil<'a, M: 'a>(content: Element<'a, M>, color: Color, padding: [u16; 2], radius: f32) -> Element<'a, M> {
    parts::veil(widget::container(content).padding(padding).into(), color, radius)
}

/// Texto centrado de uma linha, sem folga de linha (a caixa tem exatamente `size` de altura).
fn clock_text<'a, M: 'a>(s: &str, size: f32, font: Font, color: Color) -> widget::Text<'a, cosmic::Theme> {
    widget::text(s.to_owned())
        .size(size)
        .font(font)
        .line_height(LineHeight::Relative(1.0))
        .class(theme::Text::Color(color))
}

/// Relógio em "degradê de vidro": o iced não pinta texto com gradiente, então empilha cópias do
/// texto recortadas por altura (cada uma mais opaca que a de baixo) sobre uma sombra suave.
fn glass_clock<'a, M: 'a>(s: &str, size: f32, font: Font, color: Color, shadow: Color, alpha: f32) -> Element<'a, M> {
    let mut layers: Vec<Element<'a, M>> = vec![offset(
        clock_text::<M>(s, size, font, fade(Color { a: shadow.a * 0.6, ..shadow }, alpha)),
        Vector::new(0.0, 3.0),
    )
    .into()];
    layers.push(clock_text::<M>(s, size, font, fade(Color { a: GLASS_BOTTOM, ..color }, alpha)).into());
    // (1 - base) * (1 - a)^n = 1 - top  =>  alfa por fatia.
    let step = 1.0 - ((1.0 - GLASS_TOP) / (1.0 - GLASS_BOTTOM)).powf(1.0 / GLASS_SLICES as f32);
    for i in 0..GLASS_SLICES {
        let t = i as f32 / (GLASS_SLICES - 1) as f32;
        let cut = GLASS_FIRST_CUT + (GLASS_LAST_CUT - GLASS_FIRST_CUT) * t;
        let slice = widget::container(clock_text::<M>(s, size, font, fade(Color { a: step, ..color }, alpha)))
            .height(Length::Fixed(size * cut))
            .clip(true);
        layers.push(slice.into());
    }
    iced::widget::Stack::with_children(layers)
        .width(Length::Shrink)
        .height(Length::Fixed(size))
        .into()
}

pub fn view<'a, M: Clone + 'static>(input: ScreenInput<'a, M>) -> Element<'a, M> {
    let ScreenInput { cfg, metrics, clock, user, field, status, looks, power, session, keyboard, system, backdrop } = input;
    let p = palette(cfg, backdrop.map(|a| a.average));
    let lum = |region: Region| backdrop_luminance(backdrop, region, cfg.dim_opacity);
    let veil_color = Color { a: min_alpha_for_contrast(p.text, p.veil, lum(CLOCK_REGION), 0.0), ..p.veil };
    let field_fill = Color {
        a: min_alpha_for_contrast(p.field_text, Color { a: 1.0, ..p.field_fill }, lum(FIELD_REGION), p.field_fill.a),
        ..p.field_fill
    };
    // O nome e a barra de status não têm fundo próprio: ganham o véu do trecho em que ficam.
    let login_veil = Color { a: min_alpha_for_contrast(p.text, p.veil, lum(FIELD_REGION), 0.0), ..p.veil };
    let clock_alpha = clock.look.alpha;
    let scale = cfg.clock_scale();

    // Barra de status: hora à esquerda, rede e bateria à direita.
    let mut icons = vec![network_icon_name(system.network.as_ref())];
    if let Some(battery) = system.glance_battery() {
        icons.push(battery.kind.icon_name(battery.percent, battery.charging));
    }
    let chip = |content: Element<'a, M>| soft_veil(content, veil_color, [12, 16], 20.0);
    let status_bar = widget::row![
        chip(parts::text_line(
            Cow::Owned(format!("{}:{}", clock.hour, clock.minute)),
            15.0,
            style_font(cfg, FontRole::Date),
            p.text,
            p.text_shadow,
            clock_alpha,
            Horizontal::Left,
        )),
        widget::space::horizontal().width(Length::Fill),
        chip(parts::icon_row(icons, p.text, 16, clock_alpha)),
    ]
    .align_y(Alignment::Center);

    // Data e relógio gigante, cada um com o seu véu (só existe quando o fundo pede).
    let mut clock_col = widget::column![].align_x(Alignment::Center);
    if cfg.show_date {
        let date_size = (20.0 * (0.8 + 0.2 * scale)).clamp(16.0, 28.0);
        let date = parts::text_line(
            Cow::Owned(clock.date),
            date_size,
            style_font(cfg, FontRole::Date),
            p.text,
            p.text_shadow,
            clock_alpha,
            Horizontal::Center,
        );
        clock_col = clock_col.push(soft_veil(date, veil_color, [24, 40], 24.0));
    }
    // Com segundos o texto fica mais largo: encolhe para caber na tela.
    let width_fit = if clock.time.chars().count() > 5 { 0.72 } else { 1.0 };
    let giant = (metrics.time_size * GIANT_FACTOR * scale * width_fit).clamp(64.0, GIANT_MAX);
    let glass = glass_clock::<M>(&clock.time, giant, style_font(cfg, FontRole::Clock), p.text, p.text_shadow, clock_alpha);
    clock_col = clock_col.push(soft_veil(glass, veil_color, [36, 36], 32.0));
    let clock_block = offset(widget::container(clock_col).width(Length::Fill).align_x(Alignment::Center), shift(clock.look));

    // Bloco de login: avatar, nome, campo em pílula de vidro e status.
    let mut login = widget::column![].width(Length::Fixed(FIELD_WIDTH_IOS)).align_x(Alignment::Center);
    if cfg.show_avatar {
        login = login.push(crate::login_ui::avatar(user.icon, (metrics.avatar * 0.55).max(48.0), looks.avatar));
    }
    if cfg.show_username {
        let name = parts::text_line(
            Cow::Borrowed(user.full_name),
            22.0,
            style_font(cfg, FontRole::Name),
            p.text,
            p.text_shadow,
            looks.name.alpha,
            Horizontal::Center,
        );
        login = login.push(offset(widget::container(name).padding([14, 0, 10, 0]), shift(looks.name)));
    }
    if let Some(parts_field) = field {
        let colors = FieldColors {
            fill: field_fill,
            text: p.field_text,
            placeholder: p.field_placeholder,
            border: p.field_border,
            accent: p.accent,
        };
        login = login.push(parts::field(parts_field, colors, FieldShape::Pill { lock_icon: false }, FIELD_WIDTH_IOS));
    }
    login = login.push(widget::container(parts::status(status, looks.status, p.text, p.text_shadow)).padding([10, 0, 0, 0]));
    let login = parts::card(login.into(), cfg, p.card_fill, p.glass_border, looks.field.alpha);
    let login = soft_veil(login, login_veil, [24, 48], 32.0);

    // Cantos de baixo: sessão/teclado à esquerda, energia à direita, e a barrinha inicial.
    let circle = Some(ButtonShape::Circle);
    let mut left = widget::row![].spacing(12);
    if let Some(menu) = session {
        left = left.push(parts::choice(menu, p.button_fill, p.button_icon, circle, "video-display-symbolic", looks.power));
    }
    if let Some(menu) = keyboard {
        left = left.push(parts::choice(menu, p.button_fill, p.button_icon, circle, "input-keyboard-symbolic", looks.power));
    }
    let mut corners = widget::row![left, widget::space::horizontal().width(Length::Fill)].align_y(Alignment::End);
    if let Some(power) = power {
        corners = corners.push(parts::power(power, p.button_fill, p.button_icon, circle));
    }
    let home = widget::container(parts::home_bar(p.text, HOME_BAR_WIDTH, HOME_BAR_ALPHA))
        .width(Length::Fill)
        .align_x(Alignment::Center);

    widget::container(
        widget::column![
            status_bar,
            widget::space::vertical().height(Length::Fixed(12.0)),
            clock_block,
            widget::space::vertical().height(Length::FillPortion(2)),
            widget::container(login).width(Length::Fill).align_x(Alignment::Center),
            widget::space::vertical().height(Length::FillPortion(1)),
            corners,
            widget::space::vertical().height(Length::Fixed(14.0)),
            home,
        ]
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .padding([14, 28, 12, 28])
    .width(Length::Fill)
    .height(Length::Fill)
    .class(theme::Container::Transparent)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backdrop::Analysis;
    use crate::styles::test_support::{cfg, init_i18n, input, system};

    #[test]
    fn regions_are_normalized() {
        for r in [CLOCK_REGION, FIELD_REGION] {
            assert!(0.0 <= r.x0 && r.x0 < r.x1 && r.x1 <= 1.0, "{r:?}");
            assert!(0.0 <= r.y0 && r.y0 < r.y1 && r.y1 <= 1.0, "{r:?}");
        }
    }

    #[test]
    fn view_builds_for_both_themes() {
        init_i18n();
        let analysis = Analysis::uniform(0.9, [1.0, 1.0, 1.0]);
        let sys = system();
        for theme in ["dark", "light"] {
            let config = cfg("ios", theme);
            let _element = view(input(&config, &sys, Some(&analysis), false));
        }
    }
}
