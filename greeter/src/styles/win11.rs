// SPDX-License-Identifier: GPL-3.0-only

//! Estilo `win11`: relógio Semibold centrado no topo com a data abaixo; no centro, avatar
//! redondo, nome, campo retangular de raio 4 com a linha de destaque e a seta, e "Opções de
//! entrada" (sessão) logo abaixo; no canto inferior direito, ícones de rede e bateria e o botão
//! de energia. O texto do relógio e o preenchimento do campo ganham o mínimo de véu necessário
//! para o contraste WCAG AA sobre a região do papel de parede em que ficam.

use std::borrow::Cow;

use cosmic::iced::alignment::Horizontal;
use cosmic::iced::{self, Alignment, Color, Length};
use cosmic::widget;
use cosmic::Element;

use super::palette::{FontRole, Palette, backdrop_luminance, min_alpha_for_contrast, palette, style_font};
use super::parts::{
    FieldColors, FieldShape, card, choice, field, icon_row, power, status, text_line, veil,
};
use super::{ChoiceMenu, ScreenInput};
use crate::backdrop::Region;
use crate::fl;
use crate::login_ui::{
    DATE_SIZE, EDGE_PADDING, FIELD_WIDTH, TIME_MIN_SIZE, avatar, fade, screen, shift,
};
use crate::motion::Look;
use crate::offset::offset;
use crate::sysinfo::{DeviceKind, SystemInfo, network_icon_name};

/// Onde ficam a hora e a data (normalizado): topo, centralizado.
pub const CLOCK_REGION: Region = Region { x0: 0.30, y0: 0.08, x1: 0.70, y1: 0.30 };
/// Onde ficam o nome e o campo de senha: centro da tela, um pouco abaixo do meio.
pub const FIELD_REGION: Region = Region { x0: 0.35, y0: 0.40, x1: 0.65, y1: 0.64 };

const TIME_MAX_SIZE: f32 = 156.0;
const ICON_SIZE: u16 = 18;
/// O véu é um gradiente vertical com o platô nos 40 % do meio; o relógio fica dentro do platô
/// (folga vertical de 75 % da altura do conteúdo de cada lado), senão a data cairia na parte que
/// já desvanece e o contraste calculado não valeria.
const VEIL_PAD_FACTOR: f32 = 0.75;
/// Folga vertical do véu do bloco de login (fração da altura do bloco) e teto dela em px.
const LOGIN_VEIL_PAD_FACTOR: f32 = 0.5;
const LOGIN_VEIL_PAD_MAX: f32 = 200.0;
/// O véu do login desce um pouco: o nome, o campo, o status e "Opções de entrada" ficam na parte baixa do bloco.
const LOGIN_VEIL_SHIFT: f32 = 30.0;
/// Estimativa da altura de uma linha de texto (como `login_ui::LINE_HEIGHT_ESTIMATE`).
const LINE_HEIGHT: f32 = 1.4;
/// Alturas estimadas (px) da linha de status, do campo e do botão "Opções de entrada".
const STATUS_HEIGHT: f32 = 30.0;
const FIELD_HEIGHT: f32 = 34.0;
const SESSION_HEIGHT: f32 = 30.0;
/// Largura da coluna de login (campo e linha de status); o cartão acompanha.
const LOGIN_WIDTH: f32 = FIELD_WIDTH + 20.0;

/// Ícones do canto inferior direito: rede e, se houver, a bateria do sistema.
fn status_icons(system: &SystemInfo) -> Vec<&'static str> {
    let mut icons = vec![network_icon_name(system.network.as_ref())];
    if let Some(b) = system.batteries.iter().find(|b| b.kind == DeviceKind::System) {
        icons.push(b.kind.icon_name(b.percent, b.charging));
    }
    icons
}

/// Faixa de véu de `height` px, em toda a largura da tela, só pintada (sem conteúdo); desce `dy` px
/// além do deslocamento da animação `look`. O véu só desvanece na vertical, então a faixa não pode
/// ficar dentro das margens da tela, senão as laterais mostrariam uma borda dura.
fn veil_strip<'a, M: 'a>(color: Color, height: f32, dy: f32, look: Look) -> Element<'a, M> {
    let strip = widget::container(widget::space::vertical().height(Length::Fixed(height))).width(Length::Fill);
    let by = iced::Vector::new(look.dx, look.dy + dy);
    offset(veil(strip.into(), fade(color, look.alpha), 0.0), by).into()
}

/// Relógio: hora Semibold e data. Devolve também a altura estimada do bloco, para dimensionar o véu.
fn clock_block<'a, M: 'a>(input: &ScreenInput<'a, M>, p: &Palette) -> (Element<'a, M>, f32) {
    let cfg = input.cfg;
    let alpha = input.clock.look.alpha;
    let scale = cfg.clock_scale();
    let time_size = (input.metrics.time_size * scale).clamp(TIME_MIN_SIZE, TIME_MAX_SIZE);
    let date_size = (DATE_SIZE * (0.75 + 0.25 * scale)).clamp(14.0, 30.0);
    let mut column = widget::column![text_line(
        Cow::Owned(input.clock.time.clone()),
        time_size,
        style_font(cfg, FontRole::Clock),
        p.text,
        p.text_shadow,
        alpha,
        Horizontal::Center,
    )]
    .align_x(Alignment::Center);
    if cfg.show_date {
        column = column.push(text_line(
            Cow::Owned(input.clock.date.clone()),
            date_size,
            style_font(cfg, FontRole::Date),
            p.text,
            p.text_shadow,
            alpha,
            Horizontal::Center,
        ));
    }
    let content = time_size * LINE_HEIGHT + if cfg.show_date { date_size * LINE_HEIGHT } else { 0.0 };
    let top = widget::container(column)
        .width(Length::Fill)
        .align_x(Alignment::Center)
        .padding([input.metrics.clock_top, 0.0, 0.0, 0.0]);
    (offset(top, shift(input.clock.look)).into(), content)
}

pub fn view<'a, M: Clone + 'static>(input: ScreenInput<'a, M>) -> Element<'a, M> {
    let cfg = input.cfg;
    let p = palette(cfg, input.backdrop.map(|a| a.average));
    let clock_lum = backdrop_luminance(input.backdrop, CLOCK_REGION, cfg.dim_opacity);
    let field_lum = backdrop_luminance(input.backdrop, FIELD_REGION, cfg.dim_opacity);
    let veil_color = Color { a: min_alpha_for_contrast(p.text, p.veil, clock_lum, 0.0), ..p.veil };
    let fill_alpha = min_alpha_for_contrast(p.field_text, p.field_fill, field_lum, p.field_fill.a);
    let colors = FieldColors {
        fill: Color { a: fill_alpha, ..p.field_fill },
        text: p.field_text,
        placeholder: p.field_placeholder,
        border: p.field_border,
        accent: p.accent,
    };

    let name_veil = Color { a: min_alpha_for_contrast(p.text, p.veil, field_lum, 0.0), ..p.veil };
    let (top, clock_height) = clock_block(&input, &p);

    let ScreenInput {
        cfg,
        metrics,
        user,
        field: field_parts,
        status: status_line,
        looks,
        power: power_parts,
        session,
        keyboard,
        system,
        ..
    } = input;

    let mut login = widget::column![].align_x(Alignment::Center).width(Length::Fixed(LOGIN_WIDTH));
    // Altura estimada do bloco, para dimensionar o véu.
    let mut height = STATUS_HEIGHT;
    if cfg.show_avatar {
        height += metrics.avatar;
        login = login.push(avatar(user.icon, metrics.avatar, looks.avatar));
    }
    if cfg.show_username {
        let size = (metrics.avatar * 0.2).clamp(16.0, 28.0);
        height += size * LINE_HEIGHT + 20.0;
        let name = widget::container(text_line(
            Cow::Borrowed(user.full_name),
            size,
            style_font(cfg, FontRole::Name),
            p.text,
            p.text_shadow,
            looks.name.alpha,
            Horizontal::Center,
        ))
        .padding([10, 0]);
        login = login.push(offset(name, shift(looks.name)));
    }
    if let Some(parts) = field_parts {
        height += FIELD_HEIGHT;
        login = login.push(field(parts, colors, FieldShape::Win11, FIELD_WIDTH));
    }
    login = login.push(status(status_line, looks.status, p.text, p.text_shadow));
    if let Some(menu) = session {
        height += SESSION_HEIGHT;
        let menu = ChoiceMenu { label: fl!("sign-in-options"), ..menu };
        login = login.push(choice(menu, p.card_fill, p.text, None, "", looks.field));
    }
    let card = card(login.into(), cfg, p.card_fill, p.glass_border, looks.field.alpha);
    let login_band = height + 2.0 * (height * LOGIN_VEIL_PAD_FACTOR).min(LOGIN_VEIL_PAD_MAX);
    let login_veil = widget::container(veil_strip(name_veil, login_band, LOGIN_VEIL_SHIFT, looks.field))
        .width(Length::Fill)
        .height(Length::Fill)
        .align_y(Alignment::Center);
    let clock_pad = clock_height * VEIL_PAD_FACTOR;
    let clock_veil = veil_strip(
        veil_color,
        clock_height + 2.0 * clock_pad,
        EDGE_PADDING + metrics.clock_top - clock_pad,
        looks.clock,
    );

    let mut corner = widget::row![].spacing(14.0).align_y(Alignment::Center);
    if let Some(menu) = keyboard {
        corner = corner.push(choice(menu, p.card_fill, p.text, None, "", looks.power));
    }
    corner = corner.push(icon_row(status_icons(system), p.text, ICON_SIZE, looks.power.alpha));
    if let Some(parts) = power_parts {
        corner = corner.push(power(parts, p.button_fill, p.button_icon, None));
    }
    let content = screen(top, card, Some(corner.into()));
    iced::widget::stack![login_veil, clock_veil, content].width(Length::Fill).height(Length::Fill).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backdrop::Analysis;
    use crate::styles::test_support::{TestMsg, cfg, init_i18n, input, system};

    fn normalized(r: Region) -> bool {
        0.0 <= r.x0 && r.x0 < r.x1 && r.x1 <= 1.0 && 0.0 <= r.y0 && r.y0 < r.y1 && r.y1 <= 1.0
    }

    #[test]
    fn regions_are_normalized() {
        assert!(normalized(CLOCK_REGION), "{CLOCK_REGION:?}");
        assert!(normalized(FIELD_REGION), "{FIELD_REGION:?}");
    }

    #[test]
    fn view_builds_for_both_themes() {
        init_i18n();
        let analysis = Analysis::uniform(0.9, [1.0, 1.0, 1.0]);
        let sys = system();
        for theme in ["dark", "light"] {
            let config = cfg("win11", theme);
            for with_field in [false, true] {
                let _element: Element<'_, TestMsg> = view(input(&config, &sys, Some(&analysis), with_field));
            }
            let _no_analysis: Element<'_, TestMsg> = view(input(&config, &sys, None, true));
        }
    }
}
