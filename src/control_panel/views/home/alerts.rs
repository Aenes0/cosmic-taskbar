// SPDX-License-Identifier: GPL-3.0-only

//! Alertas da Visão geral (lógica pura, testável) e formatação de durações.

use std::time::{Duration, SystemTime};

use crate::control_panel::Section;
use crate::control_panel::data::UnitStatus;
use crate::control_panel::health::{self, TimerTimes, UnitHealth};
use crate::control_panel::Message;

/// Nota de proteção abaixo da qual o selo da seção Segurança deixa de ser "bom" ("Pode melhorar").
pub(super) const SECURITY_RECOMMENDED: u8 = 80;

/// O que o botão direto de um alerta faz.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AlertAction {
    /// Só leva à seção (unidade de sistema, destino de backup).
    Open,
    RestartUnit(&'static str),
    RunBackup,
    VerifySecurity,
    ApplyPower,
    ApplyOsd,
}

impl AlertAction {
    pub(super) fn label(&self) -> &'static str {
        match self {
            AlertAction::Open => "Abrir",
            AlertAction::RestartUnit(_) => "Reiniciar",
            AlertAction::RunBackup => "Executar",
            AlertAction::VerifySecurity => "Verificar agora",
            AlertAction::ApplyPower | AlertAction::ApplyOsd => "Aplicar",
        }
    }

    pub(super) fn message(&self) -> Message {
        match self {
            AlertAction::Open => Message::None,
            AlertAction::RestartUnit(unit) => Message::RestartUserUnit(unit),
            AlertAction::RunBackup => super::backup_message(),
            AlertAction::VerifySecurity => Message::RefreshAll,
            AlertAction::ApplyPower => Message::ApplyPower,
            AlertAction::ApplyOsd => Message::ApplyOsd,
        }
    }
}

/// Um item da faixa "Alertas".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alert {
    pub tone: AlertTone,
    pub title: String,
    pub detail: String,
    /// Seção para onde "Detalhes" / "Abrir" levam.
    pub section: Section,
    pub action: AlertAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlertTone {
    Warn,
    Bad,
}

/// Dados de que os alertas dependem (recebe valores, não o `ControlPanel`, para ser testável).
pub struct AlertInput<'a> {
    pub units: &'a [UnitStatus],
    pub health: &'a [(&'static str, UnitHealth)],
    /// Nota 0–100; 0 = ainda não carregada.
    pub security_score: u8,
    pub backup_times: &'a TimerTimes,
    pub backup_installed: bool,
    pub backup_drive_mounted: bool,
    pub power_pending: bool,
    pub osd_pending: bool,
    pub now: SystemTime,
}

/// Alertas da Visão geral, na ordem: serviços parados, proteção, backup, pendências.
pub fn alerts(i: &AlertInput<'_>) -> Vec<Alert> {
    let mut out = Vec::new();
    for (unit, h) in i.health {
        if !health::is_stopped(h) {
            continue;
        }
        let Some(u) = i.units.iter().find(|u| u.unit == *unit) else {
            continue;
        };
        let action = if u.is_system {
            AlertAction::Open
        } else {
            AlertAction::RestartUnit(u.unit)
        };
        out.push(Alert {
            tone: AlertTone::Warn,
            title: format!("{} parado", u.label),
            detail: u.unit.to_owned(),
            section: Section::System,
            action,
        });
    }
    if i.security_score > 0 && i.security_score < SECURITY_RECOMMENDED {
        out.push(Alert {
            tone: AlertTone::Warn,
            title: "Proteção abaixo do recomendado".to_owned(),
            detail: format!("Nota {}/100", i.security_score),
            section: Section::Security,
            action: AlertAction::VerifySecurity,
        });
    }
    if health::backup_late(i.backup_times, i.now) {
        out.push(Alert {
            tone: AlertTone::Bad,
            title: "Backup atrasado".to_owned(),
            detail: "Último backup há mais de 26 h".to_owned(),
            section: Section::Backup,
            action: AlertAction::RunBackup,
        });
    }
    if i.backup_installed && !i.backup_drive_mounted {
        out.push(Alert {
            tone: AlertTone::Warn,
            title: "Disco de backup desmontado".to_owned(),
            detail: "/mnt/devdrive não está montado".to_owned(),
            section: Section::Backup,
            action: AlertAction::Open,
        });
    }
    if i.power_pending {
        out.push(Alert {
            tone: AlertTone::Warn,
            title: "Energia: mudanças ainda não aplicadas".to_owned(),
            detail: "Gravadas, mas o hardware ainda não recebeu".to_owned(),
            section: Section::Power,
            action: AlertAction::ApplyPower,
        });
    }
    if i.osd_pending {
        out.push(Alert {
            tone: AlertTone::Warn,
            title: "OSD: mudanças ainda não aplicadas".to_owned(),
            detail: "O serviço ainda roda com a configuração anterior".to_owned(),
            section: Section::DevicesOsd,
            action: AlertAction::ApplyOsd,
        });
    }
    out
}

/// "5 min", "3 h", "2 d" para uma duração.
pub(super) fn span(d: Duration) -> String {
    let mins = d.as_secs() / 60;
    if mins < 60 {
        format!("{} min", mins.max(1))
    } else if mins < 60 * 24 {
        format!("{} h", mins / 60)
    } else {
        format!("{} d", mins / (60 * 24))
    }
}

pub(super) fn ago(now: SystemTime, t: SystemTime) -> String {
    match now.duration_since(t) {
        Ok(d) if d.as_secs() >= 60 => format!("há {}", span(d)),
        _ => "agora".to_owned(),
    }
}

pub(super) fn ahead(now: SystemTime, t: SystemTime) -> String {
    match t.duration_since(now) {
        Ok(d) if d.as_secs() >= 60 => format!("em {}", span(d)),
        _ => "em instantes".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: Duration = Duration::from_secs(24 * 3600);

    fn unit(unit: &'static str, label: &'static str, is_system: bool) -> UnitStatus {
        UnitStatus { unit, label, active: false, is_system }
    }

    fn stopped() -> UnitHealth {
        UnitHealth { loaded: true, active: false }
    }

    fn calm<'a>(units: &'a [UnitStatus], health: &'a [(&'static str, UnitHealth)], times: &'a TimerTimes) -> AlertInput<'a> {
        AlertInput {
            units,
            health,
            security_score: 92,
            backup_times: times,
            backup_installed: true,
            backup_drive_mounted: true,
            power_pending: false,
            osd_pending: false,
            now: SystemTime::UNIX_EPOCH + 100 * DAY,
        }
    }

    #[test]
    fn nada_a_resolver_nao_gera_alertas() {
        let units = [unit("a.service", "A", false)];
        let health = [("a.service", UnitHealth { loaded: true, active: true })];
        let times = TimerTimes::default();
        assert!(alerts(&calm(&units, &health, &times)).is_empty());
    }

    #[test]
    fn unidade_de_sistema_parada_gera_alerta_com_acao_abrir() {
        let units = [unit("power-profiles-daemon.service", "Perfis de energia", true)];
        let health = [("power-profiles-daemon.service", stopped())];
        let times = TimerTimes::default();
        let list = alerts(&calm(&units, &health, &times));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].action, AlertAction::Open);
        assert_eq!(list[0].action.label(), "Abrir");
        assert_eq!(list[0].section, Section::System);
    }

    #[test]
    fn unidade_de_usuario_parada_oferece_reiniciar_e_ausente_nao_alerta() {
        let units = [unit("logi-osd.service", "OSD", false), unit("x.service", "X", false)];
        let health = [
            ("logi-osd.service", stopped()),
            ("x.service", UnitHealth { loaded: false, active: false }),
        ];
        let times = TimerTimes::default();
        let list = alerts(&calm(&units, &health, &times));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].action, AlertAction::RestartUnit("logi-osd.service"));
    }

    #[test]
    fn protecao_backup_e_pendencias_geram_alertas_proprios() {
        let times = TimerTimes {
            last: Some(SystemTime::UNIX_EPOCH + 98 * DAY),
            next: None,
        };
        let mut input = calm(&[], &[], &times);
        input.security_score = 60;
        input.backup_drive_mounted = false;
        input.power_pending = true;
        input.osd_pending = true;
        let actions: Vec<AlertAction> = alerts(&input).into_iter().map(|a| a.action).collect();
        assert_eq!(
            actions,
            vec![
                AlertAction::VerifySecurity,
                AlertAction::RunBackup,
                AlertAction::Open,
                AlertAction::ApplyPower,
                AlertAction::ApplyOsd,
            ]
        );
    }

    #[test]
    fn nota_ainda_nao_carregada_nao_alerta() {
        let times = TimerTimes::default();
        let mut input = calm(&[], &[], &times);
        input.security_score = 0;
        assert!(alerts(&input).is_empty());
    }

    #[test]
    fn duracoes_em_texto() {
        assert_eq!(span(Duration::from_secs(5 * 60)), "5 min");
        assert_eq!(span(Duration::from_secs(3 * 3600)), "3 h");
        assert_eq!(span(2 * DAY), "2 d");
    }
}
