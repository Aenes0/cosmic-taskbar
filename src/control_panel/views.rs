// SPDX-License-Identifier: GPL-3.0-only

//! Views for the sections of the native COSMIC Control Panel.

use cosmic::{
    Element,
    iced::{self, Alignment, Background, Color, Length},
    theme,
    widget::{self, button},
};

use crate::central::{
    bar_spacing::BarSpacing,
    layout::{Section as CentralSection, Show as BarShow},
    monitor::config::{
        BarItem as MonitorBarItem, BarMode as MonitorBarMode, CardMode as MonitorCardMode,
        PROCS_COUNT_OPTIONS, Panel as MonitorPanel, PollComponent as MonitorPollComponent,
        ProcSort as MonitorProcSort, WEATHER_MINS_OPTIONS,
    },
    notifications::config::{
        self as notif_cfg, NotifFormat, NotifPosition, TIMEOUT_OPTIONS, parse_hex_color,
    },
    services::{self, CameraState, PcWindow, Touchpad},
    tiles::{chip_class, icon, symbolic},
    widgets::card,
};
use crate::logi::config as osd_config;

use super::{
    ControlPanel, Message, Section, data,
    power_config::{self, PowerProfileKind, ProfileTuning},
    screenshot_config::{self, OpenMode as ScreenshotOpenMode},
};

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
        .padding(18)
        .width(Length::Fill)
        .class(theme::Container::custom(|theme| {
            let c = card(theme.cosmic());
            let mut border_col: Color = theme.cosmic().background(theme.transparent).divider.into();
            border_col.a *= 0.55;
            widget::container::Style {
                background: Some(Background::Color(c.base.into())),
                text_color: Some(c.on.into()),
                icon_color: Some(c.on.into()),
                border: iced::Border {
                    radius: crate::shared::tokens::POPUP_RADIUS.into(),
                    width: crate::shared::tokens::HAIRLINE_WIDTH,
                    color: border_col,
                },
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.14),
                    offset: iced::Vector::new(0.0, 2.0),
                    blur_radius: 10.0,
                },
                ..Default::default()
            }
        }))
        .into()
}

fn section_subtitle(sec: Section) -> &'static str {
    match sec {
        Section::Power => "Perfis de energia, bateria e tempos de tela/suspensão.",
        Section::Bar => "Ícones da barra direita, espaçamento e apps em segundo plano.",
        Section::Monitor => "Cartões do painel de medições à esquerda e tempos de atualização.",
        Section::Central => "Seções e atalhos exibidos ao abrir o Central pelo relógio.",
        Section::Notifications => "Posição, estilo, duração e comportamento dos avisos flutuantes.",
        Section::DevicesOsd => "Avisos visuais na tela para teclado e mouse Logitech.",
        Section::Camera => "Câmeras conectadas, microfone e privacidade de vídeo/áudio.",
        Section::Network => "Conexão ativa, Wi-Fi, IP e diagnóstico rápido de rede.",
        Section::Security => "Firewall, DNS criptografado, privacidade agora e blindagem.",
        Section::Backup => "Backup diário Restic, snapshots Btrfs e sincronização.",
        Section::Displays => "Brilho, escala, folga de janela dividida e foco de janelas.",
        Section::Login => "Aparência da tela de bloqueio e login no boot.",
        Section::Optional => "Componentes opcionais e integrações extras do ambiente.",
        Section::System => "Serviços em segundo plano, atualizações e saúde do sistema.",
    }
}

fn page_banner<'a>(sec: Section) -> Element<'a, Message> {
    let badge = widget::container(widget::icon::icon(icon(sec.icon())).size(20))
        .padding(10)
        .class(theme::Container::custom(|theme| {
            let mut acc: Color = theme.cosmic().accent_color().into();
            let fg = acc;
            acc.a = 0.16;
            widget::container::Style {
                background: Some(Background::Color(acc)),
                icon_color: Some(fg),
                border: iced::Border {
                    radius: 10.0.into(),
                    width: 1.0,
                    color: Color { a: 0.35, ..fg },
                },
                ..Default::default()
            }
        }));
    widget::row::with_children(vec![
        badge.into(),
        widget::column::with_children(vec![
            widget::text::title4(sec.label()).into(),
            widget::text::caption(section_subtitle(sec))
                .class(theme::Text::Custom(dimmed))
                .into(),
        ])
        .spacing(2)
        .width(Length::Fill)
        .into(),
    ])
    .spacing(12)
    .align_y(Alignment::Center)
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

fn small_chip<'a>(label: &'a str, active: bool, msg: Message) -> Element<'a, Message> {
    button::custom(widget::text::caption(label))
        .padding([5, 10])
        .class(chip_class(active))
        .on_press(msg)
        .into()
}

fn toggle_row<'a>(label: &'a str, value: bool, msg: Message) -> Element<'a, Message> {
    widget::row::with_children(vec![
        widget::text::body(label).width(Length::Fill).into(),
        widget::toggler(value).on_toggle(move |_| msg.clone()).into(),
    ])
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}

pub fn view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let query = app.search.trim().to_lowercase();

    // Left navigation sidebar: instant search + clean grouped categories
    let search_box = widget::search_input("Buscar ajuste...", &app.search)
        .on_input(Message::SearchChanged)
        .on_submit(|_| Message::SearchSubmit)
        .width(Length::Fill);

    let mut nav_items: Vec<Element<'_, Message>> = Vec::with_capacity(Section::ALL.len() + 6);
    for &(group_title, sections) in Section::GROUPS {
        let matching: Vec<Section> = sections
            .iter()
            .copied()
            .filter(|sec| {
                query.is_empty()
                    || sec.label().to_lowercase().contains(&query)
                    || sec.keywords().contains(&query)
            })
            .collect();
        if matching.is_empty() {
            continue;
        }
        if !nav_items.is_empty() {
            nav_items.push(widget::space::vertical().height(Length::Fixed(6.0)).into());
        }
        nav_items.push(
            widget::container(
                widget::text::caption(group_title)
                    .font(cosmic::font::semibold())
                    .class(theme::Text::Custom(dimmed)),
            )
            .padding([4, 10])
            .into(),
        );
        for sec in matching {
            let selected = app.section == sec;
            let row = widget::row::with_children(vec![
                widget::icon::icon(icon(sec.icon())).size(16).into(),
                widget::text::body(sec.label()).into(),
            ])
            .spacing(10)
            .align_y(Alignment::Center);
            nav_items.push(
                button::custom(row)
                    .padding([7, 10])
                    .width(Length::Fill)
                    .class(chip_class(selected))
                    .on_press(Message::SelectSection(sec))
                    .into(),
            );
        }
    }
    if nav_items.is_empty() {
        nav_items.push(
            widget::container(
                widget::text::caption("Nenhum ajuste encontrado.")
                    .class(theme::Text::Custom(dimmed)),
            )
            .padding([12, 10])
            .into(),
        );
    }

    let mut sidebar_footer: Vec<Element<'_, Message>> = Vec::new();
    if app.busy {
        sidebar_footer.push(
            widget::text::caption("Executando ação...")
                .class(theme::Text::Custom(dimmed))
                .into(),
        );
    }
    sidebar_footer.push(
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

    let nav_list = widget::scrollable(
        widget::column::with_children(nav_items).spacing(2),
    )
    .height(Length::Fill);

    let sidebar = widget::container(
        widget::column::with_children(vec![
            search_box.into(),
            nav_list.into(),
            widget::column::with_children(sidebar_footer)
                .spacing(spacing.space_xxs)
                .into(),
        ])
        .spacing(spacing.space_s)
        .height(Length::Fill),
    )
    .width(Length::Fixed(256.0))
    .height(Length::Fill)
    .padding(spacing.space_s)
    .class(theme::Container::custom(|theme| {
        let bg = theme.cosmic().background(theme.transparent);
        let mut div: Color = bg.divider.into();
        div.a *= 0.65;
        widget::container::Style {
            background: Some(Background::Color(bg.base.into())),
            border: iced::Border {
                width: 1.0,
                color: div,
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

    main_col.push(page_banner(app.section));
    let page_content = match app.section {
        Section::Power => power_view(app),
        Section::Bar => bar_view(app),
        Section::Monitor => monitor_view(app),
        Section::Central => central_view(app),
        Section::Notifications => notifications_view(app),
        Section::DevicesOsd => devices_osd_view(app),
        Section::Camera => camera_view(app),
        Section::Network => network_view(app),
        Section::Security => security_view(app),
        Section::Backup => backup_view(app),
        Section::Displays => displays_view(app),
        Section::Login => login_view(app),
        Section::Optional => optional_view(app),
        Section::System => system_view(app),
    };
    main_col.push(page_content);

    let centered_content = widget::container(
        widget::container(widget::column::with_children(main_col).spacing(spacing.space_m))
            .max_width(860.0)
            .width(Length::Fill),
    )
    .padding(spacing.space_l)
    .center_x(Length::Fill);

    let right = widget::scrollable(centered_content)
        .width(Length::Fill)
        .height(Length::Fill);

    widget::row::with_children(vec![sidebar.into(), right.into()])
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn power_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let hw = &app.snapshot.hardware;
    let topo = &app.cpu_topology;
    let editing = app.editing_profile;
    let t = app.power_config.get(editing);

    let mut items: Vec<Element<'_, Message>> = vec![
        widget::row::with_children(vec![
            widget::text::title3("Energia & Bateria").width(Length::Fill).into(),
            action_btn("Restaurar padrões do hardware", Message::PowerResetDefaults, false),
            action_btn("Salvar e aplicar plano", Message::PowerSaveAndApply, true),
        ])
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
    ];

    // Active power profile selector (power-profiles-daemon)
    if let Some(power) = &app.power {
        let profile_defs = [
            (
                "power-saver",
                "power-profile-power-saver-symbolic",
                "Economia",
                Color::from_rgb(0.30, 0.82, 0.50),
            ),
            (
                "balanced",
                "power-profile-balanced-symbolic",
                "Equilibrado",
                Color::from_rgb(0.96, 0.78, 0.25),
            ),
            (
                "performance",
                "power-profile-performance-symbolic",
                "Desempenho",
                Color::from_rgb(0.95, 0.35, 0.35),
            ),
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
                widget::row::with_children(vec![
                    widget::text::heading("Plano de energia ativo").width(Length::Fill).into(),
                    status_badge(app.snapshot.economia_energia_active, "Daemon ativo", "Daemon inativo"),
                ])
                .align_y(Alignment::Center)
                .into(),
                widget::row::with_children(buttons).spacing(spacing.space_xs).into(),
            ])
            .spacing(spacing.space_s),
        ));
    }

    // Profile editor selector tabs
    let tab_buttons: Vec<Element<'_, Message>> = PowerProfileKind::ALL
        .into_iter()
        .map(|kind| {
            let selected = editing == kind;
            let is_live = app.power.as_ref().is_some_and(|p| p.active == kind.id());
            let lbl = if is_live {
                format!("{} (ativo)", kind.label())
            } else {
                kind.label().to_owned()
            };
            button::custom(widget::container(widget::text::body(lbl)).center_x(Length::Fill))
                .padding([8, 12])
                .width(Length::Fill)
                .class(chip_class(selected))
                .on_press(Message::PowerEditProfile(kind))
                .into()
        })
        .collect();
    let topo_summary = format!(
        "{} · {} núcleos físicos / {} threads{}",
        hw.cpu_model,
        topo.physical_cores,
        topo.logical_cpus,
        if topo.is_amd_dual_ccd { " · AMD Dual-CCD" } else { "" }
    );
    items.push(card_box(
        widget::column::with_children(vec![
            widget::row::with_children(vec![
                widget::text::heading("Configurar plano de energia").width(Length::Fill).into(),
                action_btn("Salvar sem aplicar", Message::PowerSaveOnly, false),
            ])
            .align_y(Alignment::Center)
            .into(),
            widget::text::caption(topo_summary)
                .class(theme::Text::Custom(dimmed))
                .into(),
            widget::row::with_children(tab_buttons).spacing(spacing.space_xs).into(),
        ])
        .spacing(spacing.space_xs),
    ));

    // Collapsible toggle for technical CPU/GPU tuning sliders so everyday users
    // see clean profile selection & screen timeouts first.
    items.push(
        widget::row::with_children(vec![
            widget::column::with_children(vec![
                widget::text::heading(format!("Ajuste fino de CPU e GPU ({})", editing.label())).into(),
                widget::text::caption(
                    "Limites de frequência, núcleos ativos, watts e clock da GPU. Os padrões do perfil já são otimizados para seu hardware.",
                )
                .class(theme::Text::Custom(dimmed))
                .into(),
            ])
            .spacing(2)
            .width(Length::Fill)
            .into(),
            action_btn(
                if app.power_show_advanced {
                    "Ocultar ajuste fino"
                } else {
                    "Mostrar ajuste fino"
                },
                Message::TogglePowerAdvanced,
                false,
            ),
        ])
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
    );

    if app.power_show_advanced {
    // --- 1. CPU card ---
    let hw_max = topo.hw_max_mhz.max(5500).max(t.cpu_max_mhz);
    let freq_label = if t.cpu_max_mhz == 0 {
        "Máximo do hardware".to_owned()
    } else {
        format!("{} MHz ({:.2} GHz)", t.cpu_max_mhz, t.cpu_max_mhz as f32 / 1000.0)
    };
    let freq_presets: Vec<Element<'_, Message>> = [
        (0u32, "Máx."),
        (2000, "2.0 GHz"),
        (2400, "2.4 GHz"),
        (3200, "3.2 GHz"),
        (4000, "4.0 GHz"),
        (4400, "4.4 GHz"),
    ]
    .into_iter()
    .map(|(mhz, lbl)| small_chip(lbl, t.cpu_max_mhz == mhz, Message::PowerCpuMaxMhz(mhz)))
    .collect();

    let epp_chips: Vec<Element<'_, Message>> = power_config::EPP_OPTIONS
        .iter()
        .map(|&(val, lbl)| small_chip(lbl, t.epp == val, Message::PowerEpp(val.to_owned())))
        .collect();

    let max_cores_cap = topo.physical_cores.max(16).max(t.max_online_cores);
    let cores_label = if t.max_online_cores == 0 {
        format!("Todos ({} núcleos)", topo.physical_cores.max(1))
    } else {
        format!("{} núcleo(s)", t.max_online_cores)
    };

    let mut cpu_rows: Vec<Element<'_, Message>> = vec![
        widget::text::heading(format!("CPU — {}", editing.label())).into(),
        toggle_row(
            "Boost / Turbo da CPU (Precision Boost / Intel Turbo)",
            t.boost,
            Message::PowerToggleBoost,
        ),
        widget::row::with_children(vec![
            widget::text::body("Teto de frequência").width(Length::Fill).into(),
            widget::text::caption(freq_label)
                .class(theme::Text::Custom(dimmed))
                .into(),
        ])
        .align_y(Alignment::Center)
        .into(),
        widget::slider(0.0..=hw_max as f32, t.cpu_max_mhz as f32, |v| {
            let rounded = ((v / 100.0).round() as u32) * 100;
            Message::PowerCpuMaxMhz(if rounded < 800 { 0 } else { rounded })
        })
        .step(100.0_f32)
        .width(Length::Fill)
        .into(),
        widget::row::with_children(freq_presets).spacing(4).into(),
        widget::row::with_children(vec![
            widget::text::body("Preferência de eficiência (EPP)")
                .width(Length::Fill)
                .into(),
            widget::row::with_children(epp_chips).spacing(4).into(),
        ])
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
    ];

    if hw.is_intel {
        cpu_rows.push(widget::divider::horizontal::default().into());
        cpu_rows.push(
            widget::row::with_children(vec![
                widget::text::caption(format!("Intel max_perf_pct: {}%", t.intel_max_perf_pct))
                    .width(Length::Fixed(190.0))
                    .into(),
                widget::slider(10.0..=100.0, t.intel_max_perf_pct as f32, |v| {
                    Message::PowerIntelMaxPerf(v.round() as u32)
                })
                .step(5.0_f32)
                .width(Length::Fill)
                .into(),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
        );
        let pl1_txt = if t.rapl_pl1_w == 0 {
            "RAPL PL1: Padrão HW".to_owned()
        } else {
            format!("RAPL PL1 (sustentado): {} W", t.rapl_pl1_w)
        };
        cpu_rows.push(
            widget::row::with_children(vec![
                widget::text::caption(pl1_txt).width(Length::Fixed(190.0)).into(),
                widget::slider(0.0..=100.0, t.rapl_pl1_w.min(100) as f32, |v| {
                    Message::PowerRaplPl1(v.round() as u32)
                })
                .step(1.0_f32)
                .width(Length::Fill)
                .into(),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
        );
        let pl2_txt = if t.rapl_pl2_w == 0 {
            "RAPL PL2: Padrão HW".to_owned()
        } else {
            format!("RAPL PL2 (pico curto): {} W", t.rapl_pl2_w)
        };
        cpu_rows.push(
            widget::row::with_children(vec![
                widget::text::caption(pl2_txt).width(Length::Fixed(190.0)).into(),
                widget::slider(0.0..=120.0, t.rapl_pl2_w.min(120) as f32, |v| {
                    Message::PowerRaplPl2(v.round() as u32)
                })
                .step(1.0_f32)
                .width(Length::Fill)
                .into(),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
        );
    }

    cpu_rows.push(widget::divider::horizontal::default().into());
    cpu_rows.push(toggle_row(
        if topo.is_amd_dual_ccd {
            "Desativar CCD1 (AMD dual-CCD — detectado)"
        } else {
            "Desativar CCD1 (AMD dual-CCD)"
        },
        t.disable_ccd1,
        Message::PowerToggleDisableCcd1,
    ));
    cpu_rows.push(
        widget::row::with_children(vec![
            widget::text::caption(format!("Limite de núcleos online: {cores_label}"))
                .width(Length::Fixed(230.0))
                .into(),
            widget::slider(0.0..=max_cores_cap as f32, t.max_online_cores as f32, |v| {
                Message::PowerMaxOnlineCores(v.round() as u32)
            })
            .step(1.0_f32)
            .width(Length::Fill)
            .into(),
        ])
        .spacing(10)
        .align_y(Alignment::Center)
        .into(),
    );
    items.push(card_box(widget::column::with_children(cpu_rows).spacing(spacing.space_xs)));

    // --- 2. GPU card (NVIDIA dedicated + Intel Arc iGPU) ---
    let mut gpu_rows: Vec<Element<'_, Message>> = vec![
        widget::text::heading(format!("GPU — {}", editing.label())).into(),
    ];
    if hw.has_nvidia || !hw.is_intel {
        let pl_lbl = if t.nvidia_power_limit_w == 0 {
            "NVIDIA Power Limit: Padrão/Mín.".to_owned()
        } else {
            format!("NVIDIA Power Limit: {} W", t.nvidia_power_limit_w)
        };
        let gpu_clk_lbl = if t.nvidia_gpu_clock_mhz == 0 {
            "Travar clock GPU: Livre".to_owned()
        } else {
            format!("Travar clock GPU: {} MHz", t.nvidia_gpu_clock_mhz)
        };
        let mem_clk_lbl = if t.nvidia_mem_clock_mhz == 0 {
            "Travar clock Memória: Livre".to_owned()
        } else {
            format!("Travar clock Memória: {} MHz", t.nvidia_mem_clock_mhz)
        };
        gpu_rows.push(
            widget::row::with_children(vec![
                widget::text::caption(pl_lbl).width(Length::Fixed(220.0)).into(),
                widget::slider(0.0..=450.0, t.nvidia_power_limit_w.min(450) as f32, |v| {
                    let w = ((v / 5.0).round() as u32) * 5;
                    Message::PowerNvidiaPowerLimit(if w < 50 { 0 } else { w })
                })
                .step(5.0_f32)
                .width(Length::Fill)
                .into(),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
        );
        gpu_rows.push(
            widget::row::with_children(vec![
                widget::text::caption(gpu_clk_lbl).width(Length::Fixed(220.0)).into(),
                widget::slider(0.0..=3000.0, t.nvidia_gpu_clock_mhz.min(3000) as f32, |v| {
                    let m = ((v / 50.0).round() as u32) * 50;
                    Message::PowerNvidiaGpuClock(if m < 300 { 0 } else { m })
                })
                .step(50.0_f32)
                .width(Length::Fill)
                .into(),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
        );
        gpu_rows.push(
            widget::row::with_children(vec![
                widget::text::caption(mem_clk_lbl).width(Length::Fixed(220.0)).into(),
                widget::slider(0.0..=12000.0, t.nvidia_mem_clock_mhz.min(12000) as f32, |v| {
                    let m = ((v / 100.0).round() as u32) * 100;
                    Message::PowerNvidiaMemClock(if m < 400 { 0 } else { m })
                })
                .step(100.0_f32)
                .width(Length::Fill)
                .into(),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
        );
        gpu_rows.push(toggle_row(
            "NVIDIA Persistence Mode (nvidia-smi -pm)",
            t.nvidia_persistence,
            Message::PowerToggleNvidiaPersistence,
        ));
    }
    if hw.is_intel {
        if hw.has_nvidia {
            gpu_rows.push(widget::divider::horizontal::default().into());
        }
        let arc_lbl = if t.intel_igpu_max_mhz == 0 {
            "iGPU Intel Arc máx.: Padrão HW".to_owned()
        } else {
            format!("iGPU Intel Arc máx.: {} MHz", t.intel_igpu_max_mhz)
        };
        gpu_rows.push(
            widget::row::with_children(vec![
                widget::text::caption(arc_lbl).width(Length::Fixed(220.0)).into(),
                widget::slider(0.0..=2400.0, t.intel_igpu_max_mhz.min(2400) as f32, |v| {
                    let m = ((v / 50.0).round() as u32) * 50;
                    Message::PowerIntelIgpuMaxMhz(if m < 300 { 0 } else { m })
                })
                .step(50.0_f32)
                .width(Length::Fill)
                .into(),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
        );
    }
    items.push(card_box(widget::column::with_children(gpu_rows).spacing(spacing.space_xs)));
    } // end if app.power_show_advanced

    // --- 3. Screen, Login Lock & Suspend per profile ---
    let idle_chips = |current: u32, presets: &[u32], make: fn(u32) -> Message| {
        widget::row::with_children(
            presets
                .iter()
                .map(|&m| {
                    let lbl = if m == 0 {
                        "Nunca".to_owned()
                    } else if m < 60 {
                        format!("{m}m")
                    } else {
                        format!("{}h", m / 60)
                    };
                    button::custom(widget::text::caption(lbl))
                        .padding([4, 8])
                        .class(chip_class(current == m))
                        .on_press(make(m))
                        .into()
                })
                .collect::<Vec<_>>(),
        )
        .spacing(4)
    };
    let hz_chips = widget::row::with_children(
        [
            ("", "Manter"),
            ("59.987", "60 Hz"),
            ("120", "120 Hz"),
            ("144", "144 Hz"),
            ("159.962", "160 Hz"),
        ]
        .into_iter()
        .map(|(val, lbl)| small_chip(lbl, t.display_hz == val, Message::PowerDisplayHz(val.to_owned())))
        .collect::<Vec<_>>(),
    )
    .spacing(4);

    let idle_rows: Vec<Element<'_, Message>> = vec![
        widget::text::heading(format!("Tela & Tela de Login — {}", editing.label())).into(),
        widget::text::caption(
            "Estes tempos são aplicados automaticamente no COSMIC Idle ao trocar para este plano de energia.",
        )
        .class(theme::Text::Custom(dimmed))
        .into(),
        widget::row::with_children(vec![
            widget::text::body(format!(
                "Desligar vídeo ({})",
                ProfileTuning::format_mins(t.screen_off_mins)
            ))
            .width(Length::Fill)
            .into(),
            idle_chips(
                t.screen_off_mins,
                &[0, 2, 5, 10, 15, 30, 60],
                Message::PowerScreenOffMins,
            )
            .into(),
        ])
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
        widget::row::with_children(vec![
            widget::text::body(format!(
                "Pedir senha / Tela de Login ({})",
                ProfileTuning::format_mins(t.lock_mins)
            ))
            .width(Length::Fill)
            .into(),
            idle_chips(
                t.lock_mins,
                &[0, 1, 3, 5, 10, 15, 30, 60],
                Message::PowerLockMins,
            )
            .into(),
        ])
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
        widget::row::with_children(vec![
            widget::text::body(format!(
                "Suspender na tomada ({})",
                ProfileTuning::format_mins(t.suspend_ac_mins)
            ))
            .width(Length::Fill)
            .into(),
            idle_chips(
                t.suspend_ac_mins,
                &[0, 15, 30, 45, 60, 120],
                Message::PowerSuspendAcMins,
            )
            .into(),
        ])
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
        widget::row::with_children(vec![
            widget::text::body(format!(
                "Suspender na bateria ({})",
                ProfileTuning::format_mins(t.suspend_battery_mins)
            ))
            .width(Length::Fill)
            .into(),
            idle_chips(
                t.suspend_battery_mins,
                &[0, 5, 10, 15, 30, 60],
                Message::PowerSuspendBatteryMins,
            )
            .into(),
        ])
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
        widget::divider::horizontal::default().into(),
        widget::row::with_children(vec![
            widget::text::body("Taxa de atualização (Hz)")
                .width(Length::Fill)
                .into(),
            hz_chips.into(),
        ])
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
        widget::row::with_children(vec![
            widget::text::caption("Valor exato (vazio = não alterar)")
                .class(theme::Text::Custom(dimmed))
                .width(Length::Fill)
                .into(),
            widget::text_input("Ex: 59.987 ou 159.962".to_owned(), t.display_hz.clone())
                .on_input(Message::PowerDisplayHz)
                .width(Length::Fixed(180.0))
                .into(),
        ])
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
    ];
    items.push(card_box(widget::column::with_children(idle_rows).spacing(spacing.space_xs)));

    // --- 4. Platform power controls ---
    let aspm_chips = widget::row::with_children(
        power_config::ASPM_OPTIONS
            .iter()
            .map(|&(val, lbl)| {
                small_chip(lbl, t.pcie_aspm == val, Message::PowerPcieAspm(val.to_owned()))
            })
            .collect::<Vec<_>>(),
    )
    .spacing(4);
    let sata_chips = widget::row::with_children(
        power_config::SATA_ALPM_OPTIONS
            .iter()
            .map(|&(val, lbl)| {
                small_chip(lbl, t.sata_alpm == val, Message::PowerSataAlpm(val.to_owned()))
            })
            .collect::<Vec<_>>(),
    )
    .spacing(4);
    items.push(card_box(
        widget::column::with_children(vec![
            widget::text::heading(format!("Plataforma — {}", editing.label())).into(),
            widget::row::with_children(vec![
                widget::text::body("PCIe ASPM").width(Length::Fill).into(),
                aspm_chips.into(),
            ])
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::text::body("SATA ALPM").width(Length::Fill).into(),
                sata_chips.into(),
            ])
            .align_y(Alignment::Center)
            .into(),
            toggle_row(
                "Economia de energia do áudio (snd_hda_intel power_save)",
                t.audio_power_save,
                Message::PowerToggleAudioSave,
            ),
            toggle_row(
                "Economia de energia do Wi-Fi (iw power_save)",
                t.wifi_power_save,
                Message::PowerToggleWifiSave,
            ),
            kv_row(
                "Arquivo de perfis",
                power_config::PowerProfilesConfig::config_path()
                    .to_string_lossy()
                    .into_owned(),
            ),
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
            rows.push(toggle_row(
                "Modo conservação Lenovo (limitar carga em 80%)",
                cons,
                Message::ToggleConservationMode,
            ));
        }
        rows.push(toggle_row(
            "Mostrar porcentagem da bateria na barra",
            app.show_percentage,
            Message::ToggleShowPercentage,
        ));
        items.push(card_box(widget::column::with_children(rows).spacing(spacing.space_xs)));
    }

    if !app.peripherals.is_empty() {
        let mut rows: Vec<Element<'_, Message>> = vec![widget::text::heading("Periféricos").into()];
        for p in &app.peripherals {
            rows.push(kv_row(&p.name, format!("{:.0}%", p.percentage)));
        }
        items.push(card_box(widget::column::with_children(rows).spacing(spacing.space_xs)));
    }

    // Kernel, modules & power/hardware rules card
    let snap = &app.snapshot;
    let repo_dir = snap
        .repo
        .path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "$HOME/Projetos/cosmic-taskbar".to_owned());
    let install_power_cmd = format!("\"{repo_dir}/extras/power/install-economia-energia.sh\"");
    let rule_row = |label: &'static str, ok: bool, ok_lbl: &'static str, off_lbl: &'static str| {
        widget::row::with_children(vec![
            widget::text::body(label).width(Length::Fill).into(),
            status_badge(ok, ok_lbl, off_lbl),
        ])
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
    };
    items.push(card_box(
        widget::column::with_children(vec![
            widget::row::with_children(vec![
                widget::text::heading("Kernel, Módulos & Regras de Energia")
                    .width(Length::Fill)
                    .into(),
                action_btn(
                    "Instalar / Atualizar",
                    Message::RunAction("Instalar economia-energia e regras", install_power_cmd, true),
                    !snap.economia_energia_active,
                ),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            rule_row(
                "Daemon economia-energia.service",
                snap.economia_energia_active,
                "Ativo",
                "Inativo",
            ),
            rule_row(
                "Áudio power_save no boot (/etc/modprobe.d/99-economia-energia.conf)",
                snap.audio_modprobe_configured,
                "Configurado",
                "Ausente",
            ),
            rule_row(
                "Leitura RAPL de consumo em Watts (/etc/udev/rules.d/99-rapl.rules)",
                snap.rapl_udev_configured,
                "Ativo",
                "Ausente",
            ),
            rule_row(
                "Módulo i2c-dev (brilho DDC/CI nos monitores)",
                snap.i2c_dev_module_configured,
                "No boot",
                "Ausente",
            ),
            rule_row(
                "Módulo nct6775 (sensores e ventoinhas da placa-mãe)",
                snap.nct6775_module_configured,
                "No boot",
                "Ausente",
            ),
        ])
        .spacing(spacing.space_xs),
    ));

    widget::column::with_children(items).spacing(spacing.space_m).into()
}

fn security_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let snap = &app.snapshot;
    let opt = &snap.optional;
    let sec = &snap.security;
    let repo_dir = snap
        .repo
        .path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "$HOME/Projetos/cosmic-taskbar".to_owned());
    let harden_script = format!("\"{repo_dir}/extras/security/apply-hardening.sh\"");

    // Helper: clean, human-readable item with title, plain-Portuguese explanation,
    // recommendation tag, status badge, and optional action button.
    fn guided_row<'a>(
        title: &'static str,
        hint: &'static str,
        active: bool,
        on_lbl: &'static str,
        off_lbl: &'static str,
        action: Option<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        let mut right: Vec<Element<'a, Message>> = vec![status_badge(active, on_lbl, off_lbl)];
        if let Some(btn) = action {
            right.push(btn);
        }
        widget::row::with_children(vec![
            widget::column::with_children(vec![
                widget::text::body(title).into(),
                widget::text::caption(hint)
                    .class(theme::Text::Custom(dimmed))
                    .into(),
            ])
            .spacing(2)
            .width(Length::Fill)
            .into(),
            widget::row::with_children(right)
                .spacing(8)
                .align_y(Alignment::Center)
                .into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    }

    let mut items: Vec<Element<'_, Message>> = vec![
        widget::row::with_children(vec![
            widget::text::title3("Segurança & Privacidade")
                .width(Length::Fill)
                .into(),
            status_badge(
                sec.score >= 80,
                if sec.score >= 90 {
                    "Bem protegido"
                } else {
                    "Proteção boa"
                },
                "Pode melhorar",
            ),
            action_btn("Verificar agora", Message::RefreshAll, false),
        ])
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
    ];

    // 1. Plain-language summary banner + 1-click Recommended Protection
    let recommended_done = sec.ufw_active && sec.dns_over_tls && sec.nm_privacy_conf && sec.kernel_hardening_conf;
    let summary_title = if sec.score >= 90 {
        format!("Seu computador está bem protegido ({}/100)", sec.score)
    } else if sec.score >= 75 {
        format!("Proteção boa, com poucos ajustes opcionais ({}/100)", sec.score)
    } else {
        format!("Recomendamos ativar as proteções essenciais ({}/100)", sec.score)
    };
    let recommended_cmd = format!(
        "{harden_script} --firewall-strict && {harden_script} --dns quad9 && {harden_script} --nm-privacy && {harden_script} --kernel"
    );
    items.push(card_box(
        widget::column::with_children(vec![
            widget::row::with_children(vec![
                widget::column::with_children(vec![
                    widget::text::heading(summary_title).into(),
                    widget::text::caption(
                        "Ativar a proteção recomendada liga o firewall, criptografa seu DNS (Quad9) e aplica regras seguras no sistema — sem quebrar jogos, apps ou sua internet.",
                    )
                    .class(theme::Text::Custom(dimmed))
                    .into(),
                ])
                .spacing(4)
                .width(Length::Fill)
                .into(),
                action_btn(
                    if recommended_done {
                        "Reaplicar recomendado"
                    } else {
                        "Ativar proteção recomendada"
                    },
                    Message::RunAction("Proteção Recomendada", recommended_cmd, true),
                    !recommended_done,
                ),
            ])
            .spacing(12)
            .align_y(Alignment::Center)
            .into(),
        ])
        .spacing(spacing.space_xs),
    ));

    // 2. Essential Everyday Protection (clear, safe, recommended)
    let hosts_btn_lbl = if sec.hosts_adblock_active {
        "Desativar"
    } else {
        "Ativar"
    };
    let hosts_btn_cmd = if sec.hosts_adblock_active {
        format!("{harden_script} --hosts-undo")
    } else {
        format!("{harden_script} --hosts-adblock")
    };
    let nm_btn_label = if sec.nm_privacy_conf {
        "Desativar"
    } else {
        "Ativar"
    };
    let nm_btn_cmd = if sec.nm_privacy_conf {
        format!("{harden_script} --nm-privacy-undo")
    } else {
        format!("{harden_script} --nm-privacy")
    };
    let dns_label = if sec.dns_server.is_empty() {
        "Padrão do roteador".to_owned()
    } else {
        sec.dns_server.clone()
    };
    items.push(card_box(
        widget::column::with_children(vec![
            widget::text::heading("Proteção essencial do dia a dia").into(),
            widget::text::caption(
                "Opções simples e seguras para quase todo mundo. Cada item explica o efeito real antes de você clicar.",
            )
            .class(theme::Text::Custom(dimmed))
            .into(),
            guided_row(
                "Firewall (bloquear acessos externos)",
                "Recomendado · Impede que outros dispositivos na rede acessem seu PC sem permissão. Não atrapalha navegação nem jogos.",
                sec.ufw_active,
                "Ativo",
                if sec.ufw_installed { "Desativado" } else { "Não instalado" },
                Some(action_btn(
                    if sec.ufw_active { "Reaplicar" } else { "Ativar" },
                    Message::RunAction(
                        "Firewall UFW",
                        format!("{harden_script} --firewall-strict"),
                        true,
                    ),
                    !sec.ufw_active,
                )),
            ),
            guided_row(
                "DNS criptografado (navegação privada)",
                "Recomendado · Impede que o provedor ou redes Wi-Fi vejam quais sites você acessa. Escolha um provedor abaixo.",
                sec.dns_over_tls,
                "Criptografado",
                "Sem criptografia",
                None,
            ),
            widget::row::with_children(vec![
                widget::text::caption(format!("Atual: {dns_label}"))
                    .class(theme::Text::Custom(dimmed))
                    .width(Length::Fill)
                    .into(),
                action_btn(
                    "Quad9 (bloqueia golpes)",
                    Message::RunAction("DNS Quad9", format!("{harden_script} --dns quad9"), true),
                    !sec.dns_over_tls,
                ),
                action_btn(
                    "Cloudflare (mais rápido)",
                    Message::RunAction("DNS Cloudflare", format!("{harden_script} --dns cloudflare"), true),
                    false,
                ),
                action_btn(
                    "Mullvad (zero registros)",
                    Message::RunAction("DNS Mullvad", format!("{harden_script} --dns mullvad"), true),
                    false,
                ),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            guided_row(
                "Bloquear rastreadores e sites maliciosos no sistema todo",
                "Opcional · Bloqueia anúncios e rastreadores em todos os apps. Pode bloquear alguns links patrocinados de busca.",
                sec.hosts_adblock_active,
                "Ativo",
                "Desativado",
                Some(action_btn(
                    hosts_btn_lbl,
                    Message::RunAction("Bloqueio de Rastreadores", hosts_btn_cmd, true),
                    false,
                )),
            ),
            guided_row(
                "Privacidade de Wi-Fi e rede",
                "Recomendado · Esconde o identificador real ao procurar redes Wi-Fi e desativa testes automáticos de conexão.",
                sec.nm_privacy_conf,
                "Ativo",
                "Padrão",
                Some(action_btn(
                    nm_btn_label,
                    Message::RunAction("Privacidade de Rede", nm_btn_cmd, true),
                    !sec.nm_privacy_conf,
                )),
            ),
            guided_row(
                "Proteção básica do sistema (Kernel)",
                "Recomendado · Impede que programas comuns espionem a memória de outros apps. Seguro para uso diário.",
                sec.kernel_hardening_conf,
                "Ativo",
                "Padrão",
                Some(action_btn(
                    if sec.kernel_hardening_conf { "Desativar" } else { "Ativar" },
                    Message::RunAction(
                        "Proteção do Sistema",
                        if sec.kernel_hardening_conf {
                            format!("{harden_script} --kernel-undo")
                        } else {
                            format!("{harden_script} --kernel")
                        },
                        true,
                    ),
                    !sec.kernel_hardening_conf,
                )),
            ),
        ])
        .spacing(spacing.space_s),
    ));

    // 3. Privacy Right Now (Camera, Microphone, Lock, Traces & Panic Switch)
    let mic_muted = app.microphone.unwrap_or(false);
    let cam_active = !app.privacy.camera.is_empty();
    let cam_consumers = if app.privacy.camera.is_empty() {
        "Nenhum app usando a câmera agora".to_owned()
    } else {
        format!("Em uso por: {}", app.privacy.camera.join(", "))
    };
    let mic_consumers = if app.privacy.microphone.is_empty() {
        "Nenhum app gravando microfone agora".to_owned()
    } else {
        format!("Em uso por: {}", app.privacy.microphone.join(", "))
    };
    let killswitch_label = if sec.networking_enabled {
        "Cortar internet agora"
    } else {
        "Religar internet"
    };
    let killswitch_cmd = if sec.networking_enabled {
        "nmcli networking off && echo 'Internet desligada.' ".to_owned()
    } else {
        "nmcli networking on && echo 'Internet religada.' ".to_owned()
    };
    let clean_traces_cmd = concat!(
        "rm -rf \"${XDG_DATA_HOME:-$HOME/.local/share}/cosmic-central/clipboard\"* ",
        "\"${XDG_DATA_HOME:-$HOME/.local/share}/recently-used.xbel\" ",
        "\"${XDG_CACHE_HOME:-$HOME/.cache}/thumbnails\"/* 2>/dev/null || true; ",
        "systemctl --user restart central-clipboard.service 2>/dev/null || true; ",
        "echo 'Rastros limpos (Área de transferência, arquivos recentes e miniaturas).'"
    )
    .to_owned();
    items.push(card_box(
        widget::column::with_children(vec![
            widget::text::heading("Privacidade agora").into(),
            widget::row::with_children(vec![
                widget::column::with_children(vec![
                    widget::text::body("Câmera").into(),
                    widget::text::caption(cam_consumers)
                        .class(theme::Text::Custom(dimmed))
                        .into(),
                ])
                .spacing(2)
                .width(Length::Fill)
                .into(),
                status_badge(!cam_active, "Livre", "Em uso"),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::column::with_children(vec![
                    widget::text::body("Microfone").into(),
                    widget::text::caption(mic_consumers)
                        .class(theme::Text::Custom(dimmed))
                        .into(),
                ])
                .spacing(2)
                .width(Length::Fill)
                .into(),
                status_badge(mic_muted, "Silenciado", "Ativo"),
                widget::toggler(!mic_muted)
                    .on_toggle(|_| Message::ToggleMicrophone)
                    .into(),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
            guided_row(
                "Botão de emergência da internet (Kill Switch)",
                "Desliga Wi-Fi e cabo na hora caso desconfie de algo. Clique novamente para reconectar.",
                sec.networking_enabled,
                "Conectada",
                "DESLIGADA",
                Some(action_btn(
                    killswitch_label,
                    Message::RunAction("Internet (Kill Switch)", killswitch_cmd, false),
                    !sec.networking_enabled,
                )),
            ),
            widget::row::with_children(vec![
                action_btn("Bloquear tela", Message::GreeterTestLock, true),
                action_btn(
                    "Limpar histórico (Clipboard, recentes e miniaturas)",
                    Message::RunAction("Limpeza de Rastros", clean_traces_cmd, false),
                    false,
                ),
                action_btn(
                    "Câmeras & Áudio",
                    Message::SelectSection(Section::Camera),
                    false,
                ),
            ])
            .spacing(8)
            .into(),
        ])
        .spacing(spacing.space_s),
    ));

    // 4. Extra Network Privacy (VPN status + Optional Rotating MAC)
    let mac_toggle_label = if snap.mac_rotativo_active {
        "Desativar troca automática"
    } else {
        "Ativar troca automática"
    };
    let mac_toggle_cmd = if snap.mac_rotativo_active {
        "systemctl disable --now mac-rotativo.timer".to_owned()
    } else {
        "systemctl enable --now mac-rotativo.timer".to_owned()
    };
    items.push(card_box(
        widget::column::with_children(vec![
            widget::text::heading("Privacidade extra na rede").into(),
            guided_row(
                "VPN ativa",
                "Mostra se existe um túnel VPN conectado agora (WireGuard, OpenVPN ou Tailscale). Opcional.",
                !sec.vpn_iface.is_empty(),
                "Conectada",
                "Sem VPN",
                None,
            ),
            guided_row(
                "Trocar endereço MAC periodicamente",
                "Opcional (útil em Wi-Fi público) · Muda o identificador da placa de rede. Pode dar uma breve piscada na conexão ao trocar.",
                snap.mac_rotativo_active,
                "Automático",
                if opt.mac_rotativo_installed { "Desativado" } else { "Não instalado" },
                Some(
                    widget::row::with_children(vec![
                        action_btn(
                            "Trocar agora",
                            Message::RunAction(
                                "Rotacionar MAC",
                                "systemctl start mac-rotativo.service && echo 'Endereço MAC trocado.'".to_owned(),
                                true,
                            ),
                            false,
                        ),
                        action_btn(
                            mac_toggle_label,
                            Message::RunAction("MAC Rotativo", mac_toggle_cmd, true),
                            false,
                        ),
                    ])
                    .spacing(6)
                    .into(),
                ),
            ),
        ])
        .spacing(spacing.space_s),
    ));

    // 5. Collapsible Advanced Section (for power users — hidden by default!)
    items.push(
        widget::row::with_children(vec![
            widget::column::with_children(vec![
                widget::text::heading("Opções avançadas e auditoria").into(),
                widget::text::caption(
                    "Ajustes técnicos do kernel, bloqueio físico de USB e ferramentas de diagnóstico. Só mexa se souber que precisa.",
                )
                .class(theme::Text::Custom(dimmed))
                .into(),
            ])
            .spacing(2)
            .width(Length::Fill)
            .into(),
            action_btn(
                if app.security_show_advanced {
                    "Ocultar opções avançadas"
                } else {
                    "Mostrar opções avançadas"
                },
                Message::ToggleSecurityAdvanced,
                false,
            ),
        ])
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
    );

    if app.security_show_advanced {
        let opt_toggle_row = |title: &'static str,
                              hint: &'static str,
                              active: bool,
                              action_title: &'static str,
                              flag: &'static str| {
            let next_mode = if active { "off" } else { "on" };
            let btn_lbl = if active { "Desativar" } else { "Ativar" };
            guided_row(
                title,
                hint,
                active,
                "Ativo",
                "Desativado",
                Some(action_btn(
                    btn_lbl,
                    Message::RunAction(action_title, format!("{harden_script} {flag} {next_mode}"), true),
                    false,
                )),
            )
        };
        let usbguard_lbl = if sec.usbguard_active {
            "Desativar USBGuard"
        } else {
            "Ativar USBGuard"
        };
        let usbguard_cmd = if sec.usbguard_active {
            format!("{harden_script} --usbguard-disable")
        } else {
            format!("{harden_script} --usbguard-setup")
        };
        let exposed_label = if sec.exposed_ports_count == 0 {
            "Nenhuma porta exposta".to_owned()
        } else {
            format!("{} porta(s) aberta(s) para a rede", sec.exposed_ports_count)
        };
        let lockdown_ok = sec.lockdown_mode == "integrity" || sec.lockdown_mode == "confidentiality";

        items.push(card_box(
            widget::column::with_children(vec![
                widget::text::heading("Blindagem avançada do sistema").into(),
                opt_toggle_row(
                    "Bloquear arquivos de despejo de memória (Coredumps)",
                    "Recomendado · Evita salvar conteúdo da RAM em disco quando um app trava (protege senhas).",
                    sec.suid_dumpable == 0 && sec.nocoredump_conf,
                    "Bloquear Coredumps",
                    "--toggle-nocoredump",
                ),
                opt_toggle_row(
                    "Desativar protocolos antigos pouco usados (Blacklist)",
                    "Recomendado · Desliga drivers legados (FireWire, HFS, DCCP/SCTP) que quase ninguém usa.",
                    sec.blacklist_modules_conf,
                    "Blacklist de Módulos",
                    "--toggle-blacklist",
                ),
                opt_toggle_row(
                    "Restringir io_uring para apps comuns",
                    "Avançado · Fecha uma superfície frequente de falhas no kernel. Pode afetar bancos de dados ou ferramentas de benchmark.",
                    sec.io_uring_disabled >= 1,
                    "Restringir io_uring",
                    "--toggle-io-uring",
                ),
                opt_toggle_row(
                    "Modo invisível na rede (ignorar ping / Stealth ICMP)",
                    "Avançado · Seu PC para de responder ao comando ping. Pode dificultar testes de rede local.",
                    sec.icmp_stealth,
                    "Stealth ICMP",
                    "--toggle-icmp-stealth",
                ),
                opt_toggle_row(
                    "Desativar atalhos de emergência do kernel (Magic SysRq)",
                    "Avançado · Desliga combinações Alt+PrintScreen do kernel. Deixe desativado se você usa REISUB para destravar o PC.",
                    sec.sysrq_disabled,
                    "Magic SysRq",
                    "--toggle-sysrq",
                ),
                guided_row(
                    "Bloqueio físico de portas USB (USBGuard)",
                    "Cuidado · Só libera dispositivos USB já conectados hoje; novos pendrives ou teclados precisarão ser autorizados manualmente.",
                    sec.usbguard_active,
                    "Ativo",
                    if sec.usbguard_installed { "Desativado" } else { "Não instalado" },
                    Some(action_btn(
                        usbguard_lbl,
                        Message::RunAction("USBGuard", usbguard_cmd, true),
                        false,
                    )),
                ),
                widget::row::with_children(vec![
                    action_btn(
                        "Aplicar blindagem completa",
                        Message::RunAction(
                            "Blindagem Completa",
                            format!("{harden_script} --kernel && {harden_script} --kernel-armor && {harden_script} --nm-privacy"),
                            true,
                        ),
                        false,
                    ),
                    action_btn(
                        "Reverter blindagem",
                        Message::RunAction(
                            "Reverter Blindagem",
                            format!("{harden_script} --kernel-armor-undo"),
                            true,
                        ),
                        false,
                    ),
                ])
                .spacing(8)
                .into(),
            ])
            .spacing(spacing.space_s),
        ));

        items.push(card_box(
            widget::column::with_children(vec![
                widget::text::heading("Diagnóstico e auditoria técnica").into(),
                kv_row("Portas de rede abertas", format!("{exposed_label} · {} locais", sec.local_ports_count)),
                kv_row("Criptografia de disco (LUKS)", if sec.luks_active { "Ativa".to_owned() } else { "Não detectada".to_owned() }),
                kv_row("Isolamento da tela de login", if opt.greeter_service_enabled { "Boot + Sessão isolados".to_owned() } else { "Sessão isolada".to_owned() }),
                kv_row("Kernel / Lockdown", format!("{} · {}", if sec.kernel_release.is_empty() { "—" } else { sec.kernel_release.as_str() }, if lockdown_ok { sec.lockdown_mode.as_str() } else { "sem lockdown" })),
                kv_row("Serviços locais (SSH / Avahi / Impressão / Bluetooth)", format!(
                    "SSH: {} · Avahi: {} · Impressão: {} · Bluetooth: {}",
                    if sec.sshd_active { "ativo" } else { "off" },
                    if sec.avahi_active { "ativo" } else { "off" },
                    if sec.cups_active { "ativo" } else { "off" },
                    if sec.bluetooth_active { "ativo" } else { "off" },
                )),
                widget::row::with_children(vec![
                    action_btn(
                        "Ver regras do Firewall",
                        Message::OpenInTerminal("Regras UFW", "sudo ufw status verbose".to_owned()),
                        false,
                    ),
                    action_btn(
                        "Conexões ativas",
                        Message::OpenInTerminal("Conexões Ativas", "sudo ss -tunp state established".to_owned()),
                        false,
                    ),
                    action_btn(
                        "Portas abertas",
                        Message::OpenInTerminal("Portas Abertas", "sudo ss -tulnp".to_owned()),
                        false,
                    ),
                    action_btn(
                        "Verificar pacotes (pacman)",
                        Message::OpenInTerminal(
                            "Integridade do Sistema",
                            "pacman -Qkk 2>&1 | grep -Ev '0 arquivos alterados|0 altered files' | head -n 60; echo '--- Fim da auditoria ---'".to_owned(),
                        ),
                        false,
                    ),
                    action_btn(
                        "Permissões Flatpak",
                        Message::OpenInTerminal(
                            "Permissões Flatpak",
                            if sec.flatpak_installed {
                                "for a in $(flatpak list --app --columns=application); do perms=$(flatpak info --show-permissions \"$a\" 2>/dev/null | grep -E 'filesystems=.*host|sockets=.*x11|devices=all'); [ -n \"$perms\" ] && printf '\\n[%s]\\n%s\\n' \"$a\" \"$perms\"; done".to_owned()
                            } else {
                                "echo 'Flatpak não instalado.'".to_owned()
                            },
                        ),
                        false,
                    ),
                ])
                .spacing(8)
                .into(),
            ])
            .spacing(spacing.space_xs),
        ));
    }

    widget::column::with_children(items).spacing(spacing.space_m).into()
}

fn backup_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let snap = &app.snapshot;

    let mut items: Vec<Element<'_, Message>> = vec![
        widget::row::with_children(vec![
            widget::text::title3("Backup & Sincronização")
                .width(Length::Fill)
                .into(),
            action_btn("Atualizar status", Message::RefreshAll, false),
        ])
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
    ];

    // 1. Restic daily backup card
    items.push(card_box(
        widget::column::with_children(vec![
            widget::row::with_children(vec![
                widget::text::heading("Backup Diário (Restic → /mnt/devdrive)")
                    .width(Length::Fill)
                    .into(),
                status_badge(
                    snap.backup_restic_timer_active,
                    "Timer ativo (12:00)",
                    if snap.backup_restic_installed {
                        "Instalado (inativo)"
                    } else {
                        "Não instalado"
                    },
                ),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::text::body("Disco de destino (/mnt/devdrive)")
                    .width(Length::Fill)
                    .into(),
                status_badge(snap.backup_drive_mounted, "Montado", "Desmontado"),
            ])
            .align_y(Alignment::Center)
            .into(),
            widget::text::caption(
                "Salva Repositorios, Documentos, Imagens, ~/Projetos, ~/.ssh, ~/.gnupg, ~/.config e ~/.local/share (retém 7 diários + 4 semanais).",
            )
            .class(theme::Text::Custom(dimmed))
            .into(),
            widget::row::with_children(vec![
                action_btn(
                    "Executar backup agora",
                    Message::OpenInTerminal(
                        "Backup Restic",
                        "~/.local/bin/backup-restic".to_owned(),
                    ),
                    true,
                ),
                action_btn(
                    "Listar snapshots",
                    Message::OpenInTerminal(
                        "Snapshots Restic",
                        "RESTIC_REPOSITORY=/mnt/devdrive/backup-restic RESTIC_PASSWORD_FILE=$HOME/.config/restic/senha restic snapshots".to_owned(),
                    ),
                    false,
                ),
                action_btn(
                    "Ativar / Reiniciar timer",
                    Message::EnableUserUnit("backup-restic.timer"),
                    false,
                ),
            ])
            .spacing(8)
            .into(),
        ])
        .spacing(spacing.space_xs),
    ));

    // 2. Btrfs Snapper & SSD fstrim card
    items.push(card_box(
        widget::column::with_children(vec![
            widget::text::heading("Snapshots Btrfs (Snapper) & Manutenção SSD").into(),
            widget::row::with_children(vec![
                widget::text::body("Snapper timeline semanal (snapper-timeline.timer)")
                    .width(Length::Fill)
                    .into(),
                status_badge(snap.snapper_timer_active, "Ativo", "Inativo"),
            ])
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::text::body("SSD TRIM semanal (fstrim.timer — Dom 04:00)")
                    .width(Length::Fill)
                    .into(),
                status_badge(snap.fstrim_timer_active, "Ativo", "Inativo"),
            ])
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                action_btn(
                    "Listar snapshots Snapper (sudo)",
                    Message::OpenInTerminal("Snapshots Snapper", "sudo snapper list".to_owned()),
                    false,
                ),
                action_btn(
                    "Ver agendamentos",
                    Message::OpenInTerminal(
                        "Timers de Disco",
                        "systemctl list-timers snapper-timeline.timer fstrim.timer --no-pager".to_owned(),
                    ),
                    false,
                ),
            ])
            .spacing(8)
            .into(),
        ])
        .spacing(spacing.space_xs),
    ));

    // 3. Claude Sync & Cross-PC bundle card
    let export_claude_cmd = concat!(
        "OUT=\"$HOME/claude-sync-bundle-$(date +%Y%m%d-%H%M).tar.gz\"; ",
        "tar -czf \"$OUT\" -C \"$HOME\" ",
        "--exclude='.claude/.credentials.json' ",
        "--exclude='.claude/cache' ",
        "--exclude='.claude/downloads' ",
        "--exclude='.claude/telemetry' ",
        "--exclude='.claude/bash-commands.log' ",
        "--exclude='.claude/cost-tracker.log' ",
        "--exclude='.claude/backups*' ",
        ".claude/CLAUDE.md .claude/settings.json .claude/agents .claude/hooks .claude/rules .claude/skills .claude/references .claude/projects 2>/dev/null || true; ",
        "ls -lh \"$OUT\" && echo \"Para restaurar no outro PC: tar -xzf $(basename \"$OUT\") -C ~\""
    )
    .to_owned();

    items.push(card_box(
        widget::column::with_children(vec![
            widget::row::with_children(vec![
                widget::text::heading("Sincronização do Claude (Contas & Outro PC)")
                    .width(Length::Fill)
                    .into(),
                status_badge(
                    snap.claude_sync_active,
                    "Sync local ativo",
                    if snap.claude_sync_installed {
                        "Instalado (inativo)"
                    } else {
                        "Não instalado"
                    },
                ),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            widget::text::caption(
                "O claude-sync-contas espelha sessões entre as 2 contas locais. Para levar toda a estrutura do Claude (CLAUDE.md, settings, skills, agents, hooks, rules e projetos — sem credenciais) para outro PC, use o exportador abaixo.",
            )
            .class(theme::Text::Custom(dimmed))
            .into(),
            widget::row::with_children(vec![
                action_btn(
                    "Sincronizar contas locais agora",
                    Message::OpenInTerminal(
                        "Claude Sync Contas",
                        "python3 \"$HOME/claude-migracao/sincronizar_contas.py\"".to_owned(),
                    ),
                    false,
                ),
                action_btn(
                    "Exportar pacote Claude para outro PC",
                    Message::OpenInTerminal("Exportar Claude Bundle", export_claude_cmd),
                    true,
                ),
            ])
            .spacing(8)
            .into(),
        ])
        .spacing(spacing.space_xs),
    ));

    widget::column::with_children(items).spacing(spacing.space_m).into()
}

#[derive(Clone, Copy)]
enum TileTone {
    /// Always visible / Expanded (vibrant emerald/teal)
    Teal,
    /// Compact (cool sky blue)
    Blue,
    /// Active-only / Minimal (warm amber)
    Amber,
    /// Never / Hidden (dim neutral gray)
    Muted,
}

impl TileTone {
    fn colors(self) -> (Color, Color, Color) {
        // (card_bg, border, pill_fg)
        match self {
            TileTone::Teal => (
                Color::from_rgba(0.16, 0.72, 0.56, 0.16),
                Color::from_rgba(0.24, 0.82, 0.65, 0.48),
                Color::from_rgb(0.32, 0.90, 0.72),
            ),
            TileTone::Blue => (
                Color::from_rgba(0.24, 0.58, 0.96, 0.16),
                Color::from_rgba(0.34, 0.66, 0.98, 0.48),
                Color::from_rgb(0.46, 0.76, 1.0),
            ),
            TileTone::Amber => (
                Color::from_rgba(0.95, 0.70, 0.18, 0.15),
                Color::from_rgba(0.96, 0.74, 0.26, 0.46),
                Color::from_rgb(0.98, 0.80, 0.36),
            ),
            TileTone::Muted => (
                Color::from_rgba(0.50, 0.50, 0.55, 0.08),
                Color::from_rgba(0.50, 0.50, 0.55, 0.22),
                Color::from_rgba(0.75, 0.75, 0.80, 0.72),
            ),
        }
    }
}

fn tone_pill<'a>(label: &'a str, tone: TileTone) -> Element<'a, Message> {
    let (bg, _, fg) = tone.colors();
    let pill_bg = Color { a: (bg.a * 1.45).min(0.35), ..bg };
    widget::container(widget::text::caption(label).size(11))
        .padding([2, 8])
        .class(theme::Container::custom(move |_| widget::container::Style {
            background: Some(Background::Color(pill_bg)),
            text_color: Some(fg),
            border: iced::Border {
                radius: 999.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }))
        .into()
}

fn draggable_grid_tile<'a>(
    icon_name: &'static str,
    title: &'a str,
    status_label: &'a str,
    tone: TileTone,
    is_dragging_this: bool,
    any_dragging: bool,
    on_cycle: Message,
    on_drag_start: Message,
    on_drag_over: Message,
    footer: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let (bg, border_col, _) = tone.colors();
    let grip = widget::mouse_area(
        widget::container(widget::icon::icon(icon("view-app-grid-symbolic")).size(12))
            .padding([3, 4]),
    )
    .on_press(on_drag_start)
    .interaction(if is_dragging_this {
        iced::mouse::Interaction::Grabbing
    } else {
        iced::mouse::Interaction::Grab
    });

    let top_bar = widget::row::with_children(vec![
        grip.into(),
        widget::space::horizontal().width(Length::Fill).into(),
        tone_pill(status_label, tone),
    ])
    .align_y(Alignment::Center);

    let body_btn = button::custom(
        widget::column::with_children(vec![
            widget::icon::icon(icon(icon_name)).size(24).into(),
            widget::text::caption(title)
                .font(cosmic::font::semibold())
                .align_x(Alignment::Center)
                .into(),
        ])
        .spacing(6)
        .align_x(Alignment::Center)
        .width(Length::Fill),
    )
    .padding([6, 4])
    .width(Length::Fill)
    .class(theme::Button::Text)
    .on_press(on_cycle);

    let mut tile_rows: Vec<Element<'a, Message>> = vec![top_bar.into(), body_btn.into()];
    if let Some(foot) = footer {
        tile_rows.push(foot);
    }
    let tile_content = widget::column::with_children(tile_rows)
        .spacing(6)
        .width(Length::Fill);

    let card_el = widget::container(tile_content)
        .padding([8, 10])
        .width(Length::Fill)
        .class(theme::Container::custom(move |theme| {
            let mut text_col: Color = theme.cosmic().background(theme.transparent).on.into();
            if matches!(tone, TileTone::Muted) {
                text_col.a *= 0.62;
            }
            let (b_width, b_color) = if is_dragging_this {
                (2.0, theme.cosmic().accent_color().into())
            } else {
                (1.0, border_col)
            };
            widget::container::Style {
                background: Some(Background::Color(bg)),
                text_color: Some(text_col),
                icon_color: Some(text_col),
                border: iced::Border {
                    radius: crate::shared::tokens::POPUP_RADIUS.into(),
                    width: b_width,
                    color: b_color,
                },
                ..Default::default()
            }
        }));

    let mut area = widget::mouse_area(card_el).on_enter(on_drag_over);
    if any_dragging {
        area = area.interaction(iced::mouse::Interaction::Grabbing);
    }
    area.into()
}

fn tile_grid<'a>(tiles: Vec<Element<'a, Message>>, cols: usize) -> Element<'a, Message> {
    let mut rows: Vec<Element<'a, Message>> = Vec::new();
    let mut iter = tiles.into_iter();
    loop {
        let mut line: Vec<Element<'a, Message>> = Vec::with_capacity(cols);
        for _ in 0..cols {
            if let Some(el) = iter.next() {
                line.push(el);
            }
        }
        if line.is_empty() {
            break;
        }
        while line.len() < cols {
            line.push(widget::space::horizontal().width(Length::Fill).into());
        }
        rows.push(
            widget::row::with_children(line)
                .spacing(10)
                .width(Length::Fill)
                .into(),
        );
    }
    widget::column::with_children(rows)
        .spacing(10)
        .width(Length::Fill)
        .into()
}

fn monitor_panel_icon(panel: MonitorPanel) -> &'static str {
    match panel {
        MonitorPanel::Energy => "battery-good-symbolic",
        MonitorPanel::Cpu => "computer-symbolic",
        MonitorPanel::Gpu => "video-display-symbolic",
        MonitorPanel::Memory => "media-memory-symbolic",
        MonitorPanel::Disks => "drive-harddisk-symbolic",
        MonitorPanel::Network => "network-wired-symbolic",
        MonitorPanel::Sensors => "weather-clear-symbolic",
        MonitorPanel::Processes => "utilities-system-monitor-symbolic",
        MonitorPanel::System => "emblem-system-symbolic",
    }
}

fn monitor_poll_for(panel: MonitorPanel) -> Option<MonitorPollComponent> {
    match panel {
        MonitorPanel::Cpu => Some(MonitorPollComponent::Cpu),
        MonitorPanel::Gpu => Some(MonitorPollComponent::Gpu),
        MonitorPanel::Memory => Some(MonitorPollComponent::Ram),
        MonitorPanel::Disks => Some(MonitorPollComponent::Disk),
        MonitorPanel::Network => Some(MonitorPollComponent::Net),
        MonitorPanel::Processes => Some(MonitorPollComponent::Procs),
        MonitorPanel::Energy | MonitorPanel::Sensors | MonitorPanel::System => None,
    }
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

    // Reorderable system bar icons split into two clean phone-style grids:
    // 1. Active/visible icons on the bar (in exact bar order, draggable)
    // 2. Hidden (`Nunca`) icons tucked cleanly underneath (1 click to add back)
    let ordered = app.panel_icons.ordered(&app.reorderable_bar_items());
    let dragging = app.dragging_bar;
    let legend = widget::row::with_children(vec![
        widget::text::caption("Clique no card para alternar · Arraste pelos pontos para reordenar")
            .class(theme::Text::Custom(dimmed))
            .width(Length::Fill)
            .into(),
        tone_pill("Sempre", TileTone::Teal),
        tone_pill("Ativo", TileTone::Amber),
        tone_pill("Nunca", TileTone::Muted),
    ])
    .spacing(6)
    .align_y(Alignment::Center);

    let make_bar_tile = |item: crate::central::layout::BarItem| -> Element<'_, Message> {
        let current = app.panel_icons.get(item);
        let choices = item.choices();
        let next = choices
            .iter()
            .position(|&c| c == current)
            .map(|idx| choices[(idx + 1) % choices.len()])
            .unwrap_or(choices[0]);
        let (tone, status_label) = match current {
            BarShow::Always => (TileTone::Teal, "Sempre"),
            BarShow::Active => (TileTone::Amber, "Ativo"),
            BarShow::Never => (TileTone::Muted, "Nunca"),
        };
        draggable_grid_tile(
            item.icon(),
            item.name(),
            status_label,
            tone,
            dragging == Some(item),
            dragging.is_some(),
            Message::SetBarIcon(item, next),
            Message::DragBarStart(item),
            Message::DragBarOver(item),
            None,
        )
    };

    let (visible_items, hidden_items): (Vec<_>, Vec<_>) = ordered
        .into_iter()
        .partition(|&item| app.panel_icons.get(item) != BarShow::Never);

    let mut bar_card_children: Vec<Element<'_, Message>> = vec![
        widget::text::heading("Ícones na barra (na ordem exibida)").into(),
        legend.into(),
    ];
    if visible_items.is_empty() {
        bar_card_children.push(
            widget::text::caption("Nenhum ícone ativo — clique em um card abaixo para adicionar.")
                .class(theme::Text::Custom(dimmed))
                .into(),
        );
    } else {
        let visible_tiles: Vec<Element<'_, Message>> =
            visible_items.into_iter().map(&make_bar_tile).collect();
        bar_card_children.push(tile_grid(visible_tiles, 4));
    }

    if !hidden_items.is_empty() {
        bar_card_children.push(widget::divider::horizontal::default().into());
        bar_card_children.push(
            widget::text::caption("Ocultos da barra (clique para ativar)")
                .font(cosmic::font::semibold())
                .class(theme::Text::Custom(dimmed))
                .into(),
        );
        let hidden_tiles: Vec<Element<'_, Message>> =
            hidden_items.into_iter().map(&make_bar_tile).collect();
        bar_card_children.push(tile_grid(hidden_tiles, 4));
    }

    items.push(card_box(
        widget::column::with_children(bar_card_children).spacing(spacing.space_s),
    ));

    // Saved Tray SNI hidden apps
    let mut tray_rows: Vec<Element<'_, Message>> = vec![
        widget::text::heading("Apps em segundo plano ocultos da barra").into(),
    ];
    if app.tray_config.hidden.is_empty() {
        tray_rows.push(
            widget::text::caption(
                "Nenhum app oculto — novos apps aparecem na barra por padrão.",
            )
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

fn monitor_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let mut items: Vec<Element<'_, Message>> = vec![
        widget::row::with_children(vec![
            widget::text::title3("Monitor (Painel da Esquerda)")
                .width(Length::Fill)
                .into(),
            action_btn("Restaurar padrão", Message::MonitorReset, false),
        ])
        .align_y(Alignment::Center)
        .into(),
    ];

    // 1. Metrics on the bar
    let mut bar_rows: Vec<Element<'_, Message>> = vec![
        widget::text::heading("Métricas exibidas na barra").into(),
        widget::text::caption(
            "Limite padrão: ≥ 70% de uso ou ≥ 65 °C (memória: ≥ 70%). Some 10 s após normalizar.",
        )
        .class(theme::Text::Custom(dimmed))
        .into(),
    ];
    for item in MonitorBarItem::ALL {
        let control: Element<'_, Message> = if item == MonitorBarItem::Power {
            status_badge(true, "Sempre visível", "")
        } else if item.has_modes() {
            let current = app.monitor_settings.bar_mode(item);
            widget::row::with_children(
                MonitorBarMode::ALL
                    .into_iter()
                    .map(|mode| small_chip(mode.name(), current == mode, Message::MonitorBarMode(item, mode)))
                    .collect::<Vec<_>>(),
            )
            .spacing(4)
            .into()
        } else {
            widget::toggler(app.monitor_settings.shows_on_bar(item))
                .on_toggle(move |_| Message::MonitorToggleBar(item))
                .into()
        };
        bar_rows.push(
            widget::row::with_children(vec![
                widget::text::body(item.name()).width(Length::Fill).into(),
                control,
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
        );
    }

    // Thermometer mode
    let thermo_chips = widget::row::with_children(
        MonitorBarMode::ALL
            .into_iter()
            .map(|mode| {
                small_chip(
                    mode.name(),
                    app.monitor_settings.thermometer == mode,
                    Message::MonitorThermometer(mode),
                )
            })
            .collect::<Vec<_>>(),
    )
    .spacing(4);
    bar_rows.push(
        widget::row::with_children(vec![
            widget::text::body("Termômetro (CPU e GPU)").width(Length::Fill).into(),
            thermo_chips.into(),
        ])
        .spacing(10)
        .align_y(Alignment::Center)
        .into(),
    );

    bar_rows.push(widget::divider::horizontal::default().into());

    // Background poll interval
    let poll_chips = widget::row::with_children(
        [(1u64, "1 s"), (2, "2 s"), (5, "5 s (padrão)"), (10, "10 s")]
            .into_iter()
            .map(|(secs, lbl)| {
                small_chip(
                    lbl,
                    app.monitor_settings.poll_secs == secs,
                    Message::MonitorPollSecs(secs),
                )
            })
            .collect::<Vec<_>>(),
    )
    .spacing(4);
    bar_rows.push(
        widget::row::with_children(vec![
            widget::text::body("Intervalo de leitura na barra (popup fechado)")
                .width(Length::Fill)
                .into(),
            poll_chips.into(),
        ])
        .spacing(10)
        .align_y(Alignment::Center)
        .into(),
    );
    items.push(card_box(widget::column::with_children(bar_rows).spacing(spacing.space_xs)));

    // 2. Cards order & 4 display modes as a 3×3 phone-style color-coded tile grid
    let dragging = app.dragging_monitor_card;
    let mon_legend = widget::row::with_children(vec![
        widget::text::caption("Clique no card para alternar o modo · Arraste pelos pontos para reordenar")
            .class(theme::Text::Custom(dimmed))
            .width(Length::Fill)
            .into(),
        tone_pill("Expandido", TileTone::Teal),
        tone_pill("Compacto", TileTone::Blue),
        tone_pill("Reduzido", TileTone::Amber),
        tone_pill("Oculto", TileTone::Muted),
    ])
    .spacing(6)
    .align_y(Alignment::Center);

    let mon_tiles: Vec<Element<'_, Message>> = app
        .monitor_settings
        .order
        .iter()
        .copied()
        .map(|panel| {
            let current = app.monitor_settings.card_mode(panel);
            let next = match current {
                MonitorCardMode::Full => MonitorCardMode::Compact,
                MonitorCardMode::Compact => MonitorCardMode::Minimal,
                MonitorCardMode::Minimal => MonitorCardMode::Hidden,
                MonitorCardMode::Hidden => MonitorCardMode::Full,
            };
            let (tone, status_label) = match current {
                MonitorCardMode::Full => (TileTone::Teal, "Expandido"),
                MonitorCardMode::Compact => (TileTone::Blue, "Compacto"),
                MonitorCardMode::Minimal => (TileTone::Amber, "Reduzido"),
                MonitorCardMode::Hidden => (TileTone::Muted, "Oculto"),
            };
            let poll_footer = monitor_poll_for(panel).map(|comp| {
                let secs = app.monitor_settings.component_poll(comp).clamp(1, 10);
                widget::row::with_children(vec![
                    widget::text::caption(format!("{secs}s"))
                        .size(11)
                        .class(theme::Text::Custom(dimmed))
                        .width(Length::Fixed(26.0))
                        .into(),
                    widget::slider(1.0_f32..=10.0_f32, secs as f32, move |v| {
                        Message::MonitorComponentPoll(comp, v.round() as u64)
                    })
                    .step(1.0_f32)
                    .width(Length::Fill)
                    .into(),
                ])
                .spacing(6)
                .align_y(Alignment::Center)
                .into()
            });
            draggable_grid_tile(
                monitor_panel_icon(panel),
                panel.name(),
                status_label,
                tone,
                dragging == Some(panel),
                dragging.is_some(),
                Message::MonitorCardMode(panel, next),
                Message::MonitorDragStart(panel),
                Message::MonitorDragOver(panel),
                poll_footer,
            )
        })
        .collect();

    let weather_poll = app.monitor_settings.component_poll(MonitorPollComponent::Weather);
    let weather_chips: Vec<Element<'_, Message>> = WEATHER_MINS_OPTIONS
        .into_iter()
        .map(|mins| {
            button::custom(widget::text::caption(format!("{mins} min")))
                .padding([4, 9])
                .class(chip_class(weather_poll == mins))
                .on_press(Message::MonitorComponentPoll(MonitorPollComponent::Weather, mins))
                .into()
        })
        .collect();

    items.push(card_box(
        widget::column::with_children(vec![
            widget::text::heading("Cartões do painel da esquerda").into(),
            mon_legend.into(),
            tile_grid(mon_tiles, 3),
            widget::divider::horizontal::default().into(),
            widget::row::with_children(vec![
                widget::text::caption("Atualização do Clima")
                    .class(theme::Text::Custom(dimmed))
                    .width(Length::Fill)
                    .into(),
                widget::row::with_children(weather_chips).spacing(4).into(),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
        ])
        .spacing(spacing.space_s),
    ));

    // 4. Processes count & sort column
    let count_chips = widget::row::with_children(
        PROCS_COUNT_OPTIONS
            .into_iter()
            .map(|n| {
                button::custom(widget::text::caption(n.to_string()))
                    .padding([5, 10])
                    .class(chip_class(app.monitor_settings.procs_count == n))
                    .on_press(Message::MonitorProcsCount(n))
                    .into()
            })
            .collect::<Vec<_>>(),
    )
    .spacing(4);
    let sort_chips = widget::row::with_children(
        MonitorProcSort::ALL
            .into_iter()
            .map(|sort| {
                let active = app.monitor_settings.proc_sort == sort;
                let lbl = if active {
                    let arrow = if app.monitor_settings.proc_sort_asc { "▲" } else { "▼" };
                    format!("{} {arrow}", sort.name())
                } else {
                    sort.name().to_owned()
                };
                button::custom(widget::text::caption(lbl))
                    .padding([5, 10])
                    .class(chip_class(active))
                    .on_press(Message::MonitorProcSort(sort))
                    .into()
            })
            .collect::<Vec<_>>(),
    )
    .spacing(4);
    items.push(card_box(
        widget::column::with_children(vec![
            widget::text::heading("Processos").into(),
            widget::row::with_children(vec![
                widget::text::body("Quantidade de processos exibidos")
                    .width(Length::Fill)
                    .into(),
                count_chips.into(),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::text::body("Ordenar tabela por (clique novamente para inverter)")
                    .width(Length::Fill)
                    .into(),
                sort_chips.into(),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
        ])
        .spacing(spacing.space_xs),
    ));

    // 5. Weather location & card display mode
    let place = &app.weather_place;
    let current_place = if place.detail.is_empty() {
        format!("{} ({:.3}, {:.3})", place.name, place.lat, place.lon)
    } else {
        format!("{} — {} ({:.3}, {:.3})", place.name, place.detail, place.lat, place.lon)
    };
    let weather_mode_chips = widget::row::with_children(
        MonitorCardMode::ALL
            .into_iter()
            .map(|mode| {
                small_chip(
                    mode.name(),
                    app.monitor_settings.weather_mode() == mode,
                    Message::MonitorWeatherMode(mode),
                )
            })
            .collect::<Vec<_>>(),
    )
    .spacing(4);
    let mut weather_rows: Vec<Element<'_, Message>> = vec![
        widget::text::heading("Clima").into(),
        kv_row("Local configurado", current_place),
        widget::row::with_children(vec![
            widget::text_input(
                "Digite CEP (ex: 30330-330) ou nome da cidade".to_owned(),
                app.weather_query.clone(),
            )
            .on_input(Message::WeatherQueryInput)
            .on_submit(|_| Message::WeatherSearch)
            .width(Length::Fill)
            .into(),
            action_btn("Buscar", Message::WeatherSearch, true),
        ])
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
    ];
    if app.weather_searching {
        weather_rows.push(
            widget::text::caption("Procurando localidade...")
                .class(theme::Text::Custom(dimmed))
                .into(),
        );
    } else if let Some(results) = &app.weather_results {
        if results.is_empty() {
            weather_rows.push(
                widget::text::caption("Nenhum local encontrado para essa busca.")
                    .class(theme::Text::Custom(dimmed))
                    .into(),
            );
        } else {
            for candidate in results {
                let text = if candidate.detail.is_empty() {
                    candidate.name.clone()
                } else {
                    format!("{} — {}", candidate.name, candidate.detail)
                };
                weather_rows.push(
                    button::custom(widget::text::body(text))
                        .padding([6, 10])
                        .width(Length::Fill)
                        .class(chip_class(false))
                        .on_press(Message::WeatherPickPlace(candidate.clone()))
                        .into(),
                );
            }
        }
    }
    weather_rows.push(
        widget::row::with_children(vec![
            widget::text::body("Modo do cartão de clima")
                .width(Length::Fill)
                .into(),
            weather_mode_chips.into(),
        ])
        .spacing(10)
        .align_y(Alignment::Center)
        .into(),
    );
    items.push(card_box(widget::column::with_children(weather_rows).spacing(spacing.space_xs)));

    // 4. Energy tariff & base rest watts
    items.push(card_box(
        widget::column::with_children(vec![
            widget::text::heading("Cálculo de consumo elétrico").into(),
            widget::row::with_children(vec![
                widget::column::with_children(vec![
                    widget::text::caption("Tarifa (R$ por kWh)")
                        .class(theme::Text::Custom(dimmed))
                        .into(),
                    widget::text_input("0,95".to_owned(), app.tariff_input.clone())
                        .on_input(Message::MonitorTariffInput)
                        .width(Length::Fill)
                        .into(),
                ])
                .spacing(4)
                .width(Length::Fill)
                .into(),
                widget::column::with_children(vec![
                    widget::text::caption("Resto do PC (W)")
                        .class(theme::Text::Custom(dimmed))
                        .into(),
                    widget::text_input("40".to_owned(), app.rest_watts_input.clone())
                        .on_input(Message::MonitorRestWattsInput)
                        .width(Length::Fill)
                        .into(),
                ])
                .spacing(4)
                .width(Length::Fill)
                .into(),
            ])
            .spacing(12)
            .into(),
            widget::text::caption(
                "Estimativa do restante do hardware (placa-mãe, RAM, SSDs, ventoinhas e perdas da fonte).",
            )
            .class(theme::Text::Custom(dimmed))
            .into(),
        ])
        .spacing(spacing.space_xs),
    ));

    widget::column::with_children(items).spacing(spacing.space_m).into()
}

fn central_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let mut items: Vec<Element<'_, Message>> = vec![
        widget::row::with_children(vec![
            widget::text::title3("Central (Painel da Direita)")
                .width(Length::Fill)
                .into(),
            action_btn("Restaurar tiles padrão", Message::CentralResetTiles, false),
        ])
        .align_y(Alignment::Center)
        .into(),
    ];

    // 1. Appearance & Do Not Disturb
    items.push(card_box(
        widget::column::with_children(vec![
            widget::text::heading("Aparência & Notificações").into(),
            toggle_row(
                "Modo Escuro (Dark Mode do COSMIC)",
                app.dark_mode,
                Message::CentralToggleDarkMode,
            ),
            toggle_row(
                "Não Perturbe (Do Not Disturb)",
                app.do_not_disturb,
                Message::CentralToggleDnd,
            ),
        ])
        .spacing(spacing.space_xs),
    ));

    // 2. Collapsible sections in Central
    let section_name = |sec: CentralSection| match sec {
        CentralSection::Notifications => "Notificações",
        CentralSection::Media => "Reprodutor de Mídia",
        CentralSection::Calendar => "Calendário",
        CentralSection::Clipboard => "Área de transferência",
    };
    let mut sec_rows: Vec<Element<'_, Message>> = vec![
        widget::text::heading("Seções recolhíveis da Central").into(),
        widget::text::caption(
            "Escolha quais seções abrem expandidas ou recolhidas por padrão no painel da direita.",
        )
        .class(theme::Text::Custom(dimmed))
        .into(),
    ];
    for sec in CentralSection::ALL {
        let expanded = !app.central_layout.is_folded(sec);
        sec_rows.push(
            widget::row::with_children(vec![
                widget::icon::icon(icon(sec.icon())).size(16).into(),
                widget::text::body(section_name(sec)).width(Length::Fill).into(),
                status_badge(expanded, "Expandida", "Recolhida"),
                widget::toggler(expanded)
                    .on_toggle(move |_| Message::CentralToggleSectionFold(sec))
                    .into(),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
        );
    }
    items.push(card_box(widget::column::with_children(sec_rows).spacing(spacing.space_xs)));

    // 3. Tiling (cosmic-comp)
    items.push(card_box(
        widget::column::with_children(vec![
            widget::text::heading("Ladrilhamento (Tiling)").into(),
            toggle_row(
                "Ladrilhar novos espaços de trabalho automaticamente",
                app.tiling_config.new_workspaces_tile,
                Message::CentralToggleNewWorkspacesTile,
            ),
            toggle_row(
                "Destacar borda da janela ativa (Active Hint)",
                app.tiling_config.active_hint,
                Message::CentralToggleActiveHint,
            ),
        ])
        .spacing(spacing.space_xs),
    ));

    // 4. Touchpad & Hot Edge
    let mut input_rows: Vec<Element<'_, Message>> = vec![
        widget::text::heading("Entrada & Extensões").into(),
    ];
    if let Some(tp) = app.touchpad {
        let tp_chips = widget::row::with_children(vec![
            small_chip(
                "Ativado",
                tp == Touchpad::Enabled,
                Message::CentralSetTouchpad(Touchpad::Enabled),
            ),
            small_chip(
                "Desligar c/ mouse",
                tp == Touchpad::DisabledOnExternalMouse,
                Message::CentralSetTouchpad(Touchpad::DisabledOnExternalMouse),
            ),
            small_chip(
                "Desativado",
                tp == Touchpad::Disabled,
                Message::CentralSetTouchpad(Touchpad::Disabled),
            ),
        ])
        .spacing(4);
        input_rows.push(
            widget::row::with_children(vec![
                widget::text::body("Touchpad").width(Length::Fill).into(),
                tp_chips.into(),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
        );
    } else {
        input_rows.push(
            widget::text::caption("Nenhuma configuração de touchpad disponível neste sistema.")
                .class(theme::Text::Custom(dimmed))
                .into(),
        );
    }
    input_rows.push(widget::divider::horizontal::default().into());
    if services::hot_edge_installed() {
        input_rows.push(
            widget::row::with_children(vec![
                widget::text::body("Hot Edge (cosmic-ext-hot-edge)")
                    .width(Length::Fill)
                    .into(),
                action_btn("Configurar", Message::CentralOpenHotEdge, false),
                widget::toggler(app.hot_edge)
                    .on_toggle(|_| Message::CentralToggleHotEdge)
                    .into(),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
        );
    } else {
        input_rows.push(
            widget::row::with_children(vec![
                widget::text::body("Hot Edge (cosmic-ext-hot-edge)")
                    .width(Length::Fill)
                    .into(),
                status_badge(false, "Instalado", "Não instalado"),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
        );
    }
    items.push(card_box(widget::column::with_children(input_rows).spacing(spacing.space_xs)));

    // 5. Reset tiles layout
    items.push(card_box(
        widget::column::with_children(vec![
            widget::text::heading("Grade de atalhos rápidos (Tiles)").into(),
            widget::text::caption(
                "Restaura os botões rápidos da Central (Brilho, Wi-Fi, Não Perturbe, Bluetooth, Energia, Modo Escuro, etc.) para o arranjo padrão.",
            )
            .class(theme::Text::Custom(dimmed))
            .into(),
            widget::row::with_children(vec![action_btn(
                "Restaurar layout padrão dos tiles",
                Message::CentralResetTiles,
                false,
            )])
            .into(),
        ])
        .spacing(spacing.space_xs),
    ));

    widget::column::with_children(items).spacing(spacing.space_m).into()
}

fn color_swatch<'a>(hex: &str, fallback_accent: bool) -> Element<'a, Message> {
    let parsed = parse_hex_color(hex);
    widget::container(widget::space::horizontal())
        .width(Length::Fixed(24.0))
        .height(Length::Fixed(24.0))
        .class(theme::Container::custom(move |theme| {
            let color = match parsed {
                Some([r, g, b]) => Color::from_rgb(r, g, b),
                None if fallback_accent => theme.cosmic().accent_color().into(),
                None => card(theme.cosmic()).base.into(),
            };
            widget::container::Style {
                background: Some(Background::Color(color)),
                border: iced::Border {
                    radius: 6.0.into(),
                    width: 1.0,
                    color: theme.cosmic().background(theme.transparent).divider.into(),
                },
                ..Default::default()
            }
        }))
        .into()
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
        .spacing(6)
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
        card_rows.push(widget::row::with_children(vec![mk_btn("Abrir"), mk_btn("Dispensar")]).spacing(6).into());
    }

    let toast_card = widget::container(widget::column::with_children(card_rows).spacing(4))
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
        .height(Length::Fixed(185.0))
        .padding(14)
        .align_x(align_x)
        .align_y(align_y)
        .class(theme::Container::custom(|_| widget::container::Style {
            background: Some(Background::Color(Color::from_rgb(0.09, 0.11, 0.16))),
            border: iced::Border {
                radius: 12.0.into(),
                width: 1.0,
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.12),
            },
            ..Default::default()
        }));

    card_box(
        widget::column::with_children(vec![
            widget::row::with_children(vec![
                widget::text::heading("Prévia ao vivo").width(Length::Fill).into(),
                widget::text::caption(format!(
                    "{} · {} · {:.0}%",
                    cfg.position.label(),
                    cfg.format.label(),
                    cfg.opacity * 100.0
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

fn notifications_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let cfg = &app.notif_config;

    let mut items: Vec<Element<'_, Message>> = vec![
        widget::row::with_children(vec![
            widget::text::title3("Notificações").width(Length::Fill).into(),
            action_btn("Restaurar padrão", Message::NotifReset, false),
            action_btn(
                "Testar OSD",
                Message::RunAction(
                    "Teste OSD",
                    "notify-send -a logi-osd -i logi-backlight -h string:x-estilo:osd -h string:x-tom:normal -h int:x-pontos:5 -h int:x-pontos-total:7 -h boolean:transient:true -t 2000 'Luz do teclado' ''".to_owned(),
                    false,
                ),
                false,
            ),
            action_btn("Enviar notificação de teste", Message::NotifSendTest, true),
        ])
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
        notif_live_preview(cfg),
    ];

    // 1. Duration, Format & Position
    let timeout_chips = widget::row::with_children(
        TIMEOUT_OPTIONS
            .into_iter()
            .map(|secs| {
                let lbl = if secs == 0 {
                    "Persistente".to_owned()
                } else {
                    format!("{secs}s")
                };
                button::custom(widget::text::caption(lbl))
                    .padding([5, 10])
                    .class(chip_class(cfg.timeout_secs == secs))
                    .on_press(Message::NotifTimeout(secs))
                    .into()
            })
            .collect::<Vec<_>>(),
    )
    .spacing(4);

    let format_chips = widget::row::with_children(
        NotifFormat::ALL
            .into_iter()
            .map(|fmt| small_chip(fmt.label(), cfg.format == fmt, Message::NotifFormat(fmt)))
            .collect::<Vec<_>>(),
    )
    .spacing(4);

    let position_chips = widget::row::with_children(
        NotifPosition::ALL
            .into_iter()
            .map(|pos| small_chip(pos.label(), cfg.position == pos, Message::NotifPosition(pos)))
            .collect::<Vec<_>>(),
    )
    .spacing(4);

    items.push(card_box(
        widget::column::with_children(vec![
            widget::text::heading("Comportamento & Posição").into(),
            widget::row::with_children(vec![
                widget::text::body("Tempo exibido").width(Length::Fill).into(),
                timeout_chips.into(),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::text::body("Formato dos cartões").width(Length::Fill).into(),
                format_chips.into(),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::text::body("Posição na tela").width(Length::Fill).into(),
                position_chips.into(),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            widget::text::caption(
                "A posição e o tempo limite são sincronizados com o daemon de notificações do COSMIC e aplicados imediatamente na Central.",
            )
            .class(theme::Text::Custom(dimmed))
            .into(),
        ])
        .spacing(spacing.space_xs),
    ));

    // 2. Transparency, Corner radius, Body lines & Content toggles
    let lines_chips = widget::row::with_children(
        (notif_cfg::MIN_BODY_LINES..=notif_cfg::MAX_BODY_LINES)
            .map(|n| {
                button::custom(widget::text::caption(n.to_string()))
                    .padding([5, 10])
                    .class(chip_class(cfg.max_body_lines == n))
                    .on_press(Message::NotifMaxBodyLines(n))
                    .into()
            })
            .collect::<Vec<_>>(),
    )
    .spacing(4);

    items.push(card_box(
        widget::column::with_children(vec![
            widget::text::heading("Aparência & Conteúdo").into(),
            widget::row::with_children(vec![
                widget::text::caption(format!("Transparência (opacidade): {:.0}%", cfg.opacity * 100.0))
                    .width(Length::Fixed(210.0))
                    .into(),
                widget::slider(40.0..=100.0, (cfg.opacity * 100.0).round(), |v| {
                    Message::NotifOpacity(v / 100.0)
                })
                .step(1.0_f32)
                .width(Length::Fill)
                .into(),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::text::caption(format!("Raio da borda: {:.0} px", cfg.corner_radius))
                    .width(Length::Fixed(210.0))
                    .into(),
                widget::slider(
                    notif_cfg::MIN_CORNER_RADIUS..=notif_cfg::MAX_CORNER_RADIUS,
                    cfg.corner_radius,
                    Message::NotifCornerRadius,
                )
                .step(1.0_f32)
                .width(Length::Fill)
                .into(),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::text::body("Linhas máximas do corpo")
                    .width(Length::Fill)
                    .into(),
                lines_chips.into(),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            widget::divider::horizontal::default().into(),
            toggle_row(
                "Mostrar corpo da mensagem",
                cfg.show_body,
                Message::NotifToggleShowBody,
            ),
            toggle_row(
                "Mostrar botões de ação",
                cfg.show_actions,
                Message::NotifToggleShowActions,
            ),
            toggle_row(
                "Agrupar notificações por aplicativo",
                cfg.group_by_app,
                Message::NotifToggleGroupByApp,
            ),
        ])
        .spacing(spacing.space_xs),
    ));

    // 3. Colors (Accent + Background: presets + hex + system theme)
    let accent_presets = [
        ("", "Tema do sistema"),
        ("#3584E4", "Azul"),
        ("#2EC27E", "Verde"),
        ("#00BCD4", "Ciano"),
        ("#FF7800", "Laranja"),
        ("#9141AC", "Roxo"),
        ("#E01B24", "Vermelho"),
    ];
    let bg_presets = [
        ("", "Tema do sistema"),
        ("#111318", "Preto"),
        ("#1E2028", "Grafite"),
        ("#141C2B", "Azul noite"),
        ("#221C2A", "Roxo escuro"),
        ("#F3F4F6", "Claro"),
    ];
    let accent_chips = widget::row::with_children(
        accent_presets
            .into_iter()
            .map(|(hex, lbl)| {
                small_chip(
                    lbl,
                    cfg.accent_hex.eq_ignore_ascii_case(hex),
                    Message::NotifAccentHex(hex.to_owned()),
                )
            })
            .collect::<Vec<_>>(),
    )
    .spacing(4);
    let bg_chips = widget::row::with_children(
        bg_presets
            .into_iter()
            .map(|(hex, lbl)| {
                small_chip(
                    lbl,
                    cfg.bg_hex.eq_ignore_ascii_case(hex),
                    Message::NotifBgHex(hex.to_owned()),
                )
            })
            .collect::<Vec<_>>(),
    )
    .spacing(4);

    items.push(card_box(
        widget::column::with_children(vec![
            widget::text::heading("Cores personalizadas").into(),
            widget::text::body("Cor de destaque").into(),
            accent_chips.into(),
            widget::row::with_children(vec![
                color_swatch(&cfg.accent_hex, true),
                widget::text_input(
                    "Vazio = tema do sistema, ou #RRGGBB".to_owned(),
                    cfg.accent_hex.clone(),
                )
                .on_input(Message::NotifAccentHex)
                .width(Length::Fill)
                .into(),
                small_chip(
                    "Usar tema do sistema",
                    cfg.accent_hex.is_empty(),
                    Message::NotifAccentHex(String::new()),
                ),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            widget::divider::horizontal::default().into(),
            widget::text::body("Cor de fundo dos cartões").into(),
            bg_chips.into(),
            widget::row::with_children(vec![
                color_swatch(&cfg.bg_hex, false),
                widget::text_input(
                    "Vazio = tema do sistema, ou #RRGGBB".to_owned(),
                    cfg.bg_hex.clone(),
                )
                .on_input(Message::NotifBgHex)
                .width(Length::Fill)
                .into(),
                small_chip(
                    "Usar tema do sistema",
                    cfg.bg_hex.is_empty(),
                    Message::NotifBgHex(String::new()),
                ),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
        ])
        .spacing(spacing.space_xs),
    ));

    widget::column::with_children(items).spacing(spacing.space_m).into()
}

fn devices_osd_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let cfg = &app.osd_config;
    let service_active = app
        .snapshot
        .units
        .iter()
        .find(|u| u.unit == "logi-osd.service")
        .is_some_and(|u| u.active);
    let dur_ms = cfg.duration.as_millis() as u64;
    let interval_mins = (cfg.legacy_battery_interval.as_secs() / 60).max(1);

    let mut items: Vec<Element<'_, Message>> = vec![
        widget::row::with_children(vec![
            widget::text::title3("OSD de Dispositivos").width(Length::Fill).into(),
            action_btn("Testar OSD", Message::OsdRunDemo, false),
            action_btn("Salvar e reiniciar", Message::OsdSaveAndRestart, true),
        ])
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
    ];

    // Service status card
    items.push(card_box(
        widget::column::with_children(vec![
            widget::row::with_children(vec![
                widget::text::heading("Serviço de OSD (entrada/saída e periféricos)")
                    .width(Length::Fill)
                    .into(),
                status_badge(service_active, "Ativo", "Inativo"),
                action_btn("Reiniciar serviço", Message::RestartUserUnit("logi-osd.service"), false),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            widget::text::caption(
                "Exibe avisos instantâneos para Caps/Num/Scroll Lock, troca de host/DPI, bateria de periféricos e atalhos de teclado/mouse.",
            )
            .class(theme::Text::Custom(dimmed))
            .into(),
            kv_row("Unidade systemd", "logi-osd.service".to_owned()),
            kv_row(
                "Arquivo de configuração",
                osd_config::config_path().to_string_lossy().into_owned(),
            ),
            widget::row::with_children(vec![
                widget::text::caption("Posição e estilo visual das pílulas seguem a aba Notificações.")
                    .class(theme::Text::Custom(dimmed))
                    .width(Length::Fill)
                    .into(),
                small_chip(
                    "Abrir Notificações",
                    false,
                    Message::SelectSection(Section::Notifications),
                ),
            ])
            .align_y(Alignment::Center)
            .into(),
        ])
        .spacing(spacing.space_xs),
    ));

    // OSD Duration & Battery alerts
    let dur_presets = widget::row::with_children(
        [(800u64, "0.8s"), (1500, "1.5s"), (2500, "2.5s"), (4000, "4s"), (6000, "6s")]
            .into_iter()
            .map(|(ms, lbl)| small_chip(lbl, dur_ms == ms, Message::OsdDurationMs(ms)))
            .collect::<Vec<_>>(),
    )
    .spacing(4);
    let interval_presets = widget::row::with_children(
        [(5u64, "5 min"), (15, "15 min"), (30, "30 min"), (60, "1 h")]
            .into_iter()
            .map(|(m, lbl)| small_chip(lbl, interval_mins == m, Message::OsdBatteryIntervalMins(m)))
            .collect::<Vec<_>>(),
    )
    .spacing(4);

    items.push(card_box(
        widget::column::with_children(vec![
            widget::text::heading("Duração & Alertas de Bateria").into(),
            widget::row::with_children(vec![
                widget::text::caption(format!("Duração da pílula OSD: {dur_ms} ms"))
                    .width(Length::Fixed(220.0))
                    .into(),
                widget::slider(200.0..=10000.0, dur_ms.min(10000) as f32, |v| {
                    Message::OsdDurationMs(((v / 100.0).round() as u64) * 100)
                })
                .step(100.0_f32)
                .width(Length::Fill)
                .into(),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::text::caption("Atalhos de duração")
                    .class(theme::Text::Custom(dimmed))
                    .width(Length::Fill)
                    .into(),
                dur_presets.into(),
            ])
            .align_y(Alignment::Center)
            .into(),
            widget::divider::horizontal::default().into(),
            widget::row::with_children(vec![
                widget::text::caption(format!("Alerta de bateria baixa: {}%", cfg.battery_low))
                    .width(Length::Fixed(220.0))
                    .into(),
                widget::slider(5.0..=50.0, cfg.battery_low.clamp(5, 50) as f32, |v| {
                    Message::OsdBatteryLow(v.round() as u8)
                })
                .step(1.0_f32)
                .width(Length::Fill)
                .into(),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::text::caption(format!("Alerta de bateria crítica: {}%", cfg.battery_critical))
                    .width(Length::Fixed(220.0))
                    .into(),
                widget::slider(
                    1.0..=cfg.battery_low.saturating_sub(1).max(1) as f32,
                    cfg.battery_critical as f32,
                    |v| Message::OsdBatteryCritical(v.round() as u8),
                )
                .step(1.0_f32)
                .width(Length::Fill)
                .into(),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::text::body(format!(
                    "Intervalo de verificação de bateria ({interval_mins} min)"
                ))
                .width(Length::Fill)
                .into(),
                interval_presets.into(),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
        ])
        .spacing(spacing.space_xs),
    ));

    // Emoji key command & Save/Reset
    items.push(card_box(
        widget::column::with_children(vec![
            widget::text::heading("Tecla de Emoji & Ações").into(),
            widget::column::with_children(vec![
                widget::text::caption(
                    "Comando ao pressionar a tecla Emoji de periféricos suportados (vazio = padrão)",
                )
                .class(theme::Text::Custom(dimmed))
                .into(),
                widget::text_input(
                    "Ex: smile ou cosmic-launcher".to_owned(),
                    cfg.emoji_command.clone().unwrap_or_default(),
                )
                .on_input(Message::OsdEmojiCommand)
                .width(Length::Fill)
                .into(),
            ])
            .spacing(4)
            .into(),
            widget::row::with_children(vec![
                action_btn("Salvar rascunho", Message::OsdSaveOnly, false),
                action_btn("Restaurar padrão", Message::OsdReset, false),
            ])
            .spacing(8)
            .into(),
        ])
        .spacing(spacing.space_xs),
    ));

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
                widget::text::body("Chave física da câmera (Lenovo)")
                    .width(Length::Fill)
                    .into(),
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
            status_badge(
                cam.iriun_installed && cam.on,
                "Ativo",
                if cam.iriun_installed { "Desligado" } else { "Não instalado" },
            ),
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
            widget::text::caption(
                "Pacote opcional — instale pela aba \"Pacotes & Opcionais\" ou com ./scripts/bootstrap-pc.sh --with-iriun.",
            )
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
            widget::text::body("MAC Rotativo (mac-rotativo.timer)")
                .width(Length::Fill)
                .into(),
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
                    action_btn(
                        if on { "Desconectar" } else { "Conectar" },
                        Message::ConnectVpn(i),
                        !on,
                    ),
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
                        action_btn(
                            "Tornar principal",
                            Message::SetPrimaryDisplay(d.name.clone()),
                            false,
                        )
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
                        widget::slider(0.0..=100.0, pct, move |v| {
                            Message::SetDdcBrightness(idx, v)
                        })
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

    // Window tiling padding (gaps) & activation focus policy
    let (gap_outer, gap_inner) = app.window_gaps;
    let gap_presets: Vec<Element<'_, Message>> = [
        ((0, 0), "Sem folga (0 px)"),
        ((0, 4), "Compacto (4 px)"),
        ((4, 8), "Padrão COSMIC (8 px)"),
        ((8, 12), "Folgado (12 px)"),
    ]
    .into_iter()
    .map(|((o, i), lbl)| small_chip(lbl, gap_outer == o && gap_inner == i, Message::SetWindowGaps(o, i)))
    .collect();
    let px_options: [(u32, &'static str); 4] = [(0, "0 px"), (4, "4 px"), (8, "8 px"), (12, "12 px")];
    let outer_chips: Vec<Element<'_, Message>> = px_options
        .into_iter()
        .map(|(px, lbl)| small_chip(lbl, gap_outer == px, Message::SetWindowGaps(px, gap_inner)))
        .collect();
    let inner_chips: Vec<Element<'_, Message>> = px_options
        .into_iter()
        .map(|(px, lbl)| small_chip(lbl, gap_inner == px, Message::SetWindowGaps(gap_outer, px)))
        .collect();
    let policy_desc = match app.activation_policy.as_str() {
        "Urgent" => "Recomendado · Comandos automatizados e apps em segundo plano não saltam na sua frente; apenas acendem na barra.",
        "FocusIfActiveWorkspace" => "Traz a janela para frente apenas se ela já estiver na sua área de trabalho atual.",
        _ => "Padrão COSMIC · Qualquer app ou script que abrir janela pode roubar o foco imediatamente.",
    };
    items.push(card_box(
        widget::column::with_children(vec![
            widget::text::heading("Janelas: Modo Dividido & Prioridade de Foco").into(),
            widget::text::caption(
                "Controla a folga (padding) ao encaixar janelas lado a lado e impede que comandos automatizados fiquem pipocando na tela.",
            )
            .class(theme::Text::Custom(dimmed))
            .into(),
            widget::row::with_children(vec![
                widget::text::body("Folga no modo janela dividida").width(Length::Fill).into(),
                widget::row::with_children(gap_presets).spacing(6).into(),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::text::caption("Margem externa da tela")
                    .class(theme::Text::Custom(dimmed))
                    .width(Length::Fill)
                    .into(),
                widget::row::with_children(outer_chips).spacing(4).into(),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::text::caption("Espaço entre janelas divididas")
                    .class(theme::Text::Custom(dimmed))
                    .width(Length::Fill)
                    .into(),
                widget::row::with_children(inner_chips).spacing(4).into(),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            widget::divider::horizontal::default().into(),
            widget::row::with_children(vec![
                widget::text::body("Prioridade de foco de novas janelas").width(Length::Fill).into(),
                widget::row::with_children(vec![
                    small_chip(
                        "Não roubar foco (Recomendado)",
                        app.activation_policy == "Urgent",
                        Message::SetActivationPolicy("Urgent"),
                    ),
                    small_chip(
                        "Só na área atual",
                        app.activation_policy == "FocusIfActiveWorkspace",
                        Message::SetActivationPolicy("FocusIfActiveWorkspace"),
                    ),
                    small_chip(
                        "Sempre focar",
                        app.activation_policy == "Focus",
                        Message::SetActivationPolicy("Focus"),
                    ),
                ])
                .spacing(6)
                .into(),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            widget::text::caption(policy_desc)
                .class(theme::Text::Custom(dimmed))
                .into(),
        ])
        .spacing(spacing.space_s),
    ));

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
        ])
        .spacing(spacing.space_xs),
    ));

    // Screen snipping (Win+Shift+S / print-recorte)
    let sc = &app.screenshot_config;
    let mode_chips = widget::row::with_children(
        ScreenshotOpenMode::ALL
            .into_iter()
            .map(|m| small_chip(m.label(), sc.open_mode == m, Message::ScreenshotOpenMode(m)))
            .collect::<Vec<_>>(),
    )
    .spacing(4);
    let tool_chips = widget::row::with_children(
        screenshot_config::SATTY_TOOLS
            .iter()
            .map(|&(id, lbl)| {
                small_chip(
                    lbl,
                    sc.satty_tool == id,
                    Message::ScreenshotSattyTool(id.to_owned()),
                )
            })
            .collect::<Vec<_>>(),
    )
    .spacing(4);
    let color_chips = widget::row::with_children(
        screenshot_config::BORDER_COLOR_PRESETS
            .iter()
            .map(|&(hex, lbl)| {
                small_chip(
                    lbl,
                    sc.border_color.eq_ignore_ascii_case(hex),
                    Message::ScreenshotBorderColor(hex.to_owned()),
                )
            })
            .collect::<Vec<_>>(),
    )
    .spacing(4);
    let width_chips = widget::row::with_children(
        [1u32, 2, 3, 4, 6]
            .into_iter()
            .map(|w| {
                button::custom(widget::text::caption(format!("{w}px")))
                    .padding([4, 9])
                    .class(chip_class(sc.border_width == w))
                    .on_press(Message::ScreenshotBorderWidth(w))
                    .into()
            })
            .collect::<Vec<_>>(),
    )
    .spacing(4);
    let timeout_chips = widget::row::with_children(
        [(60u32, "1m"), (180, "3m"), (600, "10m"), (1800, "30m")]
            .into_iter()
            .map(|(s, lbl)| {
                small_chip(lbl, sc.notify_timeout_secs == s, Message::ScreenshotTimeoutSecs(s))
            })
            .collect::<Vec<_>>(),
    )
    .spacing(4);

    items.push(card_box(
        widget::column::with_children(vec![
            widget::row::with_children(vec![
                widget::text::heading("Recorte de tela (Win+Shift+S)")
                    .width(Length::Fill)
                    .into(),
                action_btn("Abrir pasta", Message::OpenScreenshotsDir, false),
                action_btn("Restaurar padrão", Message::ScreenshotReset, false),
                action_btn("Testar recorte", Message::RunScreenshot, true),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            widget::column::with_children(vec![
                widget::text::caption("Pasta de destino (vazio = ~/Imagens/Screenshots)")
                    .class(theme::Text::Custom(dimmed))
                    .into(),
                widget::text_input(
                    "Ex: ~/Imagens/Screenshots".to_owned(),
                    sc.save_dir.clone(),
                )
                .on_input(Message::ScreenshotSaveDirInput)
                .width(Length::Fill)
                .into(),
            ])
            .spacing(4)
            .into(),
            widget::row::with_children(vec![
                widget::text::body("Após capturar").width(Length::Fill).into(),
                mode_chips.into(),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::text::body("Ferramenta inicial do Satty").width(Length::Fill).into(),
                tool_chips.into(),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::text::body("Cor da borda de seleção").width(Length::Fill).into(),
                color_chips.into(),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::text::body("Espessura da borda").width(Length::Fill).into(),
                width_chips.into(),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::text::caption(format!("Escurecimento do fundo: {}%", sc.overlay_opacity))
                    .width(Length::Fixed(210.0))
                    .into(),
                widget::slider(0.0_f32..=85.0_f32, sc.overlay_opacity as f32, |v| {
                    Message::ScreenshotOverlayOpacity(v.round() as u32)
                })
                .step(5.0_f32)
                .width(Length::Fill)
                .into(),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
            widget::row::with_children(vec![
                widget::text::body("Tempo de espera das ações da notificação")
                    .width(Length::Fill)
                    .into(),
                timeout_chips.into(),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            widget::divider::horizontal::default().into(),
            toggle_row(
                "Copiar captura automaticamente para a área de transferência",
                sc.copy_clipboard,
                Message::ScreenshotToggleCopy,
            ),
            toggle_row(
                "Tocar som ao capturar",
                sc.play_sound,
                Message::ScreenshotToggleSound,
            ),
            kv_row(
                "Arquivo de configuração",
                screenshot_config::ScreenshotConfig::config_path()
                    .to_string_lossy()
                    .into_owned(),
            ),
        ])
        .spacing(spacing.space_xs),
    ));

    widget::column::with_children(items).spacing(spacing.space_m).into()
}

fn login_live_preview<'a>(g: &'a data::GreeterConfig) -> Element<'a, Message> {
    let is_light = g.theme == "light";
    let accent_rgb = parse_hex_color(&g.accent_color);
    let dim = g.dim_opacity.clamp(0.0, 0.85);
    let card_op = g.card_opacity.clamp(0.0, 0.95);
    let card_rad = g.card_radius.clamp(4.0, 32.0);
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

    let clock_block: Element<'a, Message> = if g.style == "android" {
        let mut col: Vec<Element<'a, Message>> = vec![
            widget::text::body("23").size(clock_px).into(),
            widget::text::body("54").size(clock_px).into(),
        ];
        if g.show_date {
            col.push(widget::text::caption("seg, 23 nov").into());
        }
        widget::column::with_children(col)
            .spacing(0)
            .align_x(Alignment::Center)
            .into()
    } else {
        let mut col: Vec<Element<'a, Message>> = vec![widget::text::body(time_str).size(clock_px).into()];
        if g.show_date {
            col.push(widget::text::caption("segunda-feira, 23 de novembro").into());
        }
        let align = if g.style == "editorial" {
            Alignment::Start
        } else {
            Alignment::Center
        };
        widget::column::with_children(col)
            .spacing(2)
            .align_x(align)
            .into()
    };

    let mut login_parts: Vec<Element<'a, Message>> = Vec::new();
    if g.show_avatar {
        login_parts.push(
            widget::container(widget::icon::icon(icon("avatar-default-symbolic")).size(22))
                .width(Length::Fixed(40.0))
                .height(Length::Fixed(40.0))
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .class(theme::Container::custom(move |theme| {
                    let acc = match accent_rgb {
                        Some([r, g, b]) => Color::from_rgb(r, g, b),
                        None => theme.cosmic().accent_color().into(),
                    };
                    let mut bg = acc;
                    bg.a = 0.25;
                    widget::container::Style {
                        background: Some(Background::Color(bg)),
                        icon_color: Some(acc),
                        border: iced::Border {
                            radius: 20.0.into(),
                            width: 1.5,
                            color: acc,
                        },
                        ..Default::default()
                    }
                }))
                .into(),
        );
    }
    if g.show_username {
        login_parts.push(widget::text::body(user_label).into());
    }
    login_parts.push(
        widget::container(
            widget::row::with_children(vec![
                widget::text::caption("••••••••••").width(Length::Fill).into(),
                widget::icon::icon(icon("go-next-symbolic")).size(14).into(),
            ])
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .padding([6, 10])
        .width(Length::Fixed(165.0))
        .class(theme::Container::custom(move |theme| {
            let acc = match accent_rgb {
                Some([r, g, b]) => Color::from_rgb(r, g, b),
                None => theme.cosmic().accent_color().into(),
            };
            widget::container::Style {
                background: Some(Background::Color(if is_light {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.72)
                } else {
                    Color::from_rgba(0.05, 0.06, 0.09, 0.65)
                })),
                text_color: Some(sub_col),
                icon_color: Some(acc),
                border: iced::Border {
                    radius: (card_rad * 0.5).clamp(4.0, 14.0).into(),
                    width: 1.0,
                    color: acc,
                },
                ..Default::default()
            }
        }))
        .into(),
    );

    let login_card: Element<'a, Message> = widget::container(
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
    .into();

    let inner: Element<'a, Message> = match g.style.as_str() {
        "sidebar" => widget::row::with_children(vec![
            widget::container(login_card)
                .width(Length::Fixed(210.0))
                .height(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .into(),
            widget::container(clock_block)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .into(),
        ])
        .spacing(12)
        .height(Length::Fill)
        .into(),
        "editorial" => widget::row::with_children(vec![
            widget::container(clock_block)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(iced::alignment::Horizontal::Left)
                .align_y(iced::alignment::Vertical::Bottom)
                .into(),
            widget::container(login_card)
                .height(Length::Fill)
                .align_x(iced::alignment::Horizontal::Right)
                .align_y(iced::alignment::Vertical::Center)
                .into(),
        ])
        .spacing(12)
        .height(Length::Fill)
        .into(),
        "macos" => widget::column::with_children(vec![
            widget::container(clock_block)
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center)
                .into(),
            widget::space::vertical().height(Length::Fill).into(),
            widget::container(login_card)
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center)
                .into(),
        ])
        .height(Length::Fill)
        .into(),
        _ => widget::column::with_children(vec![
            widget::container(clock_block)
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center)
                .into(),
            widget::container(login_card)
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center)
                .into(),
        ])
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
                    g.style, g.font_family, wp_badge
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

fn login_view(app: &ControlPanel) -> Element<'_, Message> {
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
    let style_options: &[(&str, &str, &str)] = &[
        ("win11", "Windows 11", "Relógio central no topo + cartão de vidro centralizado"),
        ("android", "Android / Pixel", "Relógio gigante em 2 linhas (HH / MM) + login embaixo"),
        ("macos", "macOS Sonoma", "Relógio leve no topo + barra compacta de senha na base"),
        ("sidebar", "Painel Lateral", "Coluna de vidro à esquerda + relógio amplo à direita"),
        ("editorial", "Editorial", "Relógio no canto inferior esquerdo + cartão à direita"),
    ];
    let style_chips = widget::row::with_children(
        style_options
            .iter()
            .map(|(id, label, _)| {
                small_chip(
                    *label,
                    g.style == *id,
                    Message::GreeterStyle((*id).to_owned()),
                )
            })
            .collect::<Vec<_>>(),
    )
    .spacing(6);
    let active_style_desc = style_options
        .iter()
        .find(|(id, _, _)| *id == g.style)
        .map(|(_, _, desc)| *desc)
        .unwrap_or("Layout personalizado da tela de bloqueio e login.");

    items.push(card_box(
        widget::column::with_children(vec![
            widget::text::heading("Estilo da Tela (5 layouts)").into(),
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

fn optional_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let opt = &app.snapshot.optional;
    let repo_dir = app
        .snapshot
        .repo
        .path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "$HOME/Projetos/cosmic-taskbar".to_owned());

    let mut items: Vec<Element<'_, Message>> = vec![
        widget::text::title3("Pacotes & Opcionais").into(),
        widget::text::caption(
            "Componentes opcionais do ambiente: verifique o status de instalação e abra, habilite ou instale no terminal.",
        )
        .class(theme::Text::Custom(dimmed))
        .into(),
    ];

    // 1. Alt+Tab / Win+Tab isolado (aenes-switcher)
    let switcher_btn = if opt.switcher_active {
        action_btn("Reiniciar serviço", Message::RestartUserUnit("cosmic-switcher.service"), false)
    } else if opt.switcher_installed {
        action_btn(
            "Habilitar e iniciar",
            Message::EnableUserUnit("cosmic-switcher.service"),
            true,
        )
    } else {
        action_btn(
            "Instalar via repositório",
            Message::OpenInTerminal(
                "Instalar aenes-switcher",
                format!("cd \"{repo_dir}\" && ./install.sh && ./setup.sh"),
            ),
            true,
        )
    };
    items.push(card_box(
        widget::column::with_children(vec![
            widget::row::with_children(vec![
                widget::text::heading("Alt+Tab / Win+Tab isolado (aenes-switcher)")
                    .width(Length::Fill)
                    .into(),
                status_badge(
                    opt.switcher_active,
                    "Ativo",
                    if opt.switcher_installed { "Instalado (inativo)" } else { "Não instalado" },
                ),
            ])
            .align_y(Alignment::Center)
            .into(),
            widget::text::caption(
                "Alternador de janelas próprio via D-Bus (io.github.Aenes0.CosmicSwitcher) sem depender de patches no cosmic-workspaces oficial.",
            )
            .class(theme::Text::Custom(dimmed))
            .into(),
            widget::row::with_children(vec![switcher_btn]).into(),
        ])
        .spacing(spacing.space_xs),
    ));

    // 2. IriunCam
    let iriun_btn = if opt.iriun_installed {
        action_btn("Gerenciar em Câmeras", Message::SelectSection(Section::Camera), false)
    } else {
        action_btn(
            "Instalar no terminal",
            Message::OpenInTerminal(
                "Instalar IriunCam",
                format!("cd \"{repo_dir}\" && ./scripts/bootstrap-pc.sh --with-iriun"),
            ),
            true,
        )
    };
    items.push(card_box(
        widget::column::with_children(vec![
            widget::row::with_children(vec![
                widget::text::heading("IriunCam (celular como webcam)")
                    .width(Length::Fill)
                    .into(),
                status_badge(opt.iriun_installed, "Instalado", "Não instalado"),
            ])
            .align_y(Alignment::Center)
            .into(),
            widget::text::caption(
                "Permite usar o smartphone como webcam via USB/ADB com v4l2loopback e controle integrado na Central.",
            )
            .class(theme::Text::Custom(dimmed))
            .into(),
            widget::row::with_children(vec![iriun_btn]).into(),
        ])
        .spacing(spacing.space_xs),
    ));

    // 3. CoolerControl
    let cooler_btn = if opt.coolercontrol_installed {
        action_btn(
            "Abrir CoolerControl",
            Message::SpawnApp("coolercontrol", vec![], "CoolerControl"),
            false,
        )
    } else {
        action_btn(
            "Instalar no terminal",
            Message::OpenInTerminal(
                "Instalar CoolerControl",
                "if command -v paru >/dev/null 2>&1; then paru -S --needed coolercontrol; elif command -v yay >/dev/null 2>&1; then yay -S --needed coolercontrol; else echo 'Instale coolercontrol via AUR (paru/yay)'; fi".to_owned(),
            ),
            true,
        )
    };
    items.push(card_box(
        widget::column::with_children(vec![
            widget::row::with_children(vec![
                widget::text::heading("CoolerControl (ventoinhas & curvas térmicas)")
                    .width(Length::Fill)
                    .into(),
                status_badge(
                    opt.coolercontrol_active,
                    "Ativo",
                    if opt.coolercontrol_installed { "Instalado (serviço parado)" } else { "Não instalado" },
                ),
            ])
            .align_y(Alignment::Center)
            .into(),
            widget::text::caption(
                "Integrado automaticamente ao daemon economia-energia em desktops para alternar perfis silencioso/equilibrado.",
            )
            .class(theme::Text::Custom(dimmed))
            .into(),
            widget::row::with_children(vec![cooler_btn]).into(),
        ])
        .spacing(spacing.space_xs),
    ));

    // 4. Mission Center
    let mc_btn = if opt.mission_center_installed {
        if opt.mission_center_flatpak {
            action_btn(
                "Abrir Mission Center",
                Message::SpawnApp(
                    "flatpak",
                    vec!["run", "io.missioncenter.MissionCenter"],
                    "Mission Center",
                ),
                false,
            )
        } else {
            action_btn(
                "Abrir Mission Center",
                Message::SpawnApp("missioncenter", vec![], "Mission Center"),
                false,
            )
        }
    } else {
        action_btn(
            "Instalar no terminal",
            Message::OpenInTerminal(
                "Instalar Mission Center",
                "sudo pacman -S --needed mission-center || flatpak install -y flathub io.missioncenter.MissionCenter".to_owned(),
            ),
            true,
        )
    };
    items.push(card_box(
        widget::column::with_children(vec![
            widget::row::with_children(vec![
                widget::text::heading("Mission Center (gerenciador de tarefas)")
                    .width(Length::Fill)
                    .into(),
                status_badge(opt.mission_center_installed, "Instalado", "Não instalado"),
            ])
            .align_y(Alignment::Center)
            .into(),
            widget::text::caption(
                "Monitor de CPU, GPU, memória, discos e processos estilo Gerenciador de Tarefas do Windows.",
            )
            .class(theme::Text::Custom(dimmed))
            .into(),
            widget::row::with_children(vec![mc_btn]).into(),
        ])
        .spacing(spacing.space_xs),
    ));

    // 5. Brave
    let brave_btns: Vec<Element<'_, Message>> = if opt.brave_installed {
        vec![
            action_btn("Abrir Brave", Message::SpawnApp("brave", vec![], "Brave"), false),
            action_btn("Corrigir ícones PWA", Message::RunBravePwaFix, false),
        ]
    } else {
        vec![action_btn(
            "Instalar no terminal",
            Message::OpenInTerminal(
                "Instalar Brave",
                "if command -v paru >/dev/null 2>&1; then paru -S --needed brave-bin; elif command -v yay >/dev/null 2>&1; then yay -S --needed brave-bin; else sudo pacman -S --needed brave-bin; fi".to_owned(),
            ),
            true,
        )]
    };
    items.push(card_box(
        widget::column::with_children(vec![
            widget::row::with_children(vec![
                widget::text::heading("Brave Browser").width(Length::Fill).into(),
                status_badge(opt.brave_installed, "Instalado", "Não instalado"),
            ])
            .align_y(Alignment::Center)
            .into(),
            widget::text::caption(
                "Navegador com suporte ao watcher automático de correção de ícones para Web Apps (PWAs).",
            )
            .class(theme::Text::Custom(dimmed))
            .into(),
            widget::row::with_children(brave_btns).spacing(8).into(),
        ])
        .spacing(spacing.space_xs),
    ));

    // 6. MAC Rotativo
    let mac_btn = if opt.mac_rotativo_active {
        action_btn(
            "Desabilitar no terminal",
            Message::OpenInTerminal(
                "Desabilitar MAC Rotativo",
                "sudo systemctl disable --now mac-rotativo.timer".to_owned(),
            ),
            false,
        )
    } else if opt.mac_rotativo_installed {
        action_btn(
            "Habilitar no terminal",
            Message::OpenInTerminal(
                "Habilitar MAC Rotativo",
                "sudo systemctl enable --now mac-rotativo.timer".to_owned(),
            ),
            true,
        )
    } else {
        action_btn(
            "Ver status / configurar no terminal",
            Message::OpenInTerminal(
                "MAC Rotativo",
                "systemctl status mac-rotativo.timer || echo 'mac-rotativo.timer não instalado em /etc/systemd/system/'".to_owned(),
            ),
            false,
        )
    };
    items.push(card_box(
        widget::column::with_children(vec![
            widget::row::with_children(vec![
                widget::text::heading("MAC Rotativo (mac-rotativo.timer)")
                    .width(Length::Fill)
                    .into(),
                status_badge(
                    opt.mac_rotativo_active,
                    "Ativo",
                    if opt.mac_rotativo_installed { "Instalado (inativo)" } else { "Não instalado" },
                ),
            ])
            .align_y(Alignment::Center)
            .into(),
            widget::text::caption(
                "Timer systemd para rotação periódica de endereço MAC em interfaces de rede.",
            )
            .class(theme::Text::Custom(dimmed))
            .into(),
            widget::row::with_children(vec![mac_btn]).into(),
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
