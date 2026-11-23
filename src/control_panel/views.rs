// SPDX-License-Identifier: GPL-3.0-only

//! Views for the 6 sections of the native COSMIC Control Panel.

use cosmic::{
    Element,
    iced::{self, Alignment, Background, Color, Length},
    theme,
    widget::{self, button},
};

use crate::central::{
    bar_spacing::BarSpacing,
    services::{CameraState, PcWindow},
    tiles::{chip_class, icon, symbolic},
    widgets::card,
};

use super::{ControlPanel, Message, Section};

fn dimmed(theme: &cosmic::Theme) -> iced::widget::text::Style {
    let mut color = theme.cosmic().background(theme.transparent).on;
    color.alpha *= 0.65;
    iced::widget::text::Style {
        color: Some(color.into()),
        ..Default::default()
    }
}

fn card_box<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    widget::container(content)
        .padding(14)
        .width(Length::Fill)
        .class(theme::Container::custom(|theme| {
            let c = card(theme.cosmic());
            widget::container::Style {
                background: Some(Background::Color(c.base.into())),
                text_color: Some(c.on.into()),
                icon_color: Some(c.on.into()),
                border: iced::Border {
                    radius: theme.cosmic().corner_radii.radius_m.into(),
                    width: 1.0,
                    color: theme.cosmic().background(theme.transparent).divider.into(),
                },
                ..Default::default()
            }
        }))
        .into()
}

fn kv_row<'a>(label: &'a str, value: String) -> Element<'a, Message> {
    widget::row::with_children(vec![
        widget::text::body(label)
            .class(theme::Text::Custom(dimmed))
            .width(Length::Fill)
            .into(),
        widget::text::body(value).into(),
    ])
    .spacing(12)
    .align_y(Alignment::Center)
    .into()
}

fn status_badge<'a>(active: bool, on_text: &'a str, off_text: &'a str) -> Element<'a, Message> {
    let label = if active { on_text } else { off_text };
    widget::container(widget::text::caption(label))
        .padding([3, 10])
        .class(theme::Container::custom(move |theme| {
            let bg = if active {
                Color::from_rgba(0.22, 0.72, 0.42, 0.22)
            } else {
                let mut d: Color = theme.cosmic().background(theme.transparent).divider.into();
                d.a = 0.35;
                d
            };
            let fg: Color = if active {
                Color::from_rgb(0.35, 0.88, 0.55)
            } else {
                theme.cosmic().background(theme.transparent).on.into()
            };
            widget::container::Style {
                background: Some(Background::Color(bg)),
                text_color: Some(fg),
                border: iced::Border {
                    radius: 12.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        }))
        .into()
}

fn action_btn<'a>(label: &'a str, msg: Message, primary: bool) -> Element<'a, Message> {
    button::custom(widget::text::body(label))
        .padding([7, 14])
        .class(chip_class(primary))
        .on_press(msg)
        .into()
}

pub fn view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();

    // Left navigation sidebar
    let mut nav_items: Vec<Element<'_, Message>> = Vec::with_capacity(Section::ALL.len() + 2);
    for sec in Section::ALL {
        let selected = app.section == sec;
        let row = widget::row::with_children(vec![
            widget::icon::icon(icon(sec.icon())).size(18).into(),
            widget::text::body(sec.label()).into(),
        ])
        .spacing(10)
        .align_y(Alignment::Center);
        nav_items.push(
            button::custom(row)
                .padding([10, 12])
                .width(Length::Fill)
                .class(chip_class(selected))
                .on_press(Message::SelectSection(sec))
                .into(),
        );
    }
    nav_items.push(widget::space::vertical().height(Length::Fill).into());
    nav_items.push(
        button::custom(
            widget::row::with_children(vec![
                widget::icon::icon(icon("view-refresh-symbolic")).size(16).into(),
                widget::text::caption("Atualizar status").into(),
            ])
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding([8, 12])
        .width(Length::Fill)
        .class(chip_class(false))
        .on_press(Message::RefreshAll)
        .into(),
    );

    let sidebar = widget::container(
        widget::column::with_children(nav_items).spacing(spacing.space_xxs),
    )
    .width(Length::Fixed(240.0))
    .height(Length::Fill)
    .padding(spacing.space_s)
    .class(theme::Container::custom(|theme| {
        let bg = theme.cosmic().background(theme.transparent);
        widget::container::Style {
            background: Some(Background::Color(bg.base.into())),
            border: iced::Border {
                width: 1.0,
                color: bg.divider.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }));

    // Main content column
    let mut main_col: Vec<Element<'_, Message>> = Vec::new();
    if let Some(notice) = &app.notice {
        main_col.push(
            widget::container(
                widget::row::with_children(vec![
                    widget::icon::icon(icon("dialog-information-symbolic")).size(18).into(),
                    widget::text::body(notice.as_str()).width(Length::Fill).into(),
                    button::icon(icon("window-close-symbolic"))
                        .icon_size(14)
                        .padding(4)
                        .on_press(Message::DismissNotice)
                        .into(),
                ])
                .spacing(10)
                .align_y(Alignment::Center),
            )
            .padding([8, 12])
            .width(Length::Fill)
            .class(theme::Container::custom(|theme| {
                let mut acc: Color = theme.cosmic().accent_color().into();
                acc.a = 0.18;
                widget::container::Style {
                    background: Some(Background::Color(acc)),
                    border: iced::Border {
                        radius: theme.cosmic().corner_radii.radius_s.into(),
                        width: 1.0,
                        color: theme.cosmic().accent_color().into(),
                    },
                    ..Default::default()
                }
            }))
            .into(),
        );
    }

    let page_content = match app.section {
        Section::Power => power_view(app),
        Section::Bar => bar_view(app),
        Section::Camera => camera_view(app),
        Section::Network => network_view(app),
        Section::Displays => displays_view(app),
        Section::System => system_view(app),
    };
    main_col.push(page_content);

    let right = widget::scrollable(
        widget::container(widget::column::with_children(main_col).spacing(spacing.space_m))
            .padding(spacing.space_l)
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill);

    widget::row::with_children(vec![sidebar.into(), right.into()])
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn power_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let mut items: Vec<Element<'_, Message>> = vec![
        widget::text::title3("Energia & Bateria").into(),
    ];

    // Power profiles selector
    if let Some(power) = &app.power {
        let profile_defs = [
            ("power-saver", "power-profile-power-saver-symbolic", "Economia", Color::from_rgb(0.30, 0.82, 0.50)),
            ("balanced", "power-profile-balanced-symbolic", "Equilibrado", Color::from_rgb(0.96, 0.78, 0.25)),
            ("performance", "power-profile-performance-symbolic", "Desempenho", Color::from_rgb(0.95, 0.35, 0.35)),
        ];
        let mut buttons = Vec::new();
        for (id, ico, label, col) in profile_defs {
            if !power.available.iter().any(|p| p == id) {
                continue;
            }
            let active = power.active == id;
            let content = widget::row::with_children(vec![
                widget::icon::icon(icon(ico))
                    .size(18)
                    .class(theme::Svg::Custom(std::rc::Rc::new(move |_| {
                        iced::widget::svg::Style { color: Some(col) }
                    })))
                    .into(),
                widget::text::body(label).into(),
            ])
            .spacing(8)
            .align_y(Alignment::Center);
            buttons.push(
                button::custom(widget::container(content).center_x(Length::Fill))
                    .padding([10, 14])
                    .width(Length::Fill)
                    .class(chip_class(active))
                    .on_press(Message::SetPowerProfile(id.to_owned()))
                    .into(),
            );
        }
        items.push(card_box(
            widget::column::with_children(vec![
                widget::text::heading("Plano de energia ativo").into(),
                widget::row::with_children(buttons).spacing(spacing.space_xs).into(),
            ])
            .spacing(spacing.space_s),
        ));
    }

    // Hardware & economia-energia daemon summary
    let hw = &app.snapshot.hardware;
    let profile_summary = if hw.is_intel && hw.is_laptop {
        "Notebook Intel Core Ultra: Economia (sem Turbo, 55%, PL1 12W/PL2 18W, Arc 800 MHz) · Equilibrado (85%, 28/45W, Arc 1500 MHz) · Desempenho (100%, 45/65W)"
    } else {
        "Desktop AMD Ryzen: Economia (2,4 GHz, GPU min/1000 MHz, 60 Hz, CoolerControl silencioso) · Equilibrado (4,4 GHz, GPU 200 W, 160 Hz) · Desempenho (máximo)"
    };
    items.push(card_box(
        widget::column::with_children(vec![
            widget::row::with_children(vec![
                widget::text::heading("Motor de economia (economia-energia)").width(Length::Fill).into(),
                status_badge(app.snapshot.economia_energia_active, "Ativo", "Inativo"),
            ])
            .align_y(Alignment::Center)
            .into(),
            kv_row("Processador", hw.cpu_model.clone()),
            kv_row(
                "Perfil detectado",
                if hw.is_intel && hw.is_laptop {
                    "Notebook Intel Core Ultra (Meteor Lake)".to_owned()
                } else {
                    "Desktop AMD Ryzen".to_owned()
                },
            ),
            widget::text::caption(profile_summary)
                .class(theme::Text::Custom(dimmed))
                .into(),
        ])
        .spacing(spacing.space_xs),
    ));

    // Battery & USB-C PD telemetry (Laptop / Lenovo Yoga 7i)
    if let Some(bat) = &app.battery {
        let status_str = match bat.state {
            1 => "Carregando",
            2 => "Na bateria",
            4 => "Carregada",
            5 | 6 => "Na tomada (pausado)",
            _ if bat.charging => "Carregando",
            _ => "Na bateria",
        };
        let rate_str = match bat.energy_rate_w {
            Some(w) if bat.state == 1 => format!("{status_str} (+{w:.1} W)"),
            Some(w) if bat.state == 2 => format!("{status_str} (-{w:.1} W)"),
            Some(w) => format!("{status_str} ({w:.1} W)"),
            None => status_str.to_owned(),
        };
        let mut rows: Vec<Element<'_, Message>> = vec![
            widget::row::with_children(vec![
                widget::icon::icon(icon(&symbolic(&bat.icon))).size(22).into(),
                widget::text::heading("Bateria & Carregamento").width(Length::Fill).into(),
                widget::text::title4(format!("{:.0}%", bat.percentage)).into(),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
            kv_row("Status", rate_str),
        ];
        if let Some(charger) = bat.charger.as_ref().filter(|c| c.online) {
            let mut parts = Vec::new();
            if let Some(v) = charger.voltage_v.or(charger.max_voltage_v) {
                parts.push(format!("{v:.1} V"));
            }
            if let Some(a) = charger.current_a {
                parts.push(format!("{a:.2} A"));
            }
            if let Some(w) = charger.power_w {
                parts.push(format!("{w:.1} W"));
            } else if let Some(max_w) = charger.max_power_w {
                parts.push(format!("máx. {max_w:.0} W"));
            }
            if !parts.is_empty() {
                rows.push(kv_row("Entrada (USB-C PD / AC)", parts.join(" · ")));
            }
        }
        if let Some(v) = bat.voltage_v {
            rows.push(kv_row("Tensão da bateria", format!("{v:.2} V")));
        }
        let mut cap = Vec::new();
        if let (Some(now), Some(full)) = (bat.energy_wh, bat.energy_full_wh) {
            cap.push(format!("{now:.1} / {full:.1} Wh"));
        }
        if let Some(h) = bat.health_pct {
            cap.push(format!("Saúde {h:.0}%"));
        }
        if let Some(c) = bat.cycles {
            cap.push(format!("{c} ciclos"));
        }
        if !cap.is_empty() {
            rows.push(kv_row("Capacidade & Saúde", cap.join(" · ")));
        }
        if let Some(cons) = bat.conservation_mode {
            rows.push(widget::divider::horizontal::default().into());
            rows.push(
                widget::row::with_children(vec![
                    widget::text::body("Modo conservação Lenovo (limitar carga em 80%)")
                        .width(Length::Fill)
                        .into(),
                    widget::toggler(cons)
                        .on_toggle(|_| Message::ToggleConservationMode)
                        .into(),
                ])
                .align_y(Alignment::Center)
                .into(),
            );
        }
        rows.push(
            widget::row::with_children(vec![
                widget::text::body("Mostrar porcentagem da bateria na barra")
                    .width(Length::Fill)
                    .into(),
                widget::toggler(app.show_percentage)
                    .on_toggle(|_| Message::ToggleShowPercentage)
                    .into(),
            ])
            .align_y(Alignment::Center)
            .into(),
        );
        items.push(card_box(widget::column::with_children(rows).spacing(spacing.space_xs)));
    }

    if !app.peripherals.is_empty() {
        let mut rows: Vec<Element<'_, Message>> = vec![widget::text::heading("Periféricos").into()];
        for p in &app.peripherals {
            rows.push(kv_row(&p.name, format!("{:.0}%", p.percentage)));
        }
        items.push(card_box(widget::column::with_children(rows).spacing(spacing.space_xs)));
    }

    widget::column::with_children(items).spacing(spacing.space_m).into()
}

fn bar_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let mut items: Vec<Element<'_, Message>> = vec![
        widget::row::with_children(vec![
            widget::text::title3("Barra & Tray").width(Length::Fill).into(),
            action_btn("Reiniciar painel", Message::RestartPanel, false),
        ])
        .align_y(Alignment::Center)
        .into(),
    ];

    // Bar spacing
    let spacing_chips: Vec<Element<'_, Message>> = BarSpacing::ALL
        .into_iter()
        .map(|s| {
            button::custom(widget::text::body(s.name()))
                .padding([6, 14])
                .class(chip_class(app.bar_spacing == s))
                .on_press(Message::SetBarSpacing(s))
                .into()
        })
        .collect();
    items.push(card_box(
        widget::row::with_children(vec![
            widget::text::body("Espaçamento entre ícones").width(Length::Fill).into(),
            widget::row::with_children(spacing_chips).spacing(6).into(),
        ])
        .align_y(Alignment::Center),
    ));

    // Reorderable system bar icons with 9-dot drag handle
    let ordered = app.panel_icons.ordered(&app.reorderable_bar_items());
    let dragging = app.dragging_bar;
    let mut icon_rows: Vec<Element<'_, Message>> = vec![
        widget::text::heading("Ícones do sistema na barra (arraste pelos 9 pontos para reordenar)")
            .into(),
    ];
    for item in ordered {
        let current = app.panel_icons.get(item);
        let active_drag = dragging == Some(item);
        let grip = widget::mouse_area(
            widget::container(widget::icon::icon(icon("view-app-grid-symbolic")).size(14))
                .padding([4, 4]),
        )
        .on_press(Message::DragBarStart(item))
        .interaction(if active_drag {
            iced::mouse::Interaction::Grabbing
        } else {
            iced::mouse::Interaction::Grab
        });
        let choices = widget::row::with_children(
            item.choices()
                .iter()
                .map(|&show| {
                    button::custom(widget::text::caption(show.name()))
                        .padding([4, 8])
                        .class(chip_class(show == current))
                        .on_press(Message::SetBarIcon(item, show))
                        .into()
                })
                .collect::<Vec<_>>(),
        )
        .spacing(4);
        let row = widget::row::with_children(vec![
            grip.into(),
            widget::icon::icon(icon(item.icon())).size(16).into(),
            widget::text::body(item.name()).width(Length::Fill).into(),
            choices.into(),
        ])
        .spacing(10)
        .align_y(Alignment::Center);
        let styled: Element<'_, Message> = if active_drag {
            widget::container(row)
                .padding([4, 6])
                .width(Length::Fill)
                .class(theme::Container::custom(|theme| {
                    let c = card(theme.cosmic());
                    widget::container::Style {
                        background: Some(Background::Color(c.hover.into())),
                        border: iced::Border {
                            radius: theme.cosmic().corner_radii.radius_s.into(),
                            width: 1.0,
                            color: theme.cosmic().accent_color().into(),
                        },
                        ..Default::default()
                    }
                }))
                .into()
        } else {
            widget::container(row).padding([4, 6]).width(Length::Fill).into()
        };
        let mut area = widget::mouse_area(styled).on_enter(Message::DragBarOver(item));
        if dragging.is_some() {
            area = area.interaction(iced::mouse::Interaction::Grabbing);
        }
        icon_rows.push(area.into());
    }
    items.push(card_box(widget::column::with_children(icon_rows).spacing(4)));

    // Saved Tray SNI hidden apps
    let mut tray_rows: Vec<Element<'_, Message>> = vec![
        widget::text::heading("Apps em segundo plano ocultos da barra").into(),
    ];
    if app.tray_config.hidden.is_empty() {
        tray_rows.push(
            widget::text::caption("Nenhum app oculto — novos apps aparecem na barra por padrão.")
                .class(theme::Text::Custom(dimmed))
                .into(),
        );
    } else {
        for key in &app.tray_config.hidden {
            tray_rows.push(
                widget::row::with_children(vec![
                    widget::text::body(key.as_str()).width(Length::Fill).into(),
                    action_btn("Mostrar na barra", Message::ToggleTrayHidden(key.clone()), false),
                ])
                .align_y(Alignment::Center)
                .into(),
            );
        }
    }
    items.push(card_box(widget::column::with_children(tray_rows).spacing(spacing.space_xs)));

    widget::column::with_children(items).spacing(spacing.space_m).into()
}

fn camera_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let cam = &app.camera;
    let mut items: Vec<Element<'_, Message>> = vec![
        widget::row::with_children(vec![
            widget::text::title3("Câmeras & Privacidade").width(Length::Fill).into(),
            action_btn("Reescanear", Message::RefreshCamera, false),
        ])
        .align_y(Alignment::Center)
        .into(),
    ];

    // Hardware privacy switches (Lenovo Yoga 7i camera switch + mic mute)
    let mut priv_rows: Vec<Element<'_, Message>> = vec![
        widget::text::heading("Chaves físicas & Privacidade").into(),
    ];
    if let Some(hw_on) = cam.hw_switch {
        priv_rows.push(
            widget::row::with_children(vec![
                widget::text::body("Chave física da câmera (Lenovo)").width(Length::Fill).into(),
                status_badge(hw_on, "Ligada", "Desligada"),
            ])
            .align_y(Alignment::Center)
            .into(),
        );
    } else {
        priv_rows.push(
            widget::text::caption("Nenhuma chave física de câmera detectada nesta máquina.")
                .class(theme::Text::Custom(dimmed))
                .into(),
        );
    }
    if let Some(muted) = app.microphone {
        priv_rows.push(
            widget::row::with_children(vec![
                widget::text::body("Microfone interno (sincronizado com tecla física)")
                    .width(Length::Fill)
                    .into(),
                status_badge(!muted, "Ativo", "Mudo"),
                widget::toggler(!muted)
                    .on_toggle(|_| Message::ToggleMicrophone)
                    .into(),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
        );
    }
    items.push(card_box(widget::column::with_children(priv_rows).spacing(spacing.space_xs)));

    // Detected V4L2 cameras
    let mut cam_rows: Vec<Element<'_, Message>> = vec![
        widget::text::heading("Câmeras detectadas").into(),
    ];
    if cam.devices.is_empty() {
        cam_rows.push(
            widget::text::caption("Nenhuma câmera V4L2 detectada no momento.")
                .class(theme::Text::Custom(dimmed))
                .into(),
        );
    } else {
        let using = (!app.privacy.camera.is_empty()).then(|| app.privacy.camera.join(", "));
        for dev in &cam.devices {
            let kind = if dev.is_virtual { "Virtual" } else { "Hardware" };
            let detail = match &using {
                Some(u) => format!("{kind} · {} · Em uso: {u}", dev.path),
                None => format!("{kind} · {}", dev.path),
            };
            cam_rows.push(kv_row(&dev.name, detail));
        }
    }
    items.push(card_box(widget::column::with_children(cam_rows).spacing(spacing.space_xs)));

    // Optional IriunCam section
    let mut iriun_rows: Vec<Element<'_, Message>> = vec![
        widget::row::with_children(vec![
            widget::text::heading("IriunCam (celular como webcam — opcional)")
                .width(Length::Fill)
                .into(),
            status_badge(cam.iriun_installed && cam.on, "Ativo", if cam.iriun_installed { "Desligado" } else { "Não instalado" }),
        ])
        .align_y(Alignment::Center)
        .into(),
    ];
    if cam.iriun_installed {
        let state_txt = cam
            .status
            .as_ref()
            .and_then(|s| s.state)
            .map(|st| match st {
                CameraState::Starting => "Iniciando...",
                CameraState::WaitingPhone => "Aguardando celular...",
                CameraState::Active => "Transmitindo",
                CameraState::Error => "Erro",
                CameraState::Stopped => "Parado",
            })
            .unwrap_or("Desligado");
        iriun_rows.push(kv_row("Estado", state_txt.to_owned()));
        let mut btns = vec![action_btn(
            if cam.on { "Desligar" } else { "Ligar" },
            Message::ToggleCamera,
            !cam.on,
        )];
        if cam.on {
            btns.push(action_btn("Reiniciar", Message::RestartCamera, false));
            if let Some(win) = cam.status.as_ref().and_then(|s| s.pc_window) {
                let (lbl, show) = match win {
                    PcWindow::Hidden => ("Mostrar janela", true),
                    PcWindow::Shown => ("Ocultar janela", false),
                };
                btns.push(action_btn(lbl, Message::IriunWindow(show), false));
            }
        }
        iriun_rows.push(widget::row::with_children(btns).spacing(8).into());
    } else {
        iriun_rows.push(
            widget::text::caption("Pacote opcional — instale com ./scripts/bootstrap-pc.sh --with-iriun se desejar usar o celular como webcam.")
                .class(theme::Text::Custom(dimmed))
                .into(),
        );
    }
    items.push(card_box(widget::column::with_children(iriun_rows).spacing(spacing.space_xs)));

    widget::column::with_children(items).spacing(spacing.space_m).into()
}

fn network_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let mut items: Vec<Element<'_, Message>> = vec![
        widget::row::with_children(vec![
            widget::text::title3("Rede & Segurança").width(Length::Fill).into(),
            action_btn("Configurações COSMIC", Message::OpenCosmicSettings(Some("network")), false),
        ])
        .align_y(Alignment::Center)
        .into(),
    ];

    // Active connection card
    let mut net_rows: Vec<Element<'_, Message>> = vec![
        widget::text::heading("Conexão ativa").into(),
    ];
    if let Some(d) = &app.network.details {
        let kind = match d.kind.as_str() {
            "802-3-ethernet" => "Cabeada",
            "802-11-wireless" => "Wi-Fi",
            "vpn" | "wireguard" => "VPN",
            _ => "Rede",
        };
        net_rows.push(kv_row("Conexão", format!("{} ({kind} · {})", d.name, d.interface)));
        if let Some(ip4) = &d.ipv4 {
            net_rows.push(kv_row("IPv4", ip4.clone()));
        }
        if let Some(ip6) = &d.ipv6 {
            net_rows.push(kv_row("IPv6", ip6.clone()));
        }
        if let Some(gw) = &d.gateway {
            net_rows.push(kv_row("Gateway", gw.clone()));
        }
        if !d.dns.is_empty() {
            net_rows.push(kv_row("DNS", d.dns.join(", ")));
        }
        if let Some(mac) = &d.mac {
            net_rows.push(kv_row("Endereço MAC", mac.clone()));
        }
        if let Some(sec) = &d.security {
            net_rows.push(kv_row("Segurança", sec.clone()));
        }
    } else {
        net_rows.push(
            widget::text::caption("Desconectado")
                .class(theme::Text::Custom(dimmed))
                .into(),
        );
    }
    net_rows.push(widget::divider::horizontal::default().into());
    net_rows.push(
        widget::row::with_children(vec![
            widget::text::body("MAC Rotativo (mac-rotativo.timer)").width(Length::Fill).into(),
            status_badge(app.snapshot.mac_rotativo_active, "Ativo", "Inativo"),
        ])
        .align_y(Alignment::Center)
        .into(),
    );
    items.push(card_box(widget::column::with_children(net_rows).spacing(spacing.space_xs)));

    // Wi-Fi card (when adapter exists)
    if app.network.has_wifi {
        items.push(card_box(
            widget::row::with_children(vec![
                widget::text::heading("Wi-Fi").width(Length::Fill).into(),
                widget::toggler(app.network.wifi_enabled)
                    .on_toggle(|_| Message::ToggleWifi)
                    .into(),
            ])
            .align_y(Alignment::Center),
        ));
    }

    // VPNs
    if !app.network.vpns.is_empty() {
        let mut vpn_rows: Vec<Element<'_, Message>> = vec![widget::text::heading("VPNs").into()];
        for (i, vpn) in app.network.vpns.iter().enumerate() {
            let on = vpn.active.is_some();
            vpn_rows.push(
                widget::row::with_children(vec![
                    widget::text::body(vpn.name.as_str()).width(Length::Fill).into(),
                    status_badge(on, "Conectada", "Desconectada"),
                    action_btn(if on { "Desconectar" } else { "Conectar" }, Message::ConnectVpn(i), !on),
                ])
                .spacing(10)
                .align_y(Alignment::Center)
                .into(),
            );
        }
        items.push(card_box(widget::column::with_children(vpn_rows).spacing(spacing.space_xs)));
    }

    widget::column::with_children(items).spacing(spacing.space_m).into()
}

fn displays_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let mut items: Vec<Element<'_, Message>> = vec![
        widget::row::with_children(vec![
            widget::text::title3("Monitores & Visuais").width(Length::Fill).into(),
            action_btn("Ler DDC / Atualizar", Message::RefreshDisplays, false),
        ])
        .align_y(Alignment::Center)
        .into(),
    ];

    if let Some(displays) = &app.displays {
        for (idx, d) in displays.iter().enumerate() {
            let title = if d.internal() {
                "Tela interna".to_owned()
            } else {
                format!("{} {} ({})", d.make, d.model, d.name).trim().to_owned()
            };
            let is_primary = app.primary_display.as_deref() == Some(d.name.as_str());
            let mut rows: Vec<Element<'_, Message>> = vec![
                widget::row::with_children(vec![
                    widget::text::heading(title).width(Length::Fill).into(),
                    if is_primary {
                        status_badge(true, "Principal", "")
                    } else if d.enabled && d.ddc.is_some() {
                        action_btn("Tornar principal", Message::SetPrimaryDisplay(d.name.clone()), false)
                    } else {
                        widget::space::horizontal().width(Length::Shrink).into()
                    },
                ])
                .align_y(Alignment::Center)
                .into(),
                kv_row(
                    "Modo",
                    format!(
                        "{} × {} · {:.0} Hz · Escala {:.0}%",
                        d.width,
                        d.height,
                        d.refresh as f32 / 1000.0,
                        d.scale * 100.0
                    ),
                ),
            ];
            if d.internal()
                && let Some((cur, max)) = app.brightness
            {
                let pct = (cur as f32 / max.max(1) as f32 * 100.0).clamp(1.0, 100.0);
                rows.push(
                    widget::row::with_children(vec![
                        widget::text::caption(format!("Brilho: {pct:.0}%"))
                            .width(Length::Fixed(100.0))
                            .into(),
                        widget::slider(1.0..=100.0, pct, Message::SetInternalBrightness)
                            .width(Length::Fill)
                            .into(),
                    ])
                    .spacing(10)
                    .align_y(Alignment::Center)
                    .into(),
                );
            } else if let Some((_, cur, max)) = d.ddc {
                let pct = (cur as f32 / max.max(1) as f32 * 100.0).clamp(0.0, 100.0);
                rows.push(
                    widget::row::with_children(vec![
                        widget::text::caption(format!("Brilho DDC: {pct:.0}%"))
                            .width(Length::Fixed(120.0))
                            .into(),
                        widget::slider(0.0..=100.0, pct, move |v| Message::SetDdcBrightness(idx, v))
                            .on_release(Message::ApplyDdcBrightness(idx))
                            .width(Length::Fill)
                            .into(),
                    ])
                    .spacing(10)
                    .align_y(Alignment::Center)
                    .into(),
                );
            }
            items.push(card_box(widget::column::with_children(rows).spacing(spacing.space_xs)));
        }
    } else {
        items.push(card_box(widget::text::caption("Lendo monitores...")));
    }

    // Visual utilities
    items.push(card_box(
        widget::column::with_children(vec![
            widget::text::heading("Utilitários visuais").into(),
            widget::row::with_children(vec![
                widget::text::body("Wallpapers por monitor").width(Length::Fill).into(),
                status_badge(app.snapshot.wallpapers_watcher_active, "Watcher ativo", "Inativo"),
                action_btn("Reaplicar agora", Message::RunWallpapers, false),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::text::body("Ícones PWA do Brave").width(Length::Fill).into(),
                status_badge(app.snapshot.pwa_watcher_active, "Watcher ativo", "Inativo"),
                action_btn("Corrigir agora", Message::RunBravePwaFix, false),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::text::body("Recorte de tela (Super+Shift+S)").width(Length::Fill).into(),
                action_btn("Abrir recorte", Message::RunScreenshot, false),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
        ])
        .spacing(spacing.space_xs),
    ));

    widget::column::with_children(items).spacing(spacing.space_m).into()
}

fn system_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let repo = &app.snapshot.repo;
    let mut items: Vec<Element<'_, Message>> = vec![
        widget::text::title3("Atualizações & Serviços").into(),
    ];

    // Repository & Auto-updater card
    let repo_path = repo
        .path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Não encontrado".to_owned());
    let commit_line = if repo.commit_short.is_empty() {
        "—".to_owned()
    } else if repo.dirty {
        format!("{} (alterações locais) · {}", repo.commit_short, repo.commit_subject)
    } else {
        format!("{} · {}", repo.commit_short, repo.commit_subject)
    };
    items.push(card_box(
        widget::column::with_children(vec![
            widget::row::with_children(vec![
                widget::text::heading("Atualizador automático (por boot)")
                    .width(Length::Fill)
                    .into(),
                status_badge(app.snapshot.update_timer_active, "Ativado", "Desativado"),
            ])
            .align_y(Alignment::Center)
            .into(),
            kv_row("Repositório", repo_path),
            kv_row("Versão atual", commit_line),
            widget::row::with_children(vec![
                action_btn("Verificar atualizações", Message::CheckUpdates, false),
                action_btn("Atualizar agora", Message::RunUpdateNow, true),
                action_btn("Exportar perfil visual", Message::ExportConfig, false),
            ])
            .spacing(8)
            .into(),
        ])
        .spacing(spacing.space_xs),
    ));

    // Services card
    let mut unit_rows: Vec<Element<'_, Message>> = vec![
        widget::text::heading("Serviços em segundo plano").into(),
    ];
    for u in &app.snapshot.units {
        let mut row = widget::row::with_capacity(4)
            .push(
                widget::column::with_children(vec![
                    widget::text::body(u.label).into(),
                    widget::text::caption(u.unit)
                        .class(theme::Text::Custom(dimmed))
                        .into(),
                ])
                .width(Length::Fill),
            )
            .push(status_badge(u.active, "Ativo", "Inativo"))
            .spacing(10)
            .align_y(Alignment::Center);
        if !u.is_system {
            row = row.push(action_btn("Reiniciar", Message::RestartUserUnit(u.unit), false));
        }
        unit_rows.push(row.into());
    }
    items.push(card_box(widget::column::with_children(unit_rows).spacing(spacing.space_xs)));

    widget::column::with_children(items).spacing(spacing.space_m).into()
}
