use cosmic::{
    Element,
    iced::{self, Alignment, Background, Color, Length},
    theme,
    widget::{self},
};
use crate::central::{
    notifications::config::{parse_hex_color},
    tiles::{icon},
};
use crate::control_panel::{
    ControlPanel,
    Message,
    Section,
    data,
    power_config::{PowerProfileKind, ProfileTuning},
};
use super::legacy::*;

/// Nome do estilo no Painel (id desconhecido cai em Windows 11, como a leitura da config).
fn style_label(id: &str) -> &'static str {
    let canon = data::canonical_greeter_style(id).unwrap_or("win11");
    data::GREETER_STYLES
        .iter()
        .find(|(sid, _, _)| *sid == canon)
        .map(|(_, name, _)| *name)
        .unwrap_or("Windows 11")
}

fn preview_accent(theme: &cosmic::Theme, accent_rgb: Option<[f32; 3]>) -> Color {
    match accent_rgb {
        Some([r, g, b]) => Color::from_rgb(r, g, b),
        None => theme.cosmic().accent_color().into(),
    }
}

fn preview_avatar<'a>(accent_rgb: Option<[f32; 3]>) -> Element<'a, Message> {
    widget::container(widget::icon::icon(icon("avatar-default-symbolic")).size(22))
        .width(Length::Fixed(40.0))
        .height(Length::Fixed(40.0))
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .class(theme::Container::custom(move |theme| {
            let acc = preview_accent(theme, accent_rgb);
            let mut bg = acc;
            bg.a = 0.25;
            widget::container::Style {
                background: Some(Background::Color(bg)),
                icon_color: Some(acc),
                border: iced::Border { radius: 20.0.into(), width: 1.5, color: acc },
                ..Default::default()
            }
        }))
        .into()
}

fn preview_field<'a>(
    accent_rgb: Option<[f32; 3]>,
    is_light: bool,
    sub_col: Color,
    radius: f32,
    width: f32,
) -> Element<'a, Message> {
    widget::container(
        widget::row::with_children(vec![
            widget::text::caption("••••••••••").width(Length::Fill).into(),
            widget::icon::icon(icon("go-next-symbolic")).size(14).into(),
        ])
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([6, 10])
    .width(Length::Fixed(width))
    .class(theme::Container::custom(move |theme| {
        let acc = preview_accent(theme, accent_rgb);
        widget::container::Style {
            background: Some(Background::Color(if is_light {
                Color::from_rgba(1.0, 1.0, 1.0, 0.72)
            } else {
                Color::from_rgba(0.05, 0.06, 0.09, 0.65)
            })),
            text_color: Some(sub_col),
            icon_color: Some(acc),
            border: iced::Border { radius: radius.into(), width: 1.0, color: acc },
            ..Default::default()
        }
    }))
    .into()
}

/// Botão redondo de vidro dos cantos do iOS (lanterna / câmera).
fn preview_round_button<'a>(icon_name: &'static str, is_light: bool) -> Element<'a, Message> {
    widget::container(widget::icon::icon(icon(icon_name)).size(14))
        .width(Length::Fixed(34.0))
        .height(Length::Fixed(34.0))
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .class(theme::Container::custom(move |_| widget::container::Style {
            background: Some(Background::Color(if is_light {
                Color::from_rgba(1.0, 1.0, 1.0, 0.55)
            } else {
                Color::from_rgba(0.05, 0.06, 0.09, 0.55)
            })),
            border: iced::Border {
                radius: 17.0.into(),
                width: 1.0,
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.18),
            },
            ..Default::default()
        }))
        .into()
}

/// Atalho quadrado-arredondado dos cantos do Android.
fn preview_shortcut<'a>(is_light: bool) -> Element<'a, Message> {
    widget::container(widget::space::horizontal().width(Length::Fixed(30.0)))
        .height(Length::Fixed(30.0))
        .class(theme::Container::custom(move |_| widget::container::Style {
            background: Some(Background::Color(if is_light {
                Color::from_rgba(1.0, 1.0, 1.0, 0.6)
            } else {
                Color::from_rgba(0.16, 0.10, 0.24, 0.65)
            })),
            border: iced::Border { radius: 10.0.into(), ..Default::default() },
            ..Default::default()
        }))
        .into()
}

/// Barrinha de início do iOS e do iPadOS.
fn preview_home_bar<'a>(col: Color, width: f32) -> Element<'a, Message> {
    widget::container(widget::space::horizontal().width(Length::Fixed(width)))
        .height(Length::Fixed(3.0))
        .class(theme::Container::custom(move |_| widget::container::Style {
            background: Some(Background::Color(Color { a: 0.7, ..col })),
            border: iced::Border { radius: 2.0.into(), ..Default::default() },
            ..Default::default()
        }))
        .into()
}

/// Widget de vidro de exemplo da coluna do iPadOS.
fn preview_widget_chip<'a>(label: &'a str, is_light: bool, text: Color) -> Element<'a, Message> {
    widget::container(widget::text::caption(label))
        .padding([5, 10])
        .width(Length::Fixed(120.0))
        .class(theme::Container::custom(move |_| widget::container::Style {
            background: Some(Background::Color(if is_light {
                Color::from_rgba(1.0, 1.0, 1.0, 0.6)
            } else {
                Color::from_rgba(0.08, 0.10, 0.14, 0.6)
            })),
            text_color: Some(text),
            border: iced::Border {
                radius: 12.0.into(),
                width: 1.0,
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.16),
            },
            ..Default::default()
        }))
        .into()
}

fn login_live_preview<'a>(g: &'a data::GreeterConfig) -> Element<'a, Message> {
    let is_light = g.theme == "light";
    let accent_rgb = parse_hex_color(&g.accent_color);
    let dim = g.dim_opacity.clamp(0.0, 0.85);
    let card_op = g.card_opacity.clamp(0.0, 0.95);
    let card_rad = g.card_radius.clamp(4.0, 32.0);
    let soften = (g.blur_radius as f32 / 40.0).min(1.0) * 0.5;
    let text_col = if is_light {
        Color::from_rgb(0.10, 0.12, 0.16)
    } else {
        Color::from_rgb(0.95, 0.96, 0.99)
    };
    let sub_col = if is_light {
        Color::from_rgba(0.10, 0.12, 0.16, 0.72)
    } else {
        Color::from_rgba(0.95, 0.96, 0.99, 0.72)
    };

    let clock_px: f32 = match g.clock_size.as_str() {
        "small" => 26.0,
        "medium" => 34.0,
        "huge" => 48.0,
        _ => 40.0,
    };
    let time_str = match (g.clock_24h, g.show_seconds) {
        (true, true) => "23:54:08",
        (true, false) => "23:54",
        (false, true) => "11:54:08 PM",
        (false, false) => "11:54 PM",
    };
    let user_label = if g.default_user.trim().is_empty() {
        "alexandre"
    } else {
        g.default_user.trim()
    };

    let style = data::canonical_greeter_style(&g.style).unwrap_or("win11");
    let date_str = "segunda-feira, 23 de novembro";
    let big_clock_col = Color { a: if is_light { 0.78 } else { 0.82 }, ..text_col };

    // Relógio de cada estilo: win11 centrado; android em 2 linhas; macOS/iOS com a data
    // pequena sobre o relógio enorme e translúcido; iPadOS alinhado à esquerda.
    let clock_block: Element<'a, Message> = match style {
        "android" => {
            let col: Vec<Element<'a, Message>> = vec![
                widget::text::body("23").size(clock_px * 1.3).line_height(iced::widget::text::LineHeight::Relative(1.0)).into(),
                widget::text::body("54").size(clock_px * 1.3).line_height(iced::widget::text::LineHeight::Relative(1.0)).into(),
            ];
            widget::column::with_children(col)
                .spacing(0)
                .align_x(Alignment::Center)
                .into()
        }
        "macos" | "ios" | "ipados" => {
            let (scale, align) = match style {
                "macos" => (1.5, Alignment::Center),
                "ios" => (1.7, Alignment::Center),
                _ => (1.2, Alignment::Start),
            };
            let mut col: Vec<Element<'a, Message>> = Vec::new();
            if g.show_date {
                col.push(widget::text::caption(date_str).into());
            }
            col.push(
                widget::text::body(time_str)
                    .size(clock_px * scale)
                    .line_height(iced::widget::text::LineHeight::Relative(1.0))
                    .into(),
            );
            widget::container(widget::column::with_children(col).spacing(0).align_x(align))
                .class(theme::Container::custom(move |_| widget::container::Style {
                    text_color: Some(big_clock_col),
                    ..Default::default()
                }))
                .into()
        }
        _ => {
            let mut col: Vec<Element<'a, Message>> =
                vec![widget::text::body(time_str)
                    .size(clock_px)
                    .line_height(iced::widget::text::LineHeight::Relative(1.0))
                    .into()];
            if g.show_date {
                col.push(widget::text::caption(date_str).into());
            }
            widget::column::with_children(col)
                .spacing(2)
                .align_x(Alignment::Center)
                .into()
        }
    };

    // Avatar + nome só entram onde o estilo os mostra (win11 e android).
    let mut login_parts: Vec<Element<'a, Message>> = Vec::new();
    if g.show_avatar && matches!(style, "win11" | "macos") {
        login_parts.push(preview_avatar(accent_rgb));
    }
    if g.show_username && matches!(style, "win11" | "macos") {
        login_parts.push(widget::text::body(user_label).into());
    }
    // win11: retângulo com botão-seta; demais: pílula.
    let field_radius = if style == "win11" { (card_rad * 0.5).clamp(4.0, 14.0) } else { 999.0 };
    let field_width = match style {
        "ipados" => 200.0,
        "ios" => 210.0,
        "android" => 230.0,
        _ => 165.0,
    };
    login_parts.push(preview_field(accent_rgb, is_light, sub_col, field_radius, field_width));
    if style == "macos" {
        login_parts.push(widget::text::caption("Toque para desbloquear").class(theme::Text::Custom(dimmed)).into());
    }

    let login_card: Element<'a, Message> = if style == "win11" {
        widget::container(
            widget::column::with_children(login_parts)
                .spacing(6)
                .align_x(Alignment::Center),
        )
        .padding(if card_op > 0.02 { 14 } else { 6 })
        .class(theme::Container::custom(move |_| {
            let base = if is_light {
                Color::from_rgba(1.0, 1.0, 1.0, card_op)
            } else {
                Color::from_rgba(0.08, 0.10, 0.14, card_op)
            };
            widget::container::Style {
                background: (card_op > 0.02).then_some(Background::Color(base)),
                text_color: Some(text_col),
                border: iced::Border {
                    radius: card_rad.into(),
                    width: if card_op > 0.02 { 1.0 } else { 0.0 },
                    color: Color::from_rgba(1.0, 1.0, 1.0, 0.18),
                },
                ..Default::default()
            }
        }))
        .into()
    } else {
        widget::column::with_children(login_parts)
            .spacing(6)
            .align_x(Alignment::Center)
            .into()
    };

    let centered = |e: Element<'a, Message>| -> Element<'a, Message> {
        widget::container(e)
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .into()
    };

    let inner: Element<'a, Message> = match style {
        // Material 3: "Num relance" (data) no topo à esquerda, relógio em duas linhas,
        // pílula tonal e atalhos quadrados-arredondados nos cantos da base.
        "android" => widget::column::with_children(vec![
            widget::text::caption(if g.show_date { "seg., 23 de nov." } else { "" }).into(),
            centered(clock_block),
            widget::space::vertical().height(Length::Fill).into(),
            centered(login_card),
            widget::row::with_children(vec![
                preview_shortcut(is_light),
                widget::space::horizontal().width(Length::Fill).into(),
                preview_shortcut(is_light),
            ])
            .align_y(Alignment::Center)
            .into(),
        ])
        .spacing(4)
        .height(Length::Fill)
        .into(),
        // Data + relógio enorme no topo; avatar, nome, pílula de vidro e dica na base.
        "macos" => widget::column::with_children(vec![
            centered(clock_block),
            widget::space::vertical().height(Length::Fill).into(),
            centered(login_card),
        ])
        .height(Length::Fill)
        .into(),
        // Barra de status, relógio gigante, pílula de vidro, botões redondos nos cantos
        // (sessão/teclado à esquerda, energia à direita) e barrinha inferior.
        "ios" => widget::column::with_children(vec![
            widget::row::with_children(vec![
                widget::text::caption("23:54").width(Length::Fill).into(),
                widget::text::caption("Rede · Casa  87 %").into(),
            ])
            .into(),
            centered(clock_block),
            widget::space::vertical().height(Length::Fill).into(),
            centered(login_card),
            widget::row::with_children(vec![
                preview_round_button("input-keyboard-symbolic", is_light),
                widget::space::horizontal().width(Length::Fill).into(),
                preview_round_button("system-shutdown-symbolic", is_light),
            ])
            .align_y(Alignment::Center)
            .into(),
            centered(preview_home_bar(text_col, 70.0)),
        ])
        .spacing(4)
        .height(Length::Fill)
        .into(),
        // Data e relógio no canto superior esquerdo, coluna de widgets de vidro abaixo,
        // campo centrado na base, barrinha inferior; papel de parede livre à direita.
        "ipados" => widget::column::with_children(vec![
            widget::container(clock_block)
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Left)
                .into(),
            widget::container(
                widget::column::with_children(vec![
                    preview_widget_chip("87 % Mouse", is_light, sub_col),
                    preview_widget_chip("Rede · Casa", is_light, sub_col),
                ])
                .spacing(4),
            )
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Left)
            .into(),
            widget::space::vertical().height(Length::Fill).into(),
            centered(login_card),
            centered(preview_home_bar(text_col, 90.0)),
        ])
        .spacing(4)
        .height(Length::Fill)
        .into(),
        // win11: relógio e cartão de vidro centrados.
        _ => widget::column::with_children(vec![centered(clock_block), centered(login_card)])
            .spacing(12)
            .align_x(Alignment::Center)
            .into(),
    };

    let wp_badge = match g.wallpaper_mode.as_str() {
        "rotate_on_lock" => "Wallpaper: a cada bloqueio",
        "rotate_interval" => "Wallpaper: rotativo por intervalo",
        _ => "Wallpaper: fixo",
    };

    let stage = widget::container(inner)
        .width(Length::Fill)
        .height(Length::Fixed(245.0))
        .padding(18)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .class(theme::Container::custom(move |_| {
            let (r, gg, b) = if is_light {
                (0.82 * (1.0 - dim), 0.87 * (1.0 - dim), 0.94 * (1.0 - dim))
            } else {
                (0.11 * (1.0 - dim), 0.16 * (1.0 - dim), 0.27 * (1.0 - dim))
            };
            // blur > 0: só suaviza o tom do fundo da prévia (sem blur real).
            let (r, gg, b) = ((r + (r + gg + b) / 3.0 * soften) / (1.0 + soften),
                (gg + (r + gg + b) / 3.0 * soften) / (1.0 + soften),
                (b + (r + gg + b) / 3.0 * soften) / (1.0 + soften));
            widget::container::Style {
                background: Some(Background::Color(Color::from_rgb(r, gg, b))),
                text_color: Some(text_col),
                border: iced::Border {
                    radius: 14.0.into(),
                    width: 1.5,
                    color: Color::from_rgba(1.0, 1.0, 1.0, 0.14),
                },
                ..Default::default()
            }
        }));

    card_box(
        widget::column::with_children(vec![
            widget::row::with_children(vec![
                widget::text::heading("Prévia ao vivo da Tela de Bloqueio")
                    .width(Length::Fill)
                    .into(),
                widget::text::caption(format!(
                    "{} · {} · {}",
                    style_label(&g.style), g.font_family, wp_badge
                ))
                .class(theme::Text::Custom(dimmed))
                .into(),
            ])
            .align_y(Alignment::Center)
            .into(),
            stage.into(),
        ])
        .spacing(8),
    )
}

pub(super) fn login_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let g = &app.greeter;
    let opt = &app.snapshot.optional;
    let cfg_path = data::greeter_config_path();

    let mut items: Vec<Element<'_, Message>> = vec![
        widget::row::with_children(vec![
            widget::text::title3("Tela de Login & Bloqueio")
                .width(Length::Fill)
                .into(),
            action_btn("Testar bloqueio agora", Message::GreeterTestLock, false),
            action_btn("Aplicar no login (boot)", Message::GreeterApply, true),
        ])
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
        login_live_preview(g),
    ];

    // Status & security card
    items.push(card_box(
        widget::column::with_children(vec![
            widget::row::with_children(vec![
                widget::text::heading("Greeter & Lockscreen isolados (aenes-greeter)")
                    .width(Length::Fill)
                    .into(),
                status_badge(
                    opt.greeter_service_enabled,
                    "Serviço boot ativo",
                    if opt.greeter_isolated_installed {
                        "Instalado (inativo no boot)"
                    } else {
                        "Lockscreen em ~/.local/bin"
                    },
                ),
            ])
            .align_y(Alignment::Center)
            .into(),
            kv_row(
                "Configuração ativa (salva automaticamente)",
                cfg_path.to_string_lossy().into_owned(),
            ),
            widget::text::caption(
                "Alterações abaixo valem imediatamente na tela de bloqueio (Win+L / inatividade). Use \"Aplicar no login (boot)\" para espelhar com segurança para /var/lib/aenes-greeter/ (sem expor sua pasta pessoal ao serviço de boot).",
            )
            .class(theme::Text::Custom(dimmed))
            .into(),
        ])
        .spacing(spacing.space_xs),
    ));

    // 5 Layout Styles card
    let style_options: &[(&str, &str, &str)] = &data::GREETER_STYLES;
    let style_chips = widget::row::with_children(
        style_options
            .iter()
            .map(|(id, label, _)| {
                small_chip(
                    *label,
                    data::canonical_greeter_style(&g.style) == Some(*id),
                    Message::GreeterStyle((*id).to_owned()),
                )
            })
            .collect::<Vec<_>>(),
    )
    .spacing(6);
    let active_style = data::canonical_greeter_style(&g.style).unwrap_or("win11");
    let active_style_desc = style_options
        .iter()
        .find(|(id, _, _)| *id == active_style)
        .map(|(_, _, desc)| *desc)
        .unwrap_or("Layout personalizado da tela de bloqueio e login.");

    items.push(card_box(
        widget::column::with_children(vec![
            widget::text::heading("Estilo da Tela").into(),
            style_chips.into(),
            widget::text::caption(active_style_desc)
                .class(theme::Text::Custom(dimmed))
                .into(),
        ])
        .spacing(spacing.space_xs),
    ));

    // Wallpaper & rotation card
    let wp_mode_chips = widget::row::with_children(vec![
        small_chip(
            "Fixo",
            g.wallpaper_mode == "fixed",
            Message::GreeterWallpaperMode("fixed".to_owned()),
        ),
        small_chip(
            "Trocar a cada bloqueio",
            g.wallpaper_mode == "rotate_on_lock",
            Message::GreeterWallpaperMode("rotate_on_lock".to_owned()),
        ),
        small_chip(
            "Trocar por intervalo",
            g.wallpaper_mode == "rotate_interval",
            Message::GreeterWallpaperMode("rotate_interval".to_owned()),
        ),
    ])
    .spacing(6);

    let mut wp_rows: Vec<Element<'_, Message>> = vec![
        widget::text::heading("Papel de Parede & Rotação").into(),
        widget::row::with_children(vec![
            widget::text::body("Modo do papel de parede")
                .width(Length::Fill)
                .into(),
            wp_mode_chips.into(),
        ])
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
    ];

    if g.wallpaper_mode == "fixed" {
        wp_rows.push(
            widget::column::with_children(vec![
                widget::text::caption(
                    "Papel de parede fixo (caminho absoluto ou vazio para usar o do sistema)",
                )
                .class(theme::Text::Custom(dimmed))
                .into(),
                widget::text_input(
                    "/home/usuario/Imagens/wallpaper.jpg".to_owned(),
                    g.wallpaper.clone(),
                )
                .on_input(Message::GreeterWallpaperInput)
                .width(Length::Fill)
                .into(),
            ])
            .spacing(4)
            .into(),
        );
    } else {
        wp_rows.push(
            widget::column::with_children(vec![
                widget::text::caption(
                    "Pasta de papéis de parede (suporta jpg, jpeg, png, webp, avif, bmp)",
                )
                .class(theme::Text::Custom(dimmed))
                .into(),
                widget::text_input(
                    "~/Imagens/Wallpapers".to_owned(),
                    g.wallpaper_dir.clone(),
                )
                .on_input(Message::GreeterWallpaperDirInput)
                .width(Length::Fill)
                .into(),
                widget::row::with_children(
                    [
                        ("~/Imagens", "~/Imagens"),
                        ("~/Imagens/Wallpapers", "Wallpapers"),
                        ("/mnt/dados/Imagens", "/mnt/dados/Imagens"),
                    ]
                    .into_iter()
                    .map(|(dir, lbl)| {
                        small_chip(
                            lbl,
                            g.wallpaper_dir == dir,
                            Message::GreeterWallpaperDirInput(dir.to_owned()),
                        )
                    })
                    .collect::<Vec<_>>(),
                )
                .spacing(6)
                .into(),
            ])
            .spacing(4)
            .into(),
        );
        wp_rows.push(toggle_row(
            "Ordem aleatória (shuffle)",
            g.wallpaper_shuffle,
            Message::GreeterToggleShuffle,
        ));
        if g.wallpaper_mode == "rotate_interval" {
            let interval_chips = widget::row::with_children(
                [
                    (5_u32, "5 min"),
                    (15, "15 min"),
                    (30, "30 min"),
                    (60, "1 h"),
                    (180, "3 h"),
                    (1440, "24 h"),
                ]
                .into_iter()
                .map(|(mins, label)| {
                    small_chip(
                        label,
                        g.wallpaper_interval_mins == mins,
                        Message::GreeterIntervalMins(mins),
                    )
                })
                .collect::<Vec<_>>(),
            )
            .spacing(6);
            wp_rows.push(
                widget::row::with_children(vec![
                    widget::text::body("Intervalo de rotação")
                        .width(Length::Fill)
                        .into(),
                    interval_chips.into(),
                ])
                .spacing(8)
                .align_y(Alignment::Center)
                .into(),
            );
        }
    }

    wp_rows.push(widget::divider::horizontal::default().into());
    wp_rows.push(
        widget::row::with_children(vec![
            widget::text::caption(format!("Desfoque (blur): {} px", g.blur_radius))
                .width(Length::Fixed(185.0))
                .into(),
            widget::slider(0.0..=60.0, g.blur_radius as f32, |v| {
                Message::GreeterBlurRadius(v.round() as u32)
            })
            .width(Length::Fill)
            .into(),
        ])
        .spacing(10)
        .align_y(Alignment::Center)
        .into(),
    );
    wp_rows.push(
        widget::row::with_children(vec![
            widget::text::caption(format!("Escurecimento: {:.0}%", g.dim_opacity * 100.0))
                .width(Length::Fixed(185.0))
                .into(),
            widget::slider(0.0..=85.0, (g.dim_opacity * 100.0).round(), |v| {
                Message::GreeterDimOpacity(v / 100.0)
            })
            .width(Length::Fill)
            .into(),
        ])
        .spacing(10)
        .align_y(Alignment::Center)
        .into(),
    );

    items.push(card_box(
        widget::column::with_children(wp_rows).spacing(spacing.space_xs),
    ));

    // Typography, Clock & Visibility card
    let font_family_chips = widget::row::with_children(vec![
        small_chip("Sistema", g.font_family == "system", Message::GreeterFontFamily("system".to_owned())),
        small_chip("Sans", g.font_family == "sans", Message::GreeterFontFamily("sans".to_owned())),
        small_chip("Mono", g.font_family == "mono", Message::GreeterFontFamily("mono".to_owned())),
        small_chip("Serif", g.font_family == "serif", Message::GreeterFontFamily("serif".to_owned())),
    ])
    .spacing(6);

    let font_weight_chips = widget::row::with_children(vec![
        small_chip("Leve", g.font_weight == "light", Message::GreeterFontWeight("light".to_owned())),
        small_chip("Normal", g.font_weight == "normal", Message::GreeterFontWeight("normal".to_owned())),
        small_chip("Semibold", g.font_weight == "semibold", Message::GreeterFontWeight("semibold".to_owned())),
        small_chip("Negrito", g.font_weight == "bold", Message::GreeterFontWeight("bold".to_owned())),
    ])
    .spacing(6);

    let clock_size_chips = widget::row::with_children(vec![
        small_chip("Pequeno", g.clock_size == "small", Message::GreeterClockSize("small".to_owned())),
        small_chip("Médio", g.clock_size == "medium", Message::GreeterClockSize("medium".to_owned())),
        small_chip("Grande", g.clock_size == "large", Message::GreeterClockSize("large".to_owned())),
        small_chip("Gigante", g.clock_size == "huge", Message::GreeterClockSize("huge".to_owned())),
    ])
    .spacing(6);

    items.push(card_box(
        widget::column::with_children(vec![
            widget::text::heading("Tipografia, Relógio & Elementos").into(),
            widget::row::with_children(vec![
                widget::text::body("Fonte do relógio").width(Length::Fill).into(),
                font_family_chips.into(),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::text::body("Peso da fonte").width(Length::Fill).into(),
                font_weight_chips.into(),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::text::body("Tamanho do relógio").width(Length::Fill).into(),
                clock_size_chips.into(),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            widget::divider::horizontal::default().into(),
            toggle_row(
                "Relógio no formato 24 horas",
                g.clock_24h,
                Message::GreeterToggleClock24h,
            ),
            toggle_row(
                "Exibir segundos no relógio",
                g.show_seconds,
                Message::GreeterToggleShowSeconds,
            ),
            toggle_row(
                "Exibir data abaixo do relógio",
                g.show_date,
                Message::GreeterToggleShowDate,
            ),
            toggle_row(
                "Exibir avatar do usuário",
                g.show_avatar,
                Message::GreeterToggleShowAvatar,
            ),
            toggle_row(
                "Exibir nome do usuário",
                g.show_username,
                Message::GreeterToggleShowUsername,
            ),
        ])
        .spacing(spacing.space_xs),
    ));

    // Card Glass, Theme & Accent color
    let theme_chips = widget::row::with_children(vec![
        small_chip("Escuro (dark)", g.theme == "dark", Message::GreeterTheme("dark".to_owned())),
        small_chip("Claro (light)", g.theme == "light", Message::GreeterTheme("light".to_owned())),
    ])
    .spacing(6);

    let accent_presets = widget::row::with_children(
        [
            ("", "Padrão"),
            ("#60A5FA", "Azul"),
            ("#A78BFA", "Roxo"),
            ("#34D399", "Verde"),
            ("#FBBF24", "Âmbar"),
            ("#F472B6", "Rosa"),
        ]
        .into_iter()
        .map(|(hex, label)| {
            small_chip(
                label,
                g.accent_color.eq_ignore_ascii_case(hex),
                Message::GreeterAccentColor(hex.to_owned()),
            )
        })
        .collect::<Vec<_>>(),
    )
    .spacing(6);

    items.push(card_box(
        widget::column::with_children(vec![
            widget::text::heading("Cartão de Vidro, Tema & Destaque").into(),
            widget::row::with_children(vec![
                widget::text::body("Tema da tela de login").width(Length::Fill).into(),
                theme_chips.into(),
            ])
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::text::caption(format!(
                    "Opacidade do cartão: {:.0}%",
                    g.card_opacity * 100.0
                ))
                .width(Length::Fixed(185.0))
                .into(),
                widget::slider(0.0..=95.0, (g.card_opacity * 100.0).round(), |v| {
                    Message::GreeterCardOpacity(v / 100.0)
                })
                .width(Length::Fill)
                .into(),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::text::caption(format!(
                    "Arredondamento dos cantos: {:.0} px",
                    g.card_radius
                ))
                .width(Length::Fixed(185.0))
                .into(),
                widget::slider(4.0..=32.0, g.card_radius.round(), |v| {
                    Message::GreeterCardRadius(v.round())
                })
                .width(Length::Fill)
                .into(),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::text::body("Cor de destaque").width(Length::Fill).into(),
                accent_presets.into(),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            widget::text_input(
                "#60A5FA (ou vazio para cor do tema)".to_owned(),
                g.accent_color.clone(),
            )
            .on_input(Message::GreeterAccentColor)
            .width(Length::Fill)
            .into(),
        ])
        .spacing(spacing.space_xs),
    ));

    // User, Session & Isolation card
    items.push(card_box(
        widget::column::with_children(vec![
            widget::text::heading("Usuário, Sessão & Isolamento").into(),
            widget::row::with_children(vec![
                widget::column::with_children(vec![
                    widget::text::caption("Usuário pré-selecionado (vazio = último usuário)")
                        .class(theme::Text::Custom(dimmed))
                        .into(),
                    widget::text_input("alexandre".to_owned(), g.default_user.clone())
                        .on_input(Message::GreeterDefaultUserInput)
                        .width(Length::Fill)
                        .into(),
                ])
                .spacing(4)
                .width(Length::Fill)
                .into(),
                widget::column::with_children(vec![
                    widget::text::caption("Sessão padrão")
                        .class(theme::Text::Custom(dimmed))
                        .into(),
                    widget::text_input("COSMIC".to_owned(), g.session.clone())
                        .on_input(Message::GreeterSessionInput)
                        .width(Length::Fill)
                        .into(),
                ])
                .spacing(4)
                .width(Length::Fill)
                .into(),
            ])
            .spacing(12)
            .into(),
            widget::divider::horizontal::default().into(),
            toggle_row(
                "Usar binário isolado (/usr/local/bin/aenes-greeter no boot)",
                g.isolated_binary,
                Message::GreeterToggleIsolatedBinary,
            ),
        ])
        .spacing(spacing.space_xs),
    ));

    // Per-power-profile screen-off & lock screen timeouts summary card
    let mut idle_summary_rows: Vec<Element<'_, Message>> = vec![
        widget::row::with_children(vec![
            widget::text::heading("Tempo até Tela de Login & Desligar vídeo (por plano)")
                .width(Length::Fill)
                .into(),
            action_btn(
                "Configurar em Energia & Bateria",
                Message::SelectSection(Section::Power),
                false,
            ),
        ])
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
        widget::text::caption(
            "Os tempos até desligar o vídeo e pedir senha são ajustados individualmente em cada plano de energia.",
        )
        .class(theme::Text::Custom(dimmed))
        .into(),
    ];
    for kind in PowerProfileKind::ALL {
        let tuning = app.power_config.get(kind);
        let summary = if app.snapshot.hardware.is_laptop {
            format!(
                "Vídeo: {} · Login/senha: {} · Susp. AC: {} · Bat: {}",
                ProfileTuning::format_mins(tuning.screen_off_mins),
                ProfileTuning::format_mins(tuning.lock_mins),
                ProfileTuning::format_mins(tuning.suspend_ac_mins),
                ProfileTuning::format_mins(tuning.suspend_battery_mins),
            )
        } else {
            format!(
                "Desligar vídeo: {} · Pedir senha: {} · Suspender: {}",
                ProfileTuning::format_mins(tuning.screen_off_mins),
                ProfileTuning::format_mins(tuning.lock_mins),
                ProfileTuning::format_mins(tuning.suspend_ac_mins),
            )
        };
        idle_summary_rows.push(kv_row(kind.label(), summary));
    }
    items.push(card_box(
        widget::column::with_children(idle_summary_rows).spacing(spacing.space_xs),
    ));

    widget::column::with_children(items).spacing(spacing.space_m).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_label_uses_panel_names() {
        assert_eq!(style_label("ios"), "iOS 26");
        assert_eq!(style_label("ipados"), "iPadOS 26");
        assert_eq!(style_label("desconhecido"), "Windows 11");
    }
}
