use cosmic::Element;
use crate::control_panel::{ControlPanel, Message};
use crate::control_panel::health::is_stopped;
use super::row::{self, BtnStyle, Tone};
use super::shell::action_button;

fn updater_group(app: &ControlPanel) -> Element<'_, Message> {
    let repo = &app.snapshot.repo;
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
    let (pill, tone) = if app.snapshot.update_timer_active {
        ("Ativado", Tone::Ok)
    } else {
        ("Desativado", Tone::Neutral)
    };
    row::group(
        "Atualizador automático",
        None,
        false,
        vec![
            row::status("Atualizar a cada boot", None, pill, tone, Vec::new()),
            row::kv("Repositório", repo_path),
            row::kv("Versão atual", commit_line),
            row::buttons(
                "Perfil visual",
                Some("Leva suas preferências para outro PC."),
                vec![row::button("Exportar", Message::ExportConfig, BtnStyle::Normal)],
            ),
        ],
    )
}

fn services_group(app: &ControlPanel) -> Element<'_, Message> {
    let rows: Vec<Element<'_, Message>> = app
        .snapshot
        .units
        .iter()
        .map(|u| {
            let health = app.unit_health.iter().find(|(name, _)| *name == u.unit).map(|(_, h)| h);
            let stopped = health.is_some_and(is_stopped);
            let pill = if stopped {
                ("Parado", Tone::Warn)
            } else if u.active {
                ("Ativo", Tone::Ok)
            } else {
                ("Inativo", Tone::Neutral)
            };
            let action = (!u.is_system).then(|| {
                row::button(
                    if stopped { "Iniciar" } else { "Reiniciar" },
                    Message::RestartUserUnit(u.unit),
                    BtnStyle::Normal,
                )
            });
            let name = u.unit.strip_suffix(".service").unwrap_or(u.unit);
            row::list_item(name, u.label.to_owned(), Some(pill), action)
        })
        .collect();
    row::group("Serviços em segundo plano", None, false, rows)
}

pub(super) fn system_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = cosmic::theme::spacing();
    cosmic::widget::column::with_children(vec![updater_group(app), services_group(app)])
        .spacing(spacing.space_m)
        .into()
}

/// Botões do cabeçalho da página.
pub(super) fn system_actions(app: &ControlPanel) -> Vec<Element<'_, Message>> {
    vec![
        action_button("Verificar", Message::CheckUpdates, BtnStyle::Normal, !app.busy),
        action_button("Atualizar agora", Message::RunUpdateNow, BtnStyle::Primary, !app.busy),
    ]
}

/// Selo ao lado do título: "N parado(s)" quando algum serviço carregado está inativo.
pub(super) fn system_badge(app: &ControlPanel) -> Option<(String, Tone)> {
    let n = app.unit_health.iter().filter(|(_, h)| is_stopped(h)).count();
    (n > 0).then(|| (format!("{n} parado(s)"), Tone::Warn))
}
