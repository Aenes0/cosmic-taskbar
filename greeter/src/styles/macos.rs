// SPDX-License-Identifier: GPL-3.0-only

//! Estilo `macos` (Tahoe): data pequena acima de um relógio enorme, arredondado e translúcido;
//! na base, avatar pequeno, nome, campo em pílula de vidro e a dica de desbloqueio. Rede e
//! bateria ficam mínimas no canto superior direito e a energia, discreta, no inferior direito.

use std::borrow::Cow;

use cosmic::iced::alignment::Horizontal;
use cosmic::iced::{self, Alignment, Color, Length};
use cosmic::widget;
use cosmic::{Element, theme};

use super::palette::{FontRole, Palette, backdrop_luminance, min_alpha_for_contrast, palette, style_font};
use super::parts::{ButtonShape, FieldColors, FieldShape, card, choice, field, icon_row, power, status, text_line, veil};
use super::ScreenInput;
use crate::backdrop::{Analysis, Region};
use crate::fl;
use crate::login_ui::{EDGE_PADDING, avatar, fade, shift};
use crate::offset::offset;
use crate::sysinfo::{DeviceKind, SystemInfo, network_icon_name};

/// Onde o relógio e a data ficam (fração da tela), para medir o fundo atrás deles.
pub const CLOCK_REGION: Region = Region { x0: 0.20, y0: 0.06, x1: 0.80, y1: 0.42 };
/// Onde o bloco de login (avatar, nome, campo e dica) fica, para medir o fundo atrás dele.
pub const FIELD_REGION: Region = Region { x0: 0.30, y0: 0.66, x1: 0.70, y1: 0.92 };

/// O relógio enorme é decorativo: translúcido, só um pouco abaixo do opaco.
const CLOCK_OPACITY: f32 = 0.90;
/// Tamanho do relógio em múltiplos da hora dos outros estilos; com segundos encolhe para caber.
const CLOCK_GROWTH: f32 = 2.4;
const SECONDS_SHRINK: f32 = 0.7;
const CLOCK_MIN_SIZE: f32 = 48.0;
const CLOCK_MAX_SIZE: f32 = 320.0;
const DATE_BASE_SIZE: f32 = 17.0;
/// Avatar do macOS: 40/62 do avatar do Windows 11 no protótipo, nunca menor que 40 px.
const AVATAR_RATIO: f32 = 40.0 / 62.0;
const AVATAR_MIN_SIZE: f32 = 40.0;
const NAME_SIZE: f32 = 15.0;
const HINT_SIZE: f32 = 12.0;
const FIELD_WIDTH: f32 = 240.0;
const STATUS_ICON_SIZE: u16 = 16;
const LOGIN_SPACING: f32 = 6.0;
/// Folga do véu da data: o texto fica no trecho sólido do gradiente (30%-70% da altura) e o resto
/// é fade longo, para o véu não parecer uma barra.
const DATE_VEIL_TOP: u16 = 56;
const DATE_VEIL_BOTTOM: u16 = 40;
/// O relógio gigante conta como texto grande (WCAG pede 3:1, não 4.5:1): véu mais leve que o da data.
const CLOCK_VEIL_SHARE: f32 = 0.6;
/// Véu atrás do bloco de login: folga acima e abaixo para o nome, o campo e a dica ficarem no
/// trecho sólido do gradiente.
const LOGIN_VEIL_TOP: u16 = 120;
const LOGIN_VEIL_BOTTOM: u16 = 60;

/// Véu atrás da data: o mínimo que deixa o texto da paleta com contraste 4.5:1 sobre o fundo.
fn clock_veil(p: &Palette, backdrop: Option<&Analysis>, dim: f32) -> Color {
    let lum = backdrop_luminance(backdrop, CLOCK_REGION, dim);
    Color { a: min_alpha_for_contrast(p.text, p.veil, lum, 0.0), ..p.veil }
}

/// Véu atrás do nome e da dica: mesmo cálculo, sobre a região do bloco de login.
fn login_veil(p: &Palette, backdrop: Option<&Analysis>, dim: f32) -> Color {
    let lum = backdrop_luminance(backdrop, FIELD_REGION, dim);
    Color { a: min_alpha_for_contrast(p.text, p.veil, lum, 0.0), ..p.veil }
}

/// Preenchimento do campo: o da paleta, elevado só até o texto atingir 4.5:1 sobre o fundo.
fn field_fill(p: &Palette, backdrop: Option<&Analysis>, dim: f32) -> Color {
    let lum = backdrop_luminance(backdrop, FIELD_REGION, dim);
    Color { a: min_alpha_for_contrast(p.field_text, p.field_fill, lum, p.field_fill.a), ..p.field_fill }
}

/// Rede e, se houver, a bateria do sistema (periférico não entra na barra).
fn status_icons(system: &SystemInfo) -> Vec<&'static str> {
    let mut icons = vec![network_icon_name(system.network.as_ref())];
    if let Some(b) = system.batteries.iter().find(|b| b.kind == DeviceKind::System) {
        icons.push(b.kind.icon_name(b.percent, b.charging));
    }
    icons
}

pub fn view<'a, M: Clone + 'static>(input: ScreenInput<'a, M>) -> Element<'a, M> {
    let ScreenInput { cfg, metrics, clock, user, field: pill, status: status_line, looks, power: power_parts, session, system, backdrop, .. } =
        input;
    let p = palette(cfg, backdrop.map(|a| a.average));
    let veil_color = clock_veil(&p, backdrop, cfg.dim_opacity);
    let scale = cfg.clock_scale();
    let seconds = if cfg.show_seconds { SECONDS_SHRINK } else { 1.0 };
    let time_size = (metrics.time_size * CLOCK_GROWTH * scale * seconds).clamp(CLOCK_MIN_SIZE, CLOCK_MAX_SIZE);
    let date_size = DATE_BASE_SIZE * (0.8 + 0.2 * scale);
    let clock_alpha = clock.look.alpha;
    let veil_faded = fade(veil_color, clock_alpha);

    // Data (com véu próprio e justo, para o contraste valer no texto pequeno) e relógio enorme.
    let mut clock_col = widget::column![].align_x(Alignment::Center);
    if cfg.show_date {
        let date = text_line(
            Cow::Owned(clock.date),
            date_size,
            style_font(cfg, FontRole::Date),
            p.text,
            p.text_shadow,
            clock_alpha,
            Horizontal::Center,
        );
        let band = widget::container(date).width(Length::Fill).align_x(Alignment::Center).padding([DATE_VEIL_TOP, 0, DATE_VEIL_BOTTOM, 0]);
        clock_col = clock_col.push(veil(band.into(), veil_faded, 0.0));
    }
    let time = text_line(
        Cow::Owned(clock.time),
        time_size,
        style_font(cfg, FontRole::Clock),
        Color { a: p.text.a * CLOCK_OPACITY, ..p.text },
        p.text_shadow,
        clock_alpha,
        Horizontal::Center,
    );
    let time_band = widget::container(time)
        .width(Length::Fill)
        .align_x(Alignment::Center)
        .padding([(time_size * 0.2) as u16, 0]);
    let time_veil = Color { a: veil_faded.a * CLOCK_VEIL_SHARE, ..veil_faded };
    clock_col = clock_col.push(veil(time_band.into(), time_veil, 0.0));
    let top = offset(
        widget::container(clock_col)
            .width(Length::Fill)
            .align_x(Alignment::Center),
        shift(clock.look),
    );

    // Base: avatar, nome, campo em pílula de vidro, dica, status e sessão.
    let mut login = widget::column![].align_x(Alignment::Center).spacing(LOGIN_SPACING);
    if cfg.show_avatar {
        login = login.push(avatar(user.icon, (metrics.avatar * AVATAR_RATIO).max(AVATAR_MIN_SIZE), looks.avatar));
    }
    if cfg.show_username {
        let name = text_line(
            Cow::Borrowed(user.full_name),
            NAME_SIZE,
            style_font(cfg, FontRole::Name),
            p.text,
            p.text_shadow,
            looks.name.alpha,
            Horizontal::Center,
        );
        login = login.push(offset(widget::container(name).padding([2, 0]), shift(looks.name)));
    }
    if let Some(parts) = pill {
        let colors = FieldColors {
            fill: field_fill(&p, backdrop, cfg.dim_opacity),
            text: p.field_text,
            placeholder: p.field_placeholder,
            border: p.field_border,
            accent: p.accent,
        };
        login = login.push(field(parts, colors, FieldShape::Pill { lock_icon: false }, FIELD_WIDTH));
        let hint = text_line(
            Cow::Owned(fl!("unlock-hint")),
            HINT_SIZE,
            style_font(cfg, FontRole::Body),
            p.text_muted,
            p.text_shadow,
            looks.field.alpha,
            Horizontal::Center,
        );
        login = login.push(offset(hint, shift(looks.field)));
    }
    login = login.push(status(status_line, looks.status, p.text, p.text_shadow));
    if let Some(menu) = session {
        login = login.push(choice(menu, p.button_fill, p.text_muted, None, "", looks.field));
    }
    let login = card(login.into(), cfg, p.card_fill, p.glass_border, looks.field.alpha);
    let login_band = widget::container(login)
        .width(Length::Fill)
        .align_x(Alignment::Center)
        .padding([LOGIN_VEIL_TOP, 0, LOGIN_VEIL_BOTTOM, 0]);
    let login_veil = fade(login_veil(&p, backdrop, cfg.dim_opacity), looks.field.alpha);
    let bottom_layer = widget::container(veil(login_band.into(), login_veil, 0.0))
        .width(Length::Fill)
        .height(Length::Fill)
        .align_y(Alignment::End);

    // Cantos: rede e bateria no topo, energia discreta embaixo.
    let icons = icon_row(status_icons(system), p.text, STATUS_ICON_SIZE, 0.9 * clock_alpha);
    let top_bar = widget::container(icons)
        .width(Length::Fill)
        .align_x(Alignment::End)
        .padding([10.0, 16.0]);
    let power_layer = widget::container(
        power_parts.map(|parts| power(parts, p.button_fill, p.button_icon, Some(ButtonShape::Circle))),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Alignment::End)
    .align_y(Alignment::End)
    .padding(EDGE_PADDING);

    widget::container(
        iced::widget::stack![top_bar, top, bottom_layer, power_layer]
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .class(theme::Container::Transparent)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::styles::test_support::{TestMsg, cfg, init_i18n, input, system};

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
        let sys = system();
        let analysis = Analysis::uniform(0.9, [1.0, 1.0, 1.0]);
        for theme in ["dark", "light"] {
            for with_field in [false, true] {
                let config = cfg("macos", theme);
                let _: Element<'_, TestMsg> = view(input(&config, &sys, Some(&analysis), with_field));
            }
        }
    }

    #[test]
    fn veil_and_field_meet_contrast_on_any_backdrop() {
        use crate::styles::palette::{MIN_CONTRAST, composite_luminance, contrast_ratio, relative_luminance};
        for theme in ["dark", "light"] {
            let p = palette(&cfg("macos", theme), None);
            for dim in [0.0_f32, 0.6, 0.85] {
                for step in 0..=20 {
                    let lum = step as f32 * 0.05;
                    let analysis = Analysis::uniform(lum, [lum; 3]);
                    let backdrop = Some(&analysis);
                    // O que o usuário vê sob o texto é o fundo já escurecido pelo overlay do dim.
                    let seen = composite_luminance(Color::from_rgba(0.0, 0.0, 0.0, dim), lum);
                    let veil_c = clock_veil(&p, backdrop, dim);
                    let date = contrast_ratio(relative_luminance(p.text), composite_luminance(veil_c, seen));
                    assert!(date >= MIN_CONTRAST - 0.01, "{theme} dim {dim} lum {lum}: data {date}");
                    let login_c = login_veil(&p, backdrop, dim);
                    let name = contrast_ratio(relative_luminance(p.text), composite_luminance(login_c, seen));
                    assert!(name >= MIN_CONTRAST - 0.01, "{theme} dim {dim} lum {lum}: nome {name}");
                    let fill = field_fill(&p, backdrop, dim);
                    assert!(fill.a >= p.field_fill.a, "{theme} lum {lum}: preenchimento abaixo da base");
                    let text = contrast_ratio(relative_luminance(p.field_text), composite_luminance(fill, seen));
                    assert!(text >= MIN_CONTRAST - 0.01, "{theme} dim {dim} lum {lum}: campo {text}");
                }
            }
        }
    }
}
