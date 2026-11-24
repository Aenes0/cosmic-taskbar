use std::time::SystemTime;

use chrono::{DateTime, Local};
use cosmic::Element;
use crate::control_panel::{ControlPanel, Message};
use crate::control_panel::health::backup_late;
use super::row::{self, BtnStyle, Tone};
use super::shell::action_button;

/// "dd/mm HH:MM" no fuso local, ou `empty` quando não há horário.
fn when(t: Option<SystemTime>, empty: &str) -> String {
    match t {
        Some(t) => DateTime::<Local>::from(t).format("%d/%m %H:%M").to_string(),
        None => empty.to_owned(),
    }
}

fn restic_group(app: &ControlPanel) -> Element<'_, Message> {
    let snap = &app.snapshot;
    let times = &app.backup_times;
    let late = backup_late(times, SystemTime::now());
    let (timer_pill, timer_tone) = if snap.backup_restic_timer_active {
        ("Ativo", Tone::Ok)
    } else if snap.backup_restic_installed {
        ("Instalado (inativo)", Tone::Warn)
    } else {
        ("Não instalado", Tone::Neutral)
    };
    let (drive_pill, drive_tone) = if snap.backup_drive_mounted {
        ("Montado", Tone::Ok)
    } else {
        ("Desmontado", Tone::Warn)
    };
    row::group(
        "Backup diário · Restic",
        Some("Salva Repositorios, Documentos, Imagens, ~/Projetos, ~/.ssh, ~/.gnupg, ~/.config e ~/.local/share."),
        false,
        vec![
            row::status(
                "Agendamento",
                Some("Todo dia às 12:00."),
                timer_pill,
                timer_tone,
                vec![row::button(
                    "Reativar",
                    Message::EnableUserUnit("backup-restic.timer"),
                    BtnStyle::Normal,
                )],
            ),
            row::list_item(
                "Último backup",
                when(times.last, "nunca"),
                if late { Some(("Atrasado", Tone::Warn)) } else { None },
                None,
            ),
            row::list_item("Próximo backup", when(times.next, "—"), None, None),
            row::status(
                "Destino",
                Some("/mnt/devdrive"),
                drive_pill,
                drive_tone,
                Vec::new(),
            ),
            row::kv("Retenção", "7 diários + 4 semanais".to_owned()),
            row::buttons(
                "Snapshots",
                None,
                vec![row::button(
                    "Listar snapshots",
                    Message::OpenInTerminal(
                        "Snapshots Restic",
                        "RESTIC_REPOSITORY=/mnt/devdrive/backup-restic RESTIC_PASSWORD_FILE=$HOME/.config/restic/senha restic snapshots".to_owned(),
                    ),
                    BtnStyle::Normal,
                )],
            ),
        ],
    )
}

fn btrfs_group(app: &ControlPanel) -> Element<'_, Message> {
    let snap = &app.snapshot;
    let state = |active: bool| if active { ("Ativo", Tone::Ok) } else { ("Inativo", Tone::Warn) };
    let (sn_pill, sn_tone) = state(snap.snapper_timer_active);
    let (ft_pill, ft_tone) = state(snap.fstrim_timer_active);
    row::group(
        "Btrfs e SSD",
        None,
        false,
        vec![
            row::status(
                "Snapper",
                Some("Timeline semanal no /home, snap-pac no /."),
                sn_pill,
                sn_tone,
                vec![row::button(
                    "Ver snapshots",
                    Message::OpenInTerminal("Snapshots Snapper", "sudo snapper list".to_owned()),
                    BtnStyle::Normal,
                )],
            ),
            row::status(
                "fstrim.timer",
                Some("TRIM semanal do SSD, domingo às 04:00."),
                ft_pill,
                ft_tone,
                vec![row::button(
                    "Agendamentos",
                    Message::OpenInTerminal(
                        "Timers de Disco",
                        "systemctl list-timers snapper-timeline.timer fstrim.timer --no-pager".to_owned(),
                    ),
                    BtnStyle::Normal,
                )],
            ),
        ],
    )
}

fn claude_group(app: &ControlPanel) -> Element<'_, Message> {
    let snap = &app.snapshot;
    let (pill, tone) = if snap.claude_sync_active {
        ("Ativo", Tone::Ok)
    } else if snap.claude_sync_installed {
        ("Instalado (inativo)", Tone::Warn)
    } else {
        ("Não instalado", Tone::Neutral)
    };
    let export_cmd = concat!(
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
    );
    row::group(
        "Sincronização do Claude",
        Some("Espelha sessões entre as 2 contas locais e leva a estrutura do Claude (sem credenciais) para outro PC."),
        false,
        vec![
            row::status(
                "Sync local",
                Some("Contas deste PC."),
                pill,
                tone,
                vec![row::button(
                    "Sincronizar",
                    Message::OpenInTerminal(
                        "Claude Sync Contas",
                        "python3 \"$HOME/claude-migracao/sincronizar_contas.py\"".to_owned(),
                    ),
                    BtnStyle::Normal,
                )],
            ),
            row::buttons(
                "Outro PC",
                Some("Gera um pacote para importar."),
                vec![row::button(
                    "Exportar pacote",
                    Message::OpenInTerminal("Exportar Claude Bundle", export_cmd.to_owned()),
                    BtnStyle::Normal,
                )],
            ),
        ],
    )
}

pub(super) fn backup_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = cosmic::theme::spacing();
    cosmic::widget::column::with_children(vec![
        restic_group(app),
        btrfs_group(app),
        claude_group(app),
    ])
    .spacing(spacing.space_m)
    .into()
}

/// Botões do cabeçalho da página.
pub(super) fn backup_actions(app: &ControlPanel) -> Vec<Element<'_, Message>> {
    vec![
        action_button("Atualizar status", Message::RefreshAll, BtnStyle::Normal, !app.busy),
        action_button(
            "Executar backup",
            Message::OpenInTerminal("Backup Restic", "~/.local/bin/backup-restic".to_owned()),
            BtnStyle::Primary,
            !app.busy,
        ),
    ]
}

/// Selo ao lado do título: "Atrasado" quando o último disparo passou de 26 h.
pub(super) fn backup_badge(app: &ControlPanel) -> Option<(String, Tone)> {
    backup_late(&app.backup_times, SystemTime::now()).then(|| ("Backup atrasado".to_owned(), Tone::Warn))
}
