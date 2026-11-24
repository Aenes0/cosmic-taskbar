// SPDX-License-Identifier: GPL-3.0-only

//! Estilo `android` (Android 16 / Pixel, Material 3 Expressive): barra de status mínima, linha
//! "num relance" (data + bateria), relógio gigante em duas linhas (HH sobre MM), campo em pílula
//! tonal com cadeado e atalhos quadrados-arredondados nos cantos (sessão/teclado à esquerda,
//! energia à direita). Ignora `show_seconds`; nunca pede o tick de 1 s.

use std::borrow::Cow;

use cosmic::iced::alignment::Horizontal;
use cosmic::iced::{Alignment, Background, Color, Length, Radians, gradient};
use cosmic::{Element, theme, widget};

use super::palette::{
    FontRole, Palette, backdrop_luminance, composite_luminance, min_alpha_for_contrast, palette, style_font,
};
use super::parts::{
    ButtonShape, FieldColors, FieldShape, card, card_spec, choice, field, icon_row, power,
    status, text_line, veil, veil_needed,
};
use super::{ChoiceMenu, ScreenInput};
use crate::backdrop::{Analysis, Region};
use crate::login_ui::{EDGE_PADDING, avatar, shift};
use crate::offset::offset;
use crate::style_config::StyleConfig;
use crate::sysinfo::network_icon_name;

/// Onde fica o relógio gigante (normalizada).
pub const CLOCK_REGION: Region = Region { x0: 0.25, y0: 0.12, x1: 0.75, y1: 0.52 };
/// Barra de status + "num relance": esquerda (hora, data, bateria) e direita (ícones); cada lado
/// tem a própria luminância, e o véu usa a pior.
const STRIP_REGIONS: [Region; 2] = [
    Region { x0: 0.0, y0: 0.0, x1: 0.35, y1: 0.13 },
    Region { x0: 0.65, y0: 0.0, x1: 1.0, y1: 0.13 },
];
/// Onde ficam avatar, nome, campo e status (normalizada).
pub const FIELD_REGION: Region = Region { x0: 0.30, y0: 0.58, x1: 0.70, y1: 0.90 };

const FIELD_WIDTH: f32 = 280.0;
const AVATAR_SIZE: f32 = 56.0;
const STATUS_BAR_SIZE: f32 = 13.0;
const GLANCE_SIZE: f32 = 16.0;
const NAME_SIZE: f32 = 17.0;
/// Hora em Nunito normal; entrelinha ~0,86 como no protótipo (linha do iced = 1,3 × o corpo).
const CLOCK_SIZE_FACTOR: f32 = 1.8;
const CLOCK_OVERLAP: f32 = 0.44;
/// Espaço abaixo da faixa de status: o degradê do véu só começa a sumir depois dele.
const STRIP_BOTTOM: u16 = 36;

fn tone(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

/// Barra de status (hora à esquerda; rede e bateria à direita) e a linha "num relance".
fn top_rows<'a, M: 'a>(input: &ScreenInput<'a, M>, p: &Palette) -> Element<'a, M> {
    let cfg = input.cfg;
    let alpha = input.clock.look.alpha;
    let body = style_font(cfg, FontRole::Body);
    let date_font = style_font(cfg, FontRole::Date);
    let glance = input.system.glance_battery();

    let mut icons = vec![network_icon_name(input.system.network.as_ref())];
    if let Some(b) = glance {
        icons.push(b.kind.icon_name(b.percent, b.charging));
    }
    let status = widget::row![
        text_line(
            Cow::Owned(format!("{}:{}", input.clock.hour, input.clock.minute)),
            STATUS_BAR_SIZE, body, p.text, p.text_shadow, alpha, Horizontal::Left,
        ),
        widget::space::horizontal().width(Length::Fill),
        icon_row(icons, p.text, 16, alpha),
    ]
    .align_y(Alignment::Center);

    let mut glance_row = widget::row![].spacing(8).align_y(Alignment::Center);
    if cfg.show_date {
        glance_row = glance_row.push(text_line(
            Cow::Owned(input.clock.short_date.clone()),
            GLANCE_SIZE, date_font, p.text, p.text_shadow, alpha, Horizontal::Left,
        ));
    }
    if let Some(b) = glance {
        if cfg.show_date {
            glance_row = glance_row.push(text_line(
                Cow::Borrowed("·"), GLANCE_SIZE, date_font, p.text, p.text_shadow, alpha, Horizontal::Left,
            ));
        }
        glance_row = glance_row
            .push(icon_row(vec![b.kind.icon_name(b.percent, b.charging)], p.text, 16, alpha))
            .push(text_line(
                Cow::Owned(format!("{} %", b.percent)),
                GLANCE_SIZE, date_font, p.text, p.text_shadow, alpha, Horizontal::Left,
            ));
    }
    widget::container(widget::column![status, widget::container(glance_row).padding([14, 0, 0, 0])])
        .padding([EDGE_PADDING, EDGE_PADDING, f32::from(STRIP_BOTTOM), EDGE_PADDING])
        .into()
}

/// Relógio gigante: HH sobre MM, centrado.
fn clock_block<'a, M: 'a>(input: &ScreenInput<'a, M>, p: &Palette) -> Element<'a, M> {
    let cfg = input.cfg;
    let size = (input.metrics.time_size * CLOCK_SIZE_FACTOR * cfg.clock_scale()).clamp(56.0, 230.0);
    let font = style_font(cfg, FontRole::Clock);
    let alpha = input.clock.look.alpha;
    let line = |s: &str| {
        text_line(Cow::Owned(s.to_owned()), size, font, p.text, p.text_shadow, alpha, Horizontal::Center)
    };
    widget::container(
        widget::column![line(&input.clock.hour), line(&input.clock.minute)]
            .spacing(-size * CLOCK_OVERLAP)
            .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .align_x(Alignment::Center)
    .padding([(input.metrics.clock_top * 0.3).max(4.0), 0.0, 0.0, 0.0])
    .into()
}

/// Degradê de contraste preso à borda superior e em largura total: opaco (`color`) no alto e
/// some para baixo, sem caixa nem cantos. Sem alfa não pinta nada (a árvore não muda).
fn top_fade<'a, M: 'a>(content: Element<'a, M>, color: Color) -> Element<'a, M> {
    let background = veil_needed(color).then(|| {
        Background::Gradient(
            gradient::Linear::new(Radians(std::f32::consts::PI))
                .add_stop(0.0, color)
                .add_stop(0.4, color)
                .add_stop(1.0, tone(color, 0.0))
                .into(),
        )
    });
    widget::container(content)
        .width(Length::Fill)
        .class(theme::Container::custom(move |_| widget::container::Style { background, ..Default::default() }))
        .into()
}

/// Alpha do véu do bloco de login. Parte do fundo como o usuário o vê (com o `dim_opacity`) e,
/// com cartão, já composto com ele: o cartão e o véu têm a mesma cor (preto no tema escuro, branco
/// no claro), então as duas camadas somam em qualquer ordem e o véu só cobre o que o cartão não
/// deu. Sem cartão é o véu inteiro, como nos outros estilos.
fn login_veil_alpha(cfg: &StyleConfig, p: &Palette, backdrop: Option<&Analysis>) -> f32 {
    let mut lum = backdrop_luminance(backdrop, FIELD_REGION, cfg.dim_opacity);
    if let Some((opacity, _)) = card_spec(cfg) {
        lum = composite_luminance(tone(p.card_fill, p.card_fill.a * opacity), lum);
    }
    min_alpha_for_contrast(p.text, p.veil, lum, 0.0)
}

pub fn view<'a, M: Clone + 'static>(input: ScreenInput<'a, M>) -> Element<'a, M> {
    let cfg = input.cfg;
    let p = palette(cfg, input.backdrop.map(|a| a.average));
    let luminance = |region| backdrop_luminance(input.backdrop, region, cfg.dim_opacity);
    let clock_veil = min_alpha_for_contrast(p.text, p.veil, luminance(CLOCK_REGION), 0.0);
    let field_veil = login_veil_alpha(cfg, &p, input.backdrop);
    let field_fill = min_alpha_for_contrast(p.field_text, p.field_fill, luminance(FIELD_REGION), p.field_fill.a);

    let strip_veil = STRIP_REGIONS
        .into_iter()
        .map(|r| min_alpha_for_contrast(p.text, p.veil, luminance(r), 0.0))
        .fold(0.0, f32::max);
    let top_content = widget::column![
        top_fade(top_rows(&input, &p), tone(p.veil, strip_veil)),
        veil(clock_block(&input, &p), tone(p.veil, clock_veil), 0.0),
    ];
    let top = offset(top_content, shift(input.clock.look));

    let ScreenInput { user, field: field_parts, status: status_line, looks, power: power_parts, session, keyboard, .. } = input;
    let mut login = widget::column![].align_x(Alignment::Center);
    if cfg.show_avatar {
        login = login.push(avatar(user.icon, AVATAR_SIZE, looks.avatar));
    }
    if cfg.show_username {
        let name = text_line(
            Cow::Borrowed(user.full_name),
            NAME_SIZE, style_font(cfg, FontRole::Name), p.text, p.text_shadow, looks.name.alpha, Horizontal::Center,
        );
        login = login.push(offset(widget::container(name).padding([8, 0, 10, 0]), shift(looks.name)));
    }
    if let Some(parts) = field_parts {
        let colors = FieldColors {
            fill: tone(p.field_fill, field_fill),
            text: p.field_text,
            placeholder: p.field_placeholder,
            border: p.field_border,
            accent: p.accent,
        };
        login = login.push(field(parts, colors, FieldShape::Pill { lock_icon: true }, FIELD_WIDTH));
    }
    login = login.push(
        widget::container(status(status_line, looks.status, p.text, p.text_shadow)).padding([10, 0, 0, 0]),
    );
    let login = card(login.into(), cfg, p.card_fill, p.field_border, looks.field.alpha);
    // Véu em largura total (sem caixa): o degradê só desvanece no sentido vertical.
    let login = veil(
        widget::container(login).width(Length::Fill).align_x(Alignment::Center).into(),
        tone(p.veil, field_veil),
        0.0,
    );

    let corner = |menu: ChoiceMenu<M>, icon: &'static str| {
        choice(menu, p.button_fill, p.button_icon, Some(ButtonShape::Squircle), icon, looks.power)
    };
    let mut left = widget::row![].spacing(10).align_y(Alignment::End);
    if let Some(menu) = session {
        left = left.push(corner(menu, "preferences-desktop-display-symbolic"));
    }
    if let Some(menu) = keyboard {
        left = left.push(corner(menu, "input-keyboard-symbolic"));
    }
    let mut bottom = widget::row![left, widget::space::horizontal().width(Length::Fill)].align_y(Alignment::End);
    if let Some(parts) = power_parts {
        bottom = bottom.push(power(parts, p.button_fill, p.button_icon, Some(ButtonShape::Squircle)));
    }

    let lower = widget::column![
        login,
        widget::space::vertical().height(Length::Fixed(20.0)),
        widget::container(bottom).padding([0.0, EDGE_PADDING, EDGE_PADDING, EDGE_PADDING]),
    ];
    let column = widget::column![top, widget::space::vertical().height(Length::Fill), lower]
    .width(Length::Fill)
    .height(Length::Fill);
    widget::container(column)
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
    fn login_text_readable_over_card_on_bright_backdrop() {
        use crate::styles::palette::{MIN_CONTRAST, composite_luminance, contrast_ratio, relative_luminance};
        let p = palette(&cfg("android", "dark"), None);
        let analysis = Analysis::uniform(0.9, [0.9; 3]);
        for opacity in [0.0_f32, 0.05, 0.3, 0.95] {
            for dim in [0.0_f32, 0.4] {
                let mut config = cfg("android", "dark");
                config.card_opacity = opacity;
                config.dim_opacity = dim;
                let veil_a = login_veil_alpha(&config, &p, Some(&analysis));
                // Fundo escurecido pelo dim, depois o véu, depois o cartão (ordem de pintura).
                let seen = composite_luminance(Color::from_rgba(0.0, 0.0, 0.0, dim), 0.9);
                let mut under = composite_luminance(tone(p.veil, veil_a), seen);
                if let Some((o, _)) = card_spec(&config) {
                    under = composite_luminance(tone(p.card_fill, p.card_fill.a * o), under);
                }
                let c = contrast_ratio(relative_luminance(p.text), under);
                assert!(c >= MIN_CONTRAST - 0.01, "card {opacity} dim {dim}: nome/status {c}");
                assert!(veil_a > 0.0, "card {opacity} dim {dim}: fundo claro exige véu");
            }
        }
    }

    #[test]
    fn view_builds_for_both_themes() {
        init_i18n();
        let analysis = Analysis::uniform(0.9, [1.0, 1.0, 1.0]);
        let sys = system();
        for theme in ["dark", "light"] {
            let config = cfg("android", theme);
            let _ = view(input(&config, &sys, Some(&analysis), false));
            let _ = view(input(&config, &sys, None, true));
        }
    }
}
