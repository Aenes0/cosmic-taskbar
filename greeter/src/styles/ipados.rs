// SPDX-License-Identifier: GPL-3.0-only

//! Estilo `ipados` (iPadOS 26, antigo `sidebar`): data e relógio grandes no canto superior
//! esquerdo, coluna de widgets de vidro com dados reais (um por periférico com bateria e um de
//! rede) logo abaixo, campo de vidro em pílula centrado na base e a barra de início. A direita
//! do papel de parede fica livre.

use std::borrow::Cow;

use cosmic::iced::alignment::Horizontal;
use cosmic::iced::{Alignment, Background, Border, Color, Length};
use cosmic::widget;
use cosmic::{Element, theme};

use super::palette::{
    FontRole, MIN_CONTRAST, Palette, backdrop_luminance, contrast_ratio, min_alpha_for_contrast, palette,
    relative_luminance, style_font,
};
use super::parts::{self, ButtonShape, FieldColors, FieldShape};
use super::ScreenInput;
use crate::backdrop::Region;
use crate::fl;
use crate::login_ui::{self, fade, shift};
use crate::offset::offset;
use crate::sysinfo::{DeviceKind, SystemInfo};

/// Onde ficam a data e o relógio (canto superior esquerdo), em fração da tela.
pub const CLOCK_REGION: Region = Region { x0: 0.03, y0: 0.04, x1: 0.40, y1: 0.40 };
/// Onde ficam o nome, o campo e o status (centro da base).
pub const FIELD_REGION: Region = Region { x0: 0.36, y0: 0.70, x1: 0.64, y1: 0.92 };
/// Onde ficam os widgets (coluna sob o relógio).
const TILES_REGION: Region = Region { x0: 0.04, y0: 0.40, x1: 0.38, y1: 0.72 };

const LEFT_MARGIN: f32 = 24.0;
const TILES_INSET: f32 = 32.0;
const CLOCK_PAD: [f32; 2] = [8.0, 8.0];
const CLOCK_VEIL_RADIUS: f32 = 24.0;
const CLOCK_SCALE: f32 = 1.7;
const TILE_WIDTH: f32 = 320.0;
const TILE_GAP: f32 = 8.0;
const MAX_TILES: usize = 5;
const FIELD_WIDTH: f32 = 260.0;
const VEIL_LAYERS: usize = 14;
const VEIL_STEP: f32 = 3.0;
const AVATAR_SIZE: f32 = 56.0;
const NAME_SIZE: f32 = 15.0;
const HOME_BAR_WIDTH: f32 = 134.0;
const STATUS_ICON_SIZE: u16 = 16;
const BUTTON_MIN_ALPHA: f32 = 0.45;

/// Dados de um widget: valor em destaque, rótulo e ícone.
#[derive(Clone, Debug, PartialEq, Eq)]
struct TileSpec {
    value: String,
    label: String,
    icon: &'static str,
}

/// Texto traduzido de uma chave Fluent só conhecida em tempo de execução.
fn translate(key: &str) -> String {
    crate::localize::LANGUAGE_LOADER.get().map_or_else(|| key.to_owned(), |loader| loader.get(key))
}

/// Os widgets na ordem da tela: periféricos (ordem estável do `SystemInfo`) e a rede por último.
/// Sem dado não há widget.
fn tile_specs(system: &SystemInfo) -> Vec<TileSpec> {
    let mut specs: Vec<TileSpec> = system
        .peripherals()
        .take(MAX_TILES)
        .map(|device| {
            let key = device.kind.label_key();
            let generic = device.kind == DeviceKind::Other || key == "battery-device";
            let label = if generic && !device.name.is_empty() {
                device.name.clone()
            } else {
                translate(key)
            };
            TileSpec {
                value: format!("{} %", device.percent),
                label,
                icon: device.kind.icon_name(device.percent, device.charging),
            }
        })
        .collect();
    if let Some(network) = system.network.as_ref().filter(|n| !n.name.is_empty()) {
        specs.push(TileSpec {
            value: network.name.clone(),
            label: fl!("network"),
            icon: network.icon_name(),
        });
    }
    specs
}

/// Ícones da barra de status: rede e a bateria do sistema (periféricos já têm widget).
fn status_icons(system: &SystemInfo) -> Vec<&'static str> {
    let mut icons = Vec::new();
    if let Some(network) = &system.network {
        icons.push(network.icon_name());
    }
    if let Some(battery) = system.batteries.iter().find(|b| b.kind == DeviceKind::System) {
        icons.push(battery.kind.icon_name(battery.percent, battery.charging));
    }
    icons
}

/// Véu suave: camadas arredondadas concêntricas, cada uma com uma parte do alpha, de modo que o
/// miolo (onde está o conteúdo) recebe `veil.a` inteiro e a borda esmaece sem formar caixa. A
/// árvore é sempre a mesma; sem véu as camadas só não pintam nada.
fn soft_veil<'a, M: 'a>(content: Element<'a, M>, veil: Color, inner_pad: [f32; 2], radius: f32) -> Element<'a, M> {
    let visible = parts::veil_needed(veil);
    let per_layer = 1.0 - (1.0 - veil.a.clamp(0.0, 1.0)).powf(1.0 / VEIL_LAYERS as f32);
    let color = Color { a: per_layer, ..veil };
    let mut element = content;
    for layer in 0..VEIL_LAYERS {
        let pad = if layer == 0 { inner_pad } else { [VEIL_STEP, VEIL_STEP] };
        let radius = radius + VEIL_STEP * layer as f32;
        element = widget::container(element)
            .padding([pad[0], pad[1]])
            .class(theme::Container::custom(move |_| widget::container::Style {
                background: visible.then_some(Background::Color(color)),
                border: Border { radius: radius.into(), ..Default::default() },
                ..Default::default()
            }))
            .into();
    }
    element
}

/// Tinta (e sombra) do tema, ou a oposta quando o tema não alcança [`MIN_CONTRAST`] sobre o fundo.
fn readable_ink(p: &Palette, bg_lum: f32) -> (Color, Color) {
    let ink_lum = relative_luminance(p.text);
    if contrast_ratio(ink_lum, bg_lum) >= MIN_CONTRAST {
        return (p.text, p.text_shadow);
    }
    // Branco e preto puros: o melhor dos dois passa de 4,5:1 em qualquer luminância.
    if contrast_ratio(1.0, bg_lum) >= contrast_ratio(0.0, bg_lum) {
        (Color::WHITE, Color::from_rgba(0.0, 0.0, 0.0, 0.35))
    } else {
        (Color::BLACK, Color::from_rgba(1.0, 1.0, 1.0, 0.45))
    }
}

pub fn view<'a, M: Clone + 'static>(input: ScreenInput<'a, M>) -> Element<'a, M> {
    let ScreenInput { cfg, metrics, clock, user, field, status, looks, power, session, keyboard, system, backdrop } = input;
    let p = palette(cfg, backdrop.map(|a| a.average));
    let lum = |region: Region| backdrop_luminance(backdrop, region, cfg.dim_opacity);
    let clock_look = clock.look;

    // Data e relógio, com véu só se o fundo pedir.
    let veil_alpha = min_alpha_for_contrast(p.text, p.veil, lum(CLOCK_REGION), 0.0);
    let time_size = (metrics.time_size * CLOCK_SCALE * cfg.clock_scale()).clamp(48.0, 168.0);
    let mut clock_col = widget::column![].align_x(Alignment::Start);
    if cfg.show_date {
        clock_col = clock_col.push(parts::text_line(
            Cow::Owned(clock.date),
            (time_size * 0.17).clamp(14.0, 24.0),
            style_font(cfg, FontRole::Date),
            p.text,
            p.text_shadow,
            clock_look.alpha,
            Horizontal::Left,
        ));
    }
    clock_col = clock_col.push(parts::text_line(
        Cow::Owned(clock.time),
        time_size,
        style_font(cfg, FontRole::Clock),
        p.text,
        p.text_shadow,
        clock_look.alpha,
        Horizontal::Left,
    ));
    let clock_block = soft_veil(
        clock_col.into(),
        Color { a: veil_alpha, ..p.veil },
        CLOCK_PAD,
        CLOCK_VEIL_RADIUS,
    );

    // Widgets de vidro; escuros o bastante para o texto do tema ler sobre o fundo medido.
    let tile_alpha = min_alpha_for_contrast(p.text, p.field_fill, lum(TILES_REGION), p.field_fill.a);
    let tile_fill = Color { a: tile_alpha, ..p.field_fill };
    let mut tiles = widget::column![].spacing(TILE_GAP).width(Length::Fixed(TILE_WIDTH));
    for spec in tile_specs(system) {
        tiles = tiles.push(parts::tile(spec.value, spec.label, spec.icon, tile_fill, p.glass_border, p.text, clock_look.alpha));
    }
    let tiles = offset(
        widget::container(tiles).padding([0.0, 0.0, 0.0, TILES_INSET]),
        shift(clock_look),
    );

    // Barra de status: sempre na árvore (vazia, sem altura, quando não há dados), para que os
    // dados de rede/bateria chegando não mudem a árvore de widgets e o campo não perca o foco.
    let icons = status_icons(system);
    let bar_padding = if icons.is_empty() { [0, 0] } else { [10, 16] };
    let mut root = widget::column![].width(Length::Fill).height(Length::Fill);
    root = root.push(
        widget::container(parts::icon_row(icons, p.text, STATUS_ICON_SIZE, looks.status.alpha))
            .width(Length::Fill)
            .align_x(Alignment::End)
            .padding(bar_padding),
    );
    root = root
        .push(widget::space::vertical().height(Length::Fixed(metrics.clock_top * 0.1)))
        .push(widget::container(offset(clock_block, shift(clock_look))))
        .push(widget::container(tiles).padding([0.0, 0.0, 0.0, LEFT_MARGIN]))
        .push(widget::space::vertical().height(Length::Fill));

    // Login centrado na base: avatar, nome, campo de vidro e status.
    let field_fill_alpha = min_alpha_for_contrast(p.field_text, p.field_fill, lum(FIELD_REGION), p.field_fill.a);
    // Nome e status não têm vidro: se o fundo tira o contraste do texto do tema, usam a tinta
    // oposta (uma das duas sempre passa de 4,5:1), sem caixa de véu.
    let (ink, ink_shadow) = readable_ink(&p, lum(FIELD_REGION));
    let mut login = widget::column![].align_x(Alignment::Center).spacing(8);
    if cfg.show_avatar {
        login = login.push(login_ui::avatar(user.icon, AVATAR_SIZE.min(metrics.avatar), looks.avatar));
    }
    if cfg.show_username {
        login = login.push(parts::text_line(
            Cow::Borrowed(user.full_name),
            NAME_SIZE,
            style_font(cfg, FontRole::Name),
            ink,
            ink_shadow,
            looks.name.alpha,
            Horizontal::Center,
        ));
    }
    if let Some(parts) = field {
        let colors = FieldColors {
            fill: Color { a: field_fill_alpha, ..p.field_fill },
            text: p.field_text,
            placeholder: p.field_placeholder,
            border: p.field_border,
            accent: p.accent,
        };
        login = login.push(parts::field(parts, colors, FieldShape::Pill { lock_icon: false }, FIELD_WIDTH));
    }
    login = login.push(widget::container(parts::status(status, looks.status, ink, ink_shadow)).width(Length::Fixed(FIELD_WIDTH)));
    let login = parts::card(login.into(), cfg, p.card_fill, p.glass_border, looks.field.alpha);
    root = root.push(widget::container(login).width(Length::Fill).align_x(Alignment::Center));

    // Base: sessão/teclado à esquerda, barra de início ao centro, energia à direita.
    let button_fill = Color { a: tile_alpha.max(BUTTON_MIN_ALPHA), ..p.field_fill };
    let mut left = widget::row![].spacing(8).align_y(Alignment::End);
    if let Some(menu) = session {
        left = left.push(parts::choice(menu, button_fill, p.button_icon, Some(ButtonShape::Circle), "system-users-symbolic", looks.power));
    }
    if let Some(menu) = keyboard {
        left = left.push(parts::choice(menu, button_fill, p.button_icon, Some(ButtonShape::Circle), "input-keyboard-symbolic", looks.power));
    }
    let mut right = widget::row![].align_y(Alignment::End);
    if let Some(parts_power) = power {
        right = right.push(parts::power(parts_power, button_fill, p.button_icon, Some(ButtonShape::Circle)));
    }
    let bar = parts::home_bar(fade(ink, 0.85), HOME_BAR_WIDTH, looks.power.alpha);
    let bottom = widget::row![
        widget::container(left).width(Length::Fill).align_x(Alignment::Start),
        widget::container(bar).padding([0, 0, 8, 0]),
        widget::container(right).width(Length::Fill).align_x(Alignment::End),
    ]
    .align_y(Alignment::End)
    .padding([0, 24, 12, 24]);
    root = root.push(bottom);

    widget::container(root)
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
    use crate::sysinfo::SystemInfo;

    fn normalized(r: Region) -> bool {
        0.0 <= r.x0 && r.x0 < r.x1 && r.x1 <= 1.0 && 0.0 <= r.y0 && r.y0 < r.y1 && r.y1 <= 1.0
    }

    #[test]
    fn regions_are_normalized() {
        assert!(normalized(CLOCK_REGION));
        assert!(normalized(FIELD_REGION));
    }

    #[test]
    fn view_builds_for_both_themes() {
        init_i18n();
        let analysis = Analysis::uniform(0.9, [1.0, 1.0, 1.0]);
        let sys = system();
        for theme in ["dark", "light"] {
            let config = cfg("ipados", theme);
            let _element = view(input(&config, &sys, Some(&analysis), false));
        }
    }

    #[test]
    fn readable_ink_always_reaches_min_contrast() {
        for theme in ["dark", "light"] {
            let p = palette(&cfg("ipados", theme), None);
            for step in 0..=20 {
                let bg = step as f32 * 0.05;
                let (ink, _) = readable_ink(&p, bg);
                assert!(contrast_ratio(relative_luminance(ink), bg) >= MIN_CONTRAST, "{theme} {bg}");
            }
        }
    }

    /// Forma da árvore de estado de widgets: etiqueta e número de filhos de cada nó, em profundidade.
    fn tree_shape(tree: &cosmic::iced::advanced::widget::Tree, out: &mut Vec<(cosmic::iced::advanced::widget::tree::Tag, usize)>) {
        out.push((tree.tag, tree.children.len()));
        for child in &tree.children {
            tree_shape(child, out);
        }
    }

    fn shape_of(config: &crate::style_config::StyleConfig, sys: &SystemInfo) -> Vec<(cosmic::iced::advanced::widget::tree::Tag, usize)> {
        let analysis = Analysis::uniform(0.9, [1.0, 1.0, 1.0]);
        let element = view(input(config, sys, Some(&analysis), true));
        let tree = cosmic::iced::advanced::widget::Tree::new(&element);
        let mut out = Vec::new();
        tree_shape(&tree, &mut out);
        out
    }

    #[test]
    fn status_bar_is_always_in_the_tree() {
        init_i18n();
        let config = cfg("ipados", "dark");
        let no_data = SystemInfo::default();
        let mut with_system_battery = system();
        with_system_battery.batteries.push(crate::sysinfo::BatteryDevice {
            kind: DeviceKind::System,
            name: "Bateria".into(),
            percent: 55,
            charging: false,
        });
        assert!(status_icons(&no_data).is_empty());
        let before = shape_of(&config, &no_data);
        for (name, sys) in [("rede", system()), ("rede e bateria", with_system_battery)] {
            let after = shape_of(&config, &sys);
            // A coluna raiz tem o mesmo número de filhos com e sem dados: a barra de status está
            // sempre nela (vazia sem dados), então o campo mantém o lugar na árvore e o foco.
            assert_eq!(before[0], after[0], "{name}: a barra de status saiu/entrou na árvore");
        }
    }

    #[test]
    fn empty_system_has_no_tiles() {
        init_i18n();
        assert!(tile_specs(&SystemInfo::default()).is_empty());
        let specs = tile_specs(&system());
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0].value, "87 %");
        assert_eq!(specs[1].value, "Casa");
    }
}
