// SPDX-License-Identifier: GPL-3.0-only

//! Visão geral (Início), spec §7: linha de hardware, alertas, métricas ao vivo, cartões de estado,
//! ações rápidas e "Alterados recentemente".

mod alerts;

use std::time::{Instant, SystemTime};

use cosmic::{
    Element,
    iced::{self, Alignment, Background, Color, Length},
    theme, widget,
};

use super::row::{self, Btn, BtnStyle, Tone};
use crate::central::tiles::icon;
use crate::control_panel::health;
use crate::control_panel::power_config::PowerProfileKind;
use alerts::{Alert, AlertAction, AlertInput, AlertTone, SECURITY_RECOMMENDED, ago, ahead, alerts, span};
use crate::control_panel::{ControlPanel, Message, Section};

/// A barra de uma métrica fica âmbar a partir daqui (spec §7).
const AMBER_FROM_PCT: f32 = 70.0;
const BAR_HEIGHT: f32 = 6.0;
const CARD_PADDING: u16 = 16;
const CARD_RADIUS: f32 = 14.0;
const GRID_GAP: f32 = 12.0;
const RECENT_SHOWN: usize = 3;
/// Altura dos cartões de estado: a grade do mockup estica os vizinhos até o mais alto, e o iced
/// não faz isso; a 1ª fileira (com o segmentado do plano) é mais alta que a 2ª.
const STATE_CARD_HEIGHT: f32 = 176.0;
const STATE_CARD_HEIGHT_WITH_PLAN: f32 = 224.0;

pub(super) fn backup_message() -> Message {
    Message::OpenInTerminal("Backup Restic", "~/.local/bin/backup-restic".to_owned())
}

fn dash_or(v: Option<String>) -> String {
    v.unwrap_or_else(|| "—".to_owned())
}

fn panel<'a>(content: Element<'a, Message>, padding: [u16; 2]) -> widget::Container<'a, Message, cosmic::Theme> {
    widget::container(content)
        .padding(padding)
        .width(Length::Fill)
        .class(theme::Container::custom(|theme| row::card_style(theme, CARD_RADIUS)))
}

fn heading<'a>(text: &'a str) -> Element<'a, Message> {
    widget::text(text).size(13.5).font(cosmic::font::semibold()).into()
}

fn caption<'a>(text: impl Into<std::borrow::Cow<'a, str>> + 'a, size: f32) -> Element<'a, Message> {
    widget::text(text).size(size).class(theme::Text::Custom(row::dimmed)).into()
}

fn faint<'a>(text: impl Into<std::borrow::Cow<'a, str>> + 'a) -> Element<'a, Message> {
    widget::text(text)
        .size(12.0)
        .class(theme::Text::Custom(|theme| iced::widget::text::Style {
            color: Some(row::on_alpha(theme, 0.58)),
            ..Default::default()
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
    // Mínimo de 3% para a barra nunca sumir (mockup).
    let portion = (value.max(3.0) * 10.0).round() as u16;
    let mut inner = widget::row::with_capacity(2);
    inner = inner.push(fill.width(Length::FillPortion(portion)));
    if portion < 1000 {
        inner = inner.push(widget::Space::new().width(Length::FillPortion(1000 - portion)).height(Length::Fixed(BAR_HEIGHT)));
    }
    widget::container(inner)
        .width(Length::Fill)
        .class(theme::Container::custom(|theme| widget::container::Style {
            background: Some(Background::Color(row::on_alpha(theme, 0.08))),
            border: iced::Border { radius: (BAR_HEIGHT / 2.0).into(), ..Default::default() },
            ..Default::default()
        }))
        .into()
}

/// Métrica ao vivo: rótulo à esquerda e valor à direita na mesma linha, barra e detalhe.
fn metric_card<'a>(
    title: &'a str,
    value: String,
    detail: String,
    pct: Option<f32>,
) -> Element<'a, Message> {
    let top = widget::row::with_children(vec![
        widget::container(caption(title, 12.5)).width(Length::Fill).into(),
        widget::text(value).size(20.0).font(cosmic::font::bold()).into(),
    ])
    .align_y(Alignment::Center);
    panel(
        widget::column::with_children(vec![top.into(), bar(pct), faint(detail)])
            .spacing(10)
            .into(),
        [14, 16],
    )
    .into()
}

/// Cartão de estado: ícone + título + selo, valor em destaque, detalhe, controle extra e rodapé
/// (ação à esquerda, "Abrir ›" à direita).
fn state_card<'a>(
    section: Section,
    title: &'a str,
    pill: Option<(&'static str, Tone)>,
    value: String,
    meta: String,
    extra: Option<Element<'a, Message>>,
    action: Option<Element<'a, Message>>,
    height: f32,
) -> Element<'a, Message> {
    let mut head = widget::row::with_capacity(3)
        .spacing(10)
        .align_y(Alignment::Center)
        .push(row::icon_box(section.icon(), 34.0, 17, Tone::Info))
        .push(widget::container(heading(title)).width(Length::Fill));
    if let Some((text, tone)) = pill {
        head = head.push(row::pill(text, tone));
    }
    let mut col = widget::column::with_capacity(5).spacing(14).push(head).push(
        widget::column::with_children(vec![
            widget::text(value).size(19.0).font(cosmic::font::bold()).into(),
            caption(meta, 12.5),
        ])
        .spacing(3),
    );
    if let Some(e) = extra {
        col = col.push(e);
    }
    let open = widget::button::custom(
        widget::row::with_children(vec![
            widget::text("Abrir").size(12.5).font(row::medium()).into(),
            widget::icon::icon(icon("go-next-symbolic")).size(15).into(),
        ])
        .spacing(2)
        .align_y(Alignment::Center),
    )
    .padding([7, 4, 7, 10])
    .class(row::btn_class(Btn::Link, 8.0))
    .on_press(Message::SelectSection(section));
    let mut foot = widget::row::with_capacity(3).align_y(Alignment::Center);
    if let Some(a) = action {
        foot = foot.push(a);
    }
    foot = foot.push(widget::Space::new().width(Length::Fill)).push(open);
    col = col.push(widget::Space::new().height(Length::Fill)).push(foot);
    panel(col.height(Length::Fill).into(), [CARD_PADDING, CARD_PADDING])
        .height(Length::Fixed(height))
        .into()
}

/// Ação do rodapé de um cartão (30 px, neutra).
fn card_action<'a>(label: &'a str, msg: Option<Message>) -> Element<'a, Message> {
    row::action_btn(label, msg, BtnStyle::Normal, 30.0)
}

fn plan_segmented<'a>(active: Option<PowerProfileKind>, enabled: bool) -> Element<'a, Message> {
    let kinds = [PowerProfileKind::PowerSaver, PowerProfileKind::Balanced, PowerProfileKind::Performance];
    let labels: Vec<&'static str> = kinds.iter().map(|k| k.label()).collect();
    let selected = active.and_then(|a| kinds.iter().position(|k| *k == a));
    row::seg_control(&labels, selected, true, move |i| {
        Some(if enabled { Message::SetPowerProfile(kinds[i].id().to_owned()) } else { Message::None })
    })
}

/// `n` elementos por linha, preenchendo a última com espaço.
fn grid<'a>(cells: Vec<Element<'a, Message>>, per_row: usize) -> Element<'a, Message> {
    let mut col = widget::column::with_capacity(cells.len() / per_row + 1).spacing(GRID_GAP);
    let mut it = cells.into_iter().peekable();
    while it.peek().is_some() {
        let mut r = widget::row::with_capacity(per_row).spacing(GRID_GAP);
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

/// Um alerta: cartão tingido do tom, ícone, texto, "Detalhes" e a ação cheia.
fn alert_card(a: Alert, busy: bool) -> Element<'static, Message> {
    let tone = match a.tone {
        AlertTone::Warn => Tone::Warn,
        AlertTone::Bad => Tone::Bad,
    };
    let tone_fg = move |theme: &cosmic::Theme| row::tone_colors(theme, tone).1;
    let icon_box = widget::container(widget::icon::icon(icon("dialog-warning-symbolic")).size(18))
        .width(Length::Fixed(34.0))
        .height(Length::Fixed(34.0))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .class(theme::Container::custom(move |theme| {
            let fg = tone_fg(theme);
            widget::container::Style {
                background: Some(Background::Color(Color { a: 0.16, ..fg })),
                icon_color: Some(fg),
                border: iced::Border { radius: 10.0.into(), ..Default::default() },
                ..Default::default()
            }
        }));
    let link = move |hovered: bool, theme: &cosmic::Theme| {
        let fg = tone_fg(theme);
        widget::button::Style {
            background: hovered.then(|| Background::Color(Color { a: 0.10, ..fg })),
            border_radius: 8.0.into(),
            text_color: Some(fg),
            ..widget::button::Style::new()
        }
    };
    let filled = move |hovered: bool, disabled: bool, theme: &cosmic::Theme| {
        let fg = tone_fg(theme);
        let a = if disabled { 0.45 } else if hovered { 0.9 } else { 1.0 };
        widget::button::Style {
            background: Some(Background::Color(Color { a, ..fg })),
            border_radius: 8.0.into(),
            text_color: Some(theme.cosmic().background(theme.transparent).base.into()),
            ..widget::button::Style::new()
        }
    };
    let btn_text = |t: &'static str, bold: bool| {
        widget::container(
            widget::text(t).size(13.0).font(if bold { cosmic::font::semibold() } else { row::medium() }),
        )
        .height(Length::Fill)
        .align_y(Alignment::Center)
    };
    let details = widget::button::custom(btn_text(if a.action == AlertAction::Open { "Abrir" } else { "Detalhes" }, false))
        .height(Length::Fixed(32.0))
        .padding([0, 12])
        .class(theme::Button::Custom {
            active: Box::new(move |_, t| link(false, t)),
            disabled: Box::new(move |t| link(false, t)),
            hovered: Box::new(move |_, t| link(true, t)),
            pressed: Box::new(move |_, t| link(true, t)),
        })
        .on_press(Message::SelectSection(a.section));
    let mut buttons = widget::row::with_capacity(2).spacing(4).align_y(Alignment::Center).push(details);
    if a.action != AlertAction::Open {
        buttons = buttons.push(
            widget::button::custom(btn_text(a.action.label(), true))
                .height(Length::Fixed(32.0))
                .padding([0, 14])
                .class(theme::Button::Custom {
                    active: Box::new(move |_, t| filled(false, false, t)),
                    disabled: Box::new(move |t| filled(false, true, t)),
                    hovered: Box::new(move |_, t| filled(true, false, t)),
                    pressed: Box::new(move |_, t| filled(true, false, t)),
                })
                .on_press_maybe((!busy).then(|| a.action.message())),
        );
    }
    widget::container(
        widget::row::with_children(vec![
            icon_box.into(),
            widget::column::with_children(vec![
                widget::text(a.title).size(13.5).font(cosmic::font::semibold()).into(),
                widget::text(a.detail).size(12.5).class(theme::Text::Custom(row::soft)).into(),
            ])
            .spacing(2)
            .width(Length::Fill)
            .into(),
            buttons.into(),
        ])
        .spacing(14)
        .align_y(Alignment::Center),
    )
    .padding([12, 14, 12, 16])
    .width(Length::Fill)
    .class(theme::Container::custom(move |theme| {
        let fg = tone_fg(theme);
        widget::container::Style {
            background: Some(Background::Color(Color { a: 0.08, ..fg })),
            border: iced::Border { radius: CARD_RADIUS.into(), width: 1.0, color: Color { a: 0.22, ..fg } },
            ..Default::default()
        }
    }))
    .into()
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
    let power_detail = dash_or(m.and_then(|m| m.cost_per_hour).map(|c| format!("≈ R$ {c:.2} por hora")));
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

/// "5 min" → "5 min"; "Nunca" → "nunca" (para o meio da frase).
fn mins_phrase(mins: u32) -> String {
    let s = crate::control_panel::power_config::ProfileTuning::format_mins(mins);
    if mins == 0 { s.to_lowercase() } else { s }
}

fn power_card(app: &ControlPanel) -> Element<'_, Message> {
    let active = app.active_profile_kind();
    let (pill, value, meta) = match active {
        Some(k) => {
            let t = app.power_config.get(k);
            (
                Some(("Ativo", Tone::Ok)),
                format!("Plano {}", k.label()),
                format!("Tela apaga em {} · senha em {}", mins_phrase(t.screen_off_mins), mins_phrase(t.lock_mins)),
            )
        }
        None => (None, "Sem plano".to_owned(), "Serviço de energia não respondeu".to_owned()),
    };
    state_card(
        Section::Power,
        "Energia",
        pill,
        value,
        meta,
        Some(plan_segmented(active, !app.busy && app.power.is_some())),
        None,
        STATE_CARD_HEIGHT_WITH_PLAN,
    )
}

fn security_card(app: &ControlPanel) -> Element<'_, Message> {
    let score = app.snapshot.security.score;
    let (pill, value) = if score == 0 {
        (None, "Lendo a nota…".to_owned())
    } else if score >= SECURITY_RECOMMENDED {
        (Some(("Boa", Tone::Ok)), format!("{score} de 100"))
    } else {
        (Some(("Pode melhorar", Tone::Warn)), format!("{score} de 100"))
    };
    state_card(
        Section::Security,
        "Proteção",
        pill,
        value,
        "Nota de proteção do sistema".to_owned(),
        None,
        Some(card_action("Verificar agora", Some(Message::RefreshAll))),
        STATE_CARD_HEIGHT_WITH_PLAN,
    )
}

fn backup_card(app: &ControlPanel) -> Element<'_, Message> {
    let now = SystemTime::now();
    let t = &app.backup_times;
    let late = health::backup_late(t, now);
    let pill = if late { Some(("Atrasado", Tone::Bad)) } else { None };
    let value = t.last.map_or_else(|| "Nunca rodou".to_owned(), |l| ago(now, l));
    let meta = format!("Próximo: {}", t.next.map_or_else(|| "—".to_owned(), |n| ahead(now, n)));
    let run = row::when_idle(app.busy, backup_message());
    state_card(
        Section::Backup,
        "Backup",
        pill,
        value,
        meta,
        None,
        Some(card_action("Executar agora", run)),
        STATE_CARD_HEIGHT_WITH_PLAN,
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
    state_card(Section::Network, "Rede", pill, conn, vpn, None, None, STATE_CARD_HEIGHT)
}

fn displays_card(app: &ControlPanel) -> Element<'_, Message> {
    let count = app.displays.as_ref().map(|d| d.iter().filter(|d| d.enabled).count());
    let value = match count {
        Some(1) => "1 tela".to_owned(),
        Some(n) => format!("{n} telas"),
        None => "—".to_owned(),
    };
    let primary = app.primary_display.clone().unwrap_or_else(|| "—".to_owned());
    let pill = count.map(|_| ("OK", Tone::Info));
    state_card(
        Section::Displays,
        "Monitores",
        pill,
        value,
        format!("Principal: {primary}"),
        None,
        None,
        STATE_CARD_HEIGHT,
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
    let meta = if stopped_system > 0 {
        format!("{stopped_system} de sistema parado(s): abra a seção")
    } else if stopped_user.is_empty() {
        "Nenhum serviço parado".to_owned()
    } else {
        format!("Parados: {}", stopped_user.join(", "))
    };
    let restart = (!app.busy && !stopped_user.is_empty()).then(|| {
        Message::RunAction(
            "Reiniciar serviços parados",
            format!("systemctl --user restart {}", stopped_user.join(" ")),
            false,
        )
    });
    let pill = if !stopped_user.is_empty() || stopped_system > 0 {
        Some(("Atenção", Tone::Warn))
    } else {
        Some(("OK", Tone::Ok))
    };
    state_card(
        Section::System,
        "Serviços",
        pill,
        format!("{active} de {} ativos", units.len()),
        meta,
        None,
        Some(card_action("Reiniciar parados", restart)),
        STATE_CARD_HEIGHT,
    )
}

fn quick_actions(app: &ControlPanel) -> Element<'_, Message> {
    let enabled = !app.busy;
    let item = |ico: &'static str, label: &'static str, msg: Message| -> Element<'_, Message> {
        widget::button::custom(
            widget::row::with_children(vec![
                widget::icon::icon(icon(ico))
                    .size(16)
                    .class(theme::Svg::Custom(std::rc::Rc::new(|theme| iced::widget::svg::Style {
                        color: Some(row::accent(theme)),
                    })))
                    .into(),
                widget::text(label).size(13.0).into(),
            ])
            .spacing(10)
            .align_y(Alignment::Center)
            .height(Length::Fill),
        )
        .height(Length::Fixed(40.0))
        .padding([0, 12])
        .width(Length::Fill)
        .class(row::btn_class(Btn::Tile, 10.0))
        .on_press_maybe(enabled.then_some(msg))
        .into()
    };
    let cells = vec![
        item("mail-send-symbolic", "Notificação de teste", Message::NotifSendTest),
        item(Section::Capture.icon(), "Testar recorte", Message::RunScreenshot),
        item("media-playback-start-symbolic", "Executar backup", backup_message()),
        item(Section::Security.icon(), "Verificar segurança", Message::RefreshAll),
        item("view-refresh-symbolic", "Reiniciar painel", Message::RestartPanel),
        item("system-lock-screen-symbolic", "Bloquear tela", Message::GreeterTestLock),
    ];
    let mut col = widget::column::with_capacity(4).spacing(8);
    let mut it = cells.into_iter();
    while let (Some(a), b) = (it.next(), it.next()) {
        let mut r = widget::row::with_capacity(2).spacing(8).push(a);
        r = match b {
            Some(b) => r.push(b),
            None => r.push(widget::Space::new().width(Length::Fill)),
        };
        col = col.push(r);
    }
    panel(
        widget::column::with_children(vec![heading("Ações rápidas"), col.into()])
            .spacing(10)
            .into(),
        [CARD_PADDING, CARD_PADDING],
    )
    .into()
}

fn recent_group(app: &ControlPanel) -> Element<'_, Message> {
    let now = Instant::now();
    let mut col = widget::column::with_capacity(RECENT_SHOWN * 2 + 1).push(
        widget::container(heading("Alterados recentemente")).padding([0, 0, 6, 0]),
    );
    for (i, c) in app.recent.items().iter().enumerate().take(RECENT_SHOWN) {
        let when = now.saturating_duration_since(c.at);
        let when = if when.as_secs() < 60 { "agora".to_owned() } else { format!("há {}", span(when)) };
        if i > 0 {
            col = col.push(row::hairline());
        }
        let mut line = widget::row::with_capacity(3)
            .spacing(12)
            .align_y(Alignment::Center)
            .padding([9, 0])
            .push(
                widget::column::with_children(vec![
                    widget::text(c.label.as_str()).size(13.0).into(),
                    faint(c.section.label()),
                ])
                .spacing(1)
                .width(Length::Fill),
            )
            .push(faint(when));
        if c.undo.is_some() {
            line = line.push(
                widget::button::custom(widget::text("Desfazer").size(12.0).font(row::medium()))
                    .padding([6, 10])
                    .class(row::btn_class(Btn::Link, 7.0))
                    .on_press(Message::Undo(i)),
            );
        }
        col = col.push(line);
    }
    if app.recent.items().is_empty() {
        col = col.push(widget::container(faint("Nada alterado nesta sessão.")).padding([9, 0]));
    }
    panel(col.into(), [CARD_PADDING, CARD_PADDING]).into()
}

pub(super) fn home_view(app: &ControlPanel) -> Element<'_, Message> {
    let title = widget::column::with_children(vec![
        widget::text("Visão geral").size(26.0).font(cosmic::font::bold()).into(),
        widget::text(hardware_line(app)).size(13.0).class(theme::Text::Custom(row::dimmed)).into(),
    ])
    .spacing(4);
    let mut page: Vec<Element<'_, Message>> = vec![title.into()];
    for a in alerts(&alert_input(app)) {
        page.push(alert_card(a, app.busy));
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
    page.push(
        widget::row::with_children(vec![
            widget::container(quick_actions(app)).width(Length::FillPortion(1)).into(),
            widget::container(recent_group(app)).width(Length::FillPortion(1)).into(),
        ])
        .spacing(GRID_GAP)
        .into(),
    );
    widget::column::with_children(page).spacing(22).into()
}
