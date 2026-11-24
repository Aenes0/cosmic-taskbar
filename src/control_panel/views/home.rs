// SPDX-License-Identifier: GPL-3.0-only

//! Visão geral (Início), spec §7: linha de hardware, alertas, métricas ao vivo, cartões de estado,
//! ações rápidas e "Alterados recentemente".

mod alerts;

use std::time::{Instant, SystemTime};

use cosmic::{
    Element,
    iced::{self, Alignment, Background, Length},
    theme, widget,
};

use super::row::{self, BtnStyle, Tone};
use crate::central::tiles::chip_class;
use crate::central::widgets::card;
use crate::control_panel::health;
use crate::control_panel::power_config::PowerProfileKind;
use alerts::{Alert, AlertAction, AlertInput, AlertTone, SECURITY_RECOMMENDED, ago, ahead, alerts, span};
use crate::control_panel::{ControlPanel, Message, Section};

/// A barra de uma métrica fica âmbar a partir daqui (spec §7).
const AMBER_FROM_PCT: f32 = 70.0;
const BAR_HEIGHT: f32 = 6.0;
const CARD_PADDING: u16 = 16;
const RECENT_SHOWN: usize = 3;

pub(super) fn backup_message() -> Message {
    Message::OpenInTerminal("Backup Restic", "~/.local/bin/backup-restic".to_owned())
}

fn dash_or(v: Option<String>) -> String {
    v.unwrap_or_else(|| "—".to_owned())
}

fn panel<'a>(content: Element<'a, Message>) -> Element<'a, Message> {
    widget::container(content)
        .padding(CARD_PADDING)
        .width(Length::Fill)
        .class(theme::Container::custom(|theme| {
            let c = card(theme.cosmic());
            let mut border: iced::Color = theme.cosmic().background(theme.transparent).divider.into();
            border.a *= 0.55;
            widget::container::Style {
                background: Some(Background::Color(c.base.into())),
                text_color: Some(c.on.into()),
                icon_color: Some(c.on.into()),
                border: iced::Border {
                    radius: crate::shared::tokens::POPUP_RADIUS.into(),
                    width: crate::shared::tokens::HAIRLINE_WIDTH,
                    color: border,
                },
                ..Default::default()
            }
        }))
        .into()
}

fn heading<'a>(text: &'a str) -> Element<'a, Message> {
    widget::text(text).size(14.0).font(cosmic::font::bold()).into()
}

fn caption<'a>(text: impl Into<std::borrow::Cow<'a, str>> + 'a) -> Element<'a, Message> {
    widget::text::caption(text)
        .class(theme::Text::Custom(|theme| {
            let mut color = theme.cosmic().background(theme.transparent).on;
            color.alpha *= 0.65;
            iced::widget::text::Style { color: Some(color.into()), ..Default::default() }
        }))
        .into()
}

fn bar<'a>(pct: Option<f32>) -> Element<'a, Message> {
    let value = pct.unwrap_or(0.0).clamp(0.0, 100.0);
    let amber = value >= AMBER_FROM_PCT;
    let fill = widget::container(widget::Space::new().width(Length::Fill).height(Length::Fixed(BAR_HEIGHT)))
        .class(theme::Container::custom(move |theme| {
            let c = theme.cosmic();
            let color: iced::Color = if amber { c.warning_color().into() } else { c.accent_color().into() };
            widget::container::Style {
                background: Some(Background::Color(color)),
                border: iced::Border { radius: (BAR_HEIGHT / 2.0).into(), ..Default::default() },
                ..Default::default()
            }
        }));
    let portion = (value * 10.0).round() as u16;
    let mut inner = widget::row::with_capacity(2);
    if portion > 0 {
        inner = inner.push(fill.width(Length::FillPortion(portion)));
    }
    if portion < 1000 {
        inner = inner.push(widget::Space::new().width(Length::FillPortion(1000 - portion)).height(Length::Fixed(BAR_HEIGHT)));
    }
    widget::container(inner)
        .width(Length::Fill)
        .class(theme::Container::custom(|theme| {
            let mut c: iced::Color = theme.cosmic().background(theme.transparent).on.into();
            c.a = 0.12;
            widget::container::Style {
                background: Some(Background::Color(c)),
                border: iced::Border { radius: (BAR_HEIGHT / 2.0).into(), ..Default::default() },
                ..Default::default()
            }
        }))
        .into()
}

fn metric_card<'a>(
    title: &'a str,
    value: String,
    detail: String,
    pct: Option<f32>,
) -> Element<'a, Message> {
    let spacing = cosmic::theme::spacing();
    panel(
        widget::column::with_children(vec![
            caption(title),
            widget::text::title3(value).into(),
            bar(pct),
            caption(detail),
        ])
        .spacing(spacing.space_xxs)
        .into(),
    )
}

/// Cartão de estado: título + selo, linhas de texto, controle extra e botões.
fn state_card<'a>(
    title: &'a str,
    pill: Option<(&'static str, Tone)>,
    lines: Vec<String>,
    extra: Option<Element<'a, Message>>,
    buttons: Vec<Element<'a, Message>>,
) -> Element<'a, Message> {
    let spacing = cosmic::theme::spacing();
    let mut head = widget::row::with_capacity(3)
        .spacing(spacing.space_xs)
        .align_y(Alignment::Center)
        .push(widget::container(heading(title)).width(Length::Fill));
    if let Some((text, tone)) = pill {
        head = head.push(row::pill(text, tone));
    }
    let mut col = widget::column::with_capacity(lines.len() + 3)
        .spacing(spacing.space_xxs)
        .push(head);
    for l in lines {
        col = col.push(caption(l));
    }
    if let Some(e) = extra {
        col = col.push(e);
    }
    let mut foot = widget::row::with_capacity(buttons.len()).spacing(spacing.space_xxs);
    for b in buttons {
        foot = foot.push(b);
    }
    col = col.push(foot);
    panel(col.into())
}

fn open_button<'a>(section: Section) -> Element<'a, Message> {
    row::button("Abrir", Message::SelectSection(section), BtnStyle::Normal)
}

fn plan_chips<'a>(active: Option<PowerProfileKind>, enabled: bool) -> Element<'a, Message> {
    let spacing = cosmic::theme::spacing();
    let mut r = widget::row::with_capacity(3).spacing(spacing.space_xxxs);
    for kind in [PowerProfileKind::PowerSaver, PowerProfileKind::Balanced, PowerProfileKind::Performance] {
        let msg = if enabled { Message::SetPowerProfile(kind.id().to_owned()) } else { Message::None };
        r = r.push(
            widget::button::custom(widget::text::caption(kind.label()))
                .padding([5, 12])
                .class(chip_class(active == Some(kind)))
                .on_press(msg),
        );
    }
    r.into()
}

/// `n` elementos por linha, preenchendo a última com espaço.
fn grid<'a>(cells: Vec<Element<'a, Message>>, per_row: usize) -> Element<'a, Message> {
    let spacing = cosmic::theme::spacing();
    let mut col = widget::column::with_capacity(cells.len() / per_row + 1).spacing(spacing.space_s);
    let mut it = cells.into_iter().peekable();
    while it.peek().is_some() {
        let mut r = widget::row::with_capacity(per_row).spacing(spacing.space_s);
        for _ in 0..per_row {
            r = match it.next() {
                Some(c) => r.push(widget::container(c).width(Length::FillPortion(1))),
                None => r.push(widget::Space::new().width(Length::FillPortion(1))),
            };
        }
        col = col.push(r);
    }
    col.into()
}

fn hardware_line(app: &ControlPanel) -> String {
    let hw = &app.snapshot.hardware;
    let mut parts: Vec<String> = Vec::new();
    if !hw.cpu_model.is_empty() {
        parts.push(hw.cpu_model.clone());
    }
    if hw.has_nvidia {
        parts.push("GPU NVIDIA".to_owned());
    }
    if let Some(d) = &app.displays {
        let n = d.iter().filter(|d| d.enabled).count();
        parts.push(format!("{n} monitor{}", if n == 1 { "" } else { "es" }));
    }
    parts.push(if app.metrics.is_some() { "atualizado agora" } else { "lendo…" }.to_owned());
    parts.join(" · ")
}

fn alert_input(app: &ControlPanel) -> AlertInput<'_> {
    AlertInput {
        units: &app.snapshot.units,
        health: &app.unit_health,
        security_score: app.snapshot.security.score,
        backup_times: &app.backup_times,
        backup_installed: app.snapshot.backup_restic_installed,
        backup_drive_mounted: app.snapshot.backup_drive_mounted,
        power_pending: app.power_dirty(),
        osd_pending: app.osd_apply(),
        now: SystemTime::now(),
    }
}

fn alerts_group(list: Vec<Alert>, busy: bool) -> Element<'static, Message> {
    let spacing = cosmic::theme::spacing();
    let rows = list
        .into_iter()
        .map(|a| {
            let tone = match a.tone {
                AlertTone::Warn => Tone::Warn,
                AlertTone::Bad => Tone::Bad,
            };
            let mut buttons = widget::row::with_capacity(2).spacing(spacing.space_xxs);
            if a.action == AlertAction::Open {
                buttons = buttons.push(row::button("Abrir", Message::SelectSection(a.section), BtnStyle::Normal));
            } else {
                buttons = buttons
                    .push(row::button("Detalhes", Message::SelectSection(a.section), BtnStyle::Normal))
                    .push(
                        widget::button::suggested(a.action.label())
                            .on_press_maybe((!busy).then(|| a.action.message())),
                    );
            }
            widget::row::with_children(vec![
                row::pill(if tone == Tone::Bad { "Atenção" } else { "Aviso" }, tone),
                widget::column::with_children(vec![
                    widget::text(a.title).size(14.0).font(cosmic::font::semibold()).into(),
                    caption(a.detail),
                ])
                .spacing(spacing.space_xxxs)
                .width(Length::Fill)
                .into(),
                buttons.into(),
            ])
            .spacing(spacing.space_s)
            .align_y(Alignment::Center)
            .padding([14, 18])
            .into()
        })
        .collect();
    row::group("Alertas", None, false, rows)
}

fn metrics_row(app: &ControlPanel) -> Element<'_, Message> {
    let m = app.metrics.as_ref();
    let pct = |v: Option<f32>| dash_or(v.map(|v| format!("{v:.0}%")));
    let cpu_detail = format!(
        "{} · {}",
        dash_or(m.and_then(|m| m.cpu_temp).map(|t| format!("{t:.0} °C"))),
        dash_or(m.and_then(|m| m.cpu_ghz).map(|g| format!("{g:.1} GHz")))
    );
    let gpu_detail = format!(
        "{} · {}",
        dash_or(m.and_then(|m| m.gpu_temp).map(|t| format!("{t:.0} °C"))),
        dash_or(m.and_then(|m| m.gpu_watts).map(|w| format!("{w:.0} W")))
    );
    let power_detail = dash_or(m.and_then(|m| m.cost_per_hour).map(|c| format!("R$ {c:.2}/h")));
    grid(
        vec![
            metric_card("CPU", pct(m.map(|m| m.cpu_pct)), cpu_detail, m.map(|m| m.cpu_pct)),
            metric_card("GPU", pct(m.and_then(|m| m.gpu_pct)), gpu_detail, m.and_then(|m| m.gpu_pct)),
            metric_card("Memória", pct(m.map(|m| m.mem_pct)), "em uso".to_owned(), m.map(|m| m.mem_pct)),
            metric_card(
                "Consumo",
                dash_or(m.and_then(|m| m.total_watts).map(|w| format!("{w:.0} W"))),
                power_detail,
                None,
            ),
        ],
        4,
    )
}

fn power_card(app: &ControlPanel) -> Element<'_, Message> {
    let active = app.active_profile_kind();
    let (pill, line) = match active {
        Some(k) => (Some(("Ativo", Tone::Ok)), format!("Plano {}", k.label())),
        None => (None, "Serviço de energia não respondeu".to_owned()),
    };
    state_card(
        "Energia",
        pill,
        vec![line],
        Some(plan_chips(active, !app.busy && app.power.is_some())),
        vec![open_button(Section::Power)],
    )
}

fn security_card(app: &ControlPanel) -> Element<'_, Message> {
    let score = app.snapshot.security.score;
    let (pill, line) = if score == 0 {
        (None, "Lendo a nota…".to_owned())
    } else if score >= SECURITY_RECOMMENDED {
        (Some(("Boa", Tone::Ok)), format!("Nota {score}/100"))
    } else {
        (Some(("Pode melhorar", Tone::Warn)), format!("Nota {score}/100"))
    };
    state_card(
        "Proteção",
        pill,
        vec![line],
        None,
        vec![
            row::button("Verificar agora", Message::RefreshAll, BtnStyle::Normal),
            open_button(Section::Security),
        ],
    )
}

fn backup_card(app: &ControlPanel) -> Element<'_, Message> {
    let now = SystemTime::now();
    let t = &app.backup_times;
    let late = health::backup_late(t, now);
    let pill = if late { Some(("Atrasado", Tone::Bad)) } else { None };
    state_card(
        "Backup",
        pill,
        vec![
            format!("Último: {}", t.last.map_or_else(|| "nunca rodou".to_owned(), |l| ago(now, l))),
            format!("Próximo: {}", t.next.map_or_else(|| "—".to_owned(), |n| ahead(now, n))),
        ],
        None,
        vec![
            row::button("Executar agora", backup_message(), BtnStyle::Normal),
            open_button(Section::Backup),
        ],
    )
}

fn network_card(app: &ControlPanel) -> Element<'_, Message> {
    let net = &app.network;
    let conn = net
        .connection
        .as_ref()
        .map_or_else(|| "Sem conexão".to_owned(), |(name, _)| name.clone());
    let vpn = match net.vpns.iter().find(|v| v.active.is_some()) {
        Some(v) => format!("VPN: {}", v.name),
        None if net.vpns.is_empty() => "Nenhuma VPN configurada".to_owned(),
        None => "VPN desligada".to_owned(),
    };
    let pill = net.connection.is_some().then_some(("Conectada", Tone::Ok));
    state_card("Rede", pill, vec![conn, vpn], None, vec![open_button(Section::Network)])
}

fn displays_card(app: &ControlPanel) -> Element<'_, Message> {
    let count = app
        .displays
        .as_ref()
        .map_or_else(|| "—".to_owned(), |d| d.iter().filter(|d| d.enabled).count().to_string());
    let primary = app.primary_display.clone().unwrap_or_else(|| "—".to_owned());
    state_card(
        "Monitores",
        None,
        vec![format!("{count} ativo(s)"), format!("Principal: {primary}")],
        None,
        vec![open_button(Section::Displays)],
    )
}

/// Unidades de usuário carregadas e paradas (as de sistema pedem senha: levam à seção).
fn stopped_user_units(app: &ControlPanel) -> Vec<&'static str> {
    app.unit_health
        .iter()
        .filter(|(_, h)| health::is_stopped(h))
        .filter(|(unit, _)| app.snapshot.units.iter().any(|u| u.unit == *unit && !u.is_system))
        .map(|(unit, _)| *unit)
        .collect()
}

fn services_card(app: &ControlPanel) -> Element<'_, Message> {
    let units = &app.snapshot.units;
    let active = units.iter().filter(|u| u.active).count();
    let stopped_user = stopped_user_units(app);
    let stopped_system = app
        .unit_health
        .iter()
        .filter(|(unit, h)| health::is_stopped(h) && units.iter().any(|u| u.unit == *unit && u.is_system))
        .count();
    let mut lines = vec![format!("{active} de {} ativos", units.len())];
    if stopped_system > 0 {
        lines.push(format!("{stopped_system} de sistema parado(s): abra a seção"));
    }
    let restart = (!app.busy && !stopped_user.is_empty()).then(|| {
        Message::RunAction(
            "Reiniciar serviços parados",
            format!("systemctl --user restart {}", stopped_user.join(" ")),
            false,
        )
    });
    let pill = (!stopped_user.is_empty() || stopped_system > 0).then_some(("Atenção", Tone::Warn));
    state_card(
        "Serviços",
        pill,
        lines,
        None,
        vec![
            widget::button::standard("Reiniciar parados").on_press_maybe(restart).into(),
            open_button(Section::System),
        ],
    )
}

fn quick_actions(app: &ControlPanel) -> Element<'_, Message> {
    let enabled = !app.busy;
    let item = |label: &'static str, msg: Message| -> Element<'_, Message> {
        widget::button::standard(label)
            .on_press_maybe(enabled.then_some(msg))
            .width(Length::Fill)
            .into()
    };
    let cells = vec![
        item("Notificação de teste", Message::NotifSendTest),
        item("Testar recorte", Message::RunScreenshot),
        item("Executar backup", backup_message()),
        item("Verificar segurança", Message::RefreshAll),
        item("Reiniciar painel", Message::RestartPanel),
        item("Bloquear tela", Message::GreeterTestLock),
    ];
    let spacing = cosmic::theme::spacing();
    widget::column::with_children(vec![
        container_title("Ações rápidas"),
        grid(cells, 2),
    ])
    .spacing(spacing.space_xxs)
    .into()
}

fn container_title<'a>(text: &'a str) -> Element<'a, Message> {
    widget::container(heading(text)).padding([0, 18]).into()
}

fn recent_group(app: &ControlPanel) -> Element<'_, Message> {
    let now = Instant::now();
    let mut rows: Vec<Element<'_, Message>> = Vec::new();
    for (i, c) in app.recent.items().iter().enumerate().take(RECENT_SHOWN) {
        let undo = c
            .undo
            .is_some()
            .then(|| row::button("Desfazer", Message::Undo(i), BtnStyle::Normal));
        let when = now.saturating_duration_since(c.at);
        let when = if when.as_secs() < 60 { "agora".to_owned() } else { format!("há {}", span(when)) };
        rows.push(row::list_item(c.label.as_str(), format!("{} · {when}", c.section.label()), None, undo));
    }
    if rows.is_empty() {
        rows.push(row::note("Nada alterado nesta sessão."));
    }
    row::group("Alterados recentemente", None, false, rows)
}

pub(super) fn home_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = cosmic::theme::spacing();
    let mut page: Vec<Element<'_, Message>> = vec![caption(hardware_line(app))];
    let list = alerts(&alert_input(app));
    if !list.is_empty() {
        page.push(alerts_group(list, app.busy));
    }
    page.push(metrics_row(app));
    page.push(grid(
        vec![
            power_card(app),
            security_card(app),
            backup_card(app),
            network_card(app),
            displays_card(app),
            services_card(app),
        ],
        3,
    ));
    page.push(quick_actions(app));
    page.push(recent_group(app));
    widget::column::with_children(page).spacing(spacing.space_m).into()
}

