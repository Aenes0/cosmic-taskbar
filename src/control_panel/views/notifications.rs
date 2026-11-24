// SPDX-License-Identifier: GPL-3.0-only

//! Notificações: prévia ao vivo, comportamento, aparência e cores dos cartões.
use std::sync::OnceLock;

use cosmic::{
    Element,
    iced::{self, Alignment, Background, Color, Length},
    theme,
    widget,
};

use super::legacy::dimmed;
use super::row::{self, BtnStyle, Tone};
use super::shell::action_button;
use crate::central::{
    notifications::config::{self as notif_cfg, NotifFormat, NotifPosition, TIMEOUT_OPTIONS},
    tiles::icon,
    widgets::card,
};
use crate::control_panel::{ControlPanel, Message};

/// Altura do palco da prévia.
const STAGE_HEIGHT: f32 = 185.0;

const ACCENT_PRESETS: [(&str, &str); 6] = [
    ("#3584E4", "Azul"),
    ("#2EC27E", "Verde"),
    ("#00BCD4", "Ciano"),
    ("#FF7800", "Laranja"),
    ("#9141AC", "Roxo"),
    ("#E01B24", "Vermelho"),
];
const BG_PRESETS: [(&str, &str); 5] = [
    ("#111318", "Preto"),
    ("#1E2028", "Grafite"),
    ("#141C2B", "Azul noite"),
    ("#221C2A", "Roxo escuro"),
    ("#F3F4F6", "Claro"),
];
const SYSTEM_THEME: &str = "Tema do sistema";

fn timeout_labels() -> &'static [String] {
    static L: OnceLock<Vec<String>> = OnceLock::new();
    L.get_or_init(|| {
        TIMEOUT_OPTIONS
            .iter()
            .map(|s| if *s == 0 { "Persistente".to_owned() } else { format!("{s} s") })
            .collect()
    })
}

fn position_labels() -> &'static [String] {
    static L: OnceLock<Vec<String>> = OnceLock::new();
    L.get_or_init(|| NotifPosition::ALL.iter().map(|p| p.label().to_owned()).collect())
}

fn lines_labels() -> &'static [String] {
    static L: OnceLock<Vec<String>> = OnceLock::new();
    L.get_or_init(|| {
        (notif_cfg::MIN_BODY_LINES..=notif_cfg::MAX_BODY_LINES)
            .map(|n| n.to_string())
            .collect()
    })
}

fn notif_live_preview<'a>(cfg: &'a notif_cfg::NotifConfig) -> Element<'a, Message> {
    let opacity = cfg.opacity.clamp(0.40, 1.00);
    let radius = cfg.corner_radius.clamp(4.0, 24.0);
    let bg_rgb = cfg.bg_rgb();
    let accent_rgb = cfg.accent_rgb();
    let pad = match cfg.format {
        NotifFormat::Minimal => [6, 10],
        NotifFormat::Compact => [8, 12],
        NotifFormat::Full => [12, 14],
    };

    let mut card_rows: Vec<Element<'a, Message>> = vec![
        widget::row::with_children(vec![
            widget::icon::icon(icon("preferences-system-notifications-symbolic"))
                .size(if cfg.format == NotifFormat::Minimal { 14 } else { 18 })
                .into(),
            widget::text::caption("cosmic-taskbar").width(Length::Fill).into(),
            widget::text::caption(if cfg.timeout_secs == 0 {
                "persistente".to_owned()
            } else {
                format!("{}s", cfg.timeout_secs)
            })
            .class(theme::Text::Custom(dimmed))
            .into(),
        ])
        .spacing(theme::spacing().space_xxs)
        .align_y(Alignment::Center)
        .into(),
        widget::text::body("Nova mensagem recebida").into(),
    ];

    if cfg.show_body && cfg.format != NotifFormat::Minimal {
        let sample = if cfg.max_body_lines <= 1 {
            "Prévia ao vivo do estilo da notificação."
        } else {
            "Prévia ao vivo: cor, opacidade, bordas e posição atualizam instantaneamente."
        };
        card_rows.push(
            widget::text::caption(sample)
                .class(theme::Text::Custom(dimmed))
                .into(),
        );
    }

    if cfg.show_actions && cfg.format == NotifFormat::Full {
        let mk_btn = |lbl: &'static str| {
            widget::container(widget::text::caption(lbl))
                .padding([3, 10])
                .class(theme::Container::custom(move |theme| {
                    let c = match accent_rgb {
                        Some([r, g, b]) => Color::from_rgb(r, g, b),
                        None => theme.cosmic().accent_color().into(),
                    };
                    let mut bg = c;
                    bg.a = 0.22;
                    widget::container::Style {
                        background: Some(Background::Color(bg)),
                        text_color: Some(c),
                        border: iced::Border {
                            radius: 8.0.into(),
                            width: 1.0,
                            color: c,
                        },
                        ..Default::default()
                    }
                }))
                .into()
        };
        card_rows.push(widget::row::with_children(vec![mk_btn("Abrir"), mk_btn("Dispensar")]).spacing(theme::spacing().space_xxs).into());
    }

    let toast_card = widget::container(widget::column::with_children(card_rows).spacing(theme::spacing().space_xxxs))
        .padding(pad)
        .width(Length::Fixed(if cfg.format == NotifFormat::Compact { 245.0 } else { 280.0 }))
        .class(theme::Container::custom(move |theme| {
            let cosmic = theme.cosmic();
            let c = card(cosmic);
            let bg = match bg_rgb {
                Some([r, g, b]) => Color::from_rgba(r, g, b, opacity),
                None => {
                    let mut base: Color = c.base.into();
                    base.a = opacity;
                    base
                }
            };
            let mut border_color: Color = match accent_rgb {
                Some([r, g, b]) => Color::from_rgb(r, g, b),
                None => cosmic.accent_color().into(),
            };
            border_color.a = 0.80;
            widget::container::Style {
                background: Some(Background::Color(bg)),
                text_color: Some(c.on.into()),
                icon_color: Some(border_color),
                border: iced::Border {
                    radius: radius.into(),
                    width: 1.25,
                    color: border_color,
                },
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
                    offset: iced::Vector::new(0.0, 5.0),
                    blur_radius: 14.0,
                },
                ..Default::default()
            }
        }));

    let (align_x, align_y) = match cfg.position {
        NotifPosition::TopRight => (iced::alignment::Horizontal::Right, iced::alignment::Vertical::Top),
        NotifPosition::TopCenter => (iced::alignment::Horizontal::Center, iced::alignment::Vertical::Top),
        NotifPosition::BottomRight => (iced::alignment::Horizontal::Right, iced::alignment::Vertical::Bottom),
        NotifPosition::BottomLeft => (iced::alignment::Horizontal::Left, iced::alignment::Vertical::Bottom),
    };

    let stage = widget::container(toast_card)
        .width(Length::Fill)
        .height(Length::Fixed(STAGE_HEIGHT))
        .padding(14)
        .align_x(align_x)
        .align_y(align_y)
        .class(theme::Container::custom(|theme| {
            let cosmic = theme.cosmic();
            widget::container::Style {
                background: Some(Background::Color(cosmic.bg_color().into())),
                border: iced::Border {
                    radius: cosmic.radius_s().into(),
                    width: 1.0,
                    color: cosmic.background(theme.transparent).divider.into(),
                },
                ..Default::default()
            }
        }));

    let style = format!(
        "{} · {} · {:.0}%",
        cfg.position.label(),
        cfg.format.label(),
        cfg.opacity * 100.0
    );
    row::group(
        "Prévia",
        Some("A prévia muda junto com os ajustes."),
        false,
        vec![
            widget::container(stage)
                .padding(theme::spacing().space_s)
                .into(),
            row::kv("Estilo atual", style),
        ],
    )
}

fn preset_items(
    presets: &[(&'static str, &'static str)],
    system: Color,
) -> Vec<(&'static str, Color)> {
    let mut items = vec![(SYSTEM_THEME, system)];
    for (hex, name) in presets {
        let c = notif_cfg::parse_hex_color(hex)
            .map(|[r, g, b]| Color::from_rgb(r, g, b))
            .unwrap_or(system);
        items.push((*name, c));
    }
    items
}

/// Índice (com o "Tema do sistema" em 0) do preset igual ao hex atual.
fn preset_selected(presets: &[(&str, &str)], current: &str) -> Option<usize> {
    if current.trim().is_empty() {
        return Some(0);
    }
    presets
        .iter()
        .position(|(hex, _)| current.eq_ignore_ascii_case(hex))
        .map(|i| i + 1)
}

fn preset_hex(presets: &[(&'static str, &'static str)], i: usize) -> String {
    if i == 0 {
        String::new()
    } else {
        presets[i - 1].0.to_owned()
    }
}

pub(super) fn notifications_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let cfg = &app.notif_config;

    let timeout_sel = TIMEOUT_OPTIONS.iter().position(|s| *s == cfg.timeout_secs);
    let format_names: Vec<&str> = NotifFormat::ALL.iter().map(|f| f.label()).collect();
    let format_sel = NotifFormat::ALL.iter().position(|f| *f == cfg.format).unwrap_or(0);
    let position_sel = NotifPosition::ALL.iter().position(|p| *p == cfg.position);

    let behavior = row::group(
        "Comportamento",
        Some("A posição e o estilo também valem para o OSD de dispositivos."),
        false,
        vec![
            row::select("Tempo na tela", None, timeout_labels(), timeout_sel, |i| {
                Message::NotifTimeout(TIMEOUT_OPTIONS[i])
            }),
            row::segmented("Formato", None, &format_names, format_sel, |i| {
                Message::NotifFormat(NotifFormat::ALL[i])
            }),
            row::select("Posição", None, position_labels(), position_sel, |i| {
                Message::NotifPosition(NotifPosition::ALL[i])
            }),
        ],
    );

    let lines_sel = (cfg.max_body_lines >= notif_cfg::MIN_BODY_LINES
        && cfg.max_body_lines <= notif_cfg::MAX_BODY_LINES)
        .then(|| (cfg.max_body_lines - notif_cfg::MIN_BODY_LINES) as usize);
    let appearance = row::group(
        "Aparência",
        None,
        false,
        vec![
            row::slider(
                "Opacidade",
                None,
                40.0..=100.0,
                1.0,
                (cfg.opacity * 100.0).round(),
                format!("{:.0}%", cfg.opacity * 100.0),
                ("40%".to_owned(), "100%".to_owned()),
                |v| Message::NotifOpacity(v / 100.0),
                None,
            ),
            row::slider(
                "Raio da borda",
                None,
                notif_cfg::MIN_CORNER_RADIUS..=notif_cfg::MAX_CORNER_RADIUS,
                1.0,
                cfg.corner_radius,
                format!("{:.0} px", cfg.corner_radius),
                (
                    format!("{:.0} px", notif_cfg::MIN_CORNER_RADIUS),
                    format!("{:.0} px", notif_cfg::MAX_CORNER_RADIUS),
                ),
                Message::NotifCornerRadius,
                None,
            ),
            row::select("Linhas do corpo", None, lines_labels(), lines_sel, |i| {
                Message::NotifMaxBodyLines(notif_cfg::MIN_BODY_LINES + i as u32)
            }),
            row::toggle("Mostrar corpo da mensagem", None, cfg.show_body, |_| {
                Message::NotifToggleShowBody
            }),
            row::toggle("Mostrar botões de ação", None, cfg.show_actions, |_| {
                Message::NotifToggleShowActions
            }),
            row::toggle("Agrupar por aplicativo", None, cfg.group_by_app, |_| {
                Message::NotifToggleGroupByApp
            }),
        ],
    );

    let active = theme::active();
    let cosmic = active.cosmic();
    let accent_items = preset_items(&ACCENT_PRESETS, cosmic.accent_color().into());
    let bg_items = preset_items(&BG_PRESETS, card(cosmic).base.into());
    let colors = row::group(
        "Cores",
        None,
        false,
        vec![
            row::swatches(
                "Destaque",
                Some("Ícone e botões."),
                &accent_items,
                preset_selected(&ACCENT_PRESETS, &cfg.accent_hex),
                |i| Message::NotifAccentHex(preset_hex(&ACCENT_PRESETS, i)),
            ),
            row::swatches(
                "Fundo dos cartões",
                Some("Atrás do texto."),
                &bg_items,
                preset_selected(&BG_PRESETS, &cfg.bg_hex),
                |i| Message::NotifBgHex(preset_hex(&BG_PRESETS, i)),
            ),
            row::input(
                "Cor personalizada de destaque",
                Some("#RRGGBB. Vazio usa o tema do sistema."),
                "#RRGGBB",
                &cfg.accent_hex,
                Message::NotifAccentHex,
                Some(("Usar tema", Message::NotifAccentHex(String::new()))),
            ),
            row::input(
                "Cor personalizada de fundo",
                Some("#RRGGBB. Vazio usa o tema do sistema."),
                "#RRGGBB",
                &cfg.bg_hex,
                Message::NotifBgHex,
                Some(("Usar tema", Message::NotifBgHex(String::new()))),
            ),
        ],
    );

    widget::column::with_children(vec![notif_live_preview(cfg), behavior, appearance, colors])
        .spacing(spacing.space_m)
        .into()
}

/// Botões do cabeçalho da página.
pub(super) fn notifications_actions(app: &ControlPanel) -> Vec<Element<'_, Message>> {
    let enabled = !app.busy;
    vec![
        action_button("Restaurar padrão", Message::NotifReset, BtnStyle::Normal, enabled),
        action_button("Enviar teste", Message::NotifSendTest, BtnStyle::Primary, enabled),
    ]
}

/// Selo ao lado do título: sem selo por enquanto.
pub(super) fn notifications_badge(_app: &ControlPanel) -> Option<(String, Tone)> {
    None
}
