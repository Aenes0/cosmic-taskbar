// SPDX-License-Identifier: GPL-3.0-only

//! Pendência do botão "Aplicar" de Energia e OSD (spec §6).
//!
//! `Applied` guarda o que este painel já aplicou/leu do disco; a comparação com o que está
//! sendo editado diz se há algo a aplicar.

use super::power_config::{PowerProfileKind, PowerProfilesConfig, ProfileTuning};
use crate::logi::config as osd_config;

/// Estado "aplicado" conhecido pelo painel.
#[derive(Clone, Debug, Default)]
pub struct Applied {
    power: [Option<ProfileTuning>; 3],
    osd: Option<osd_config::Config>,
}

/// O que o botão "Aplicar" deve fazer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyAction {
    ApplyHardware,
    ActivateAndApply(PowerProfileKind),
}

fn slot(kind: PowerProfileKind) -> usize {
    match kind {
        PowerProfileKind::PowerSaver => 0,
        PowerProfileKind::Balanced => 1,
        PowerProfileKind::Performance => 2,
    }
}

impl Applied {
    /// Registra `tuning` como o que está aplicado para o plano `kind`.
    pub fn mark_power(&mut self, kind: PowerProfileKind, tuning: &ProfileTuning) {
        self.power[slot(kind)] = Some(tuning.clone());
    }

    /// Esquece a marca do plano `kind` (ex.: ao trocar o plano ativo).
    pub fn forget_power(&mut self, kind: PowerProfileKind) {
        self.power[slot(kind)] = None;
    }

    /// Registra `cfg` como o OSD vigente no último reinício. Guarda a forma normalizada, que é a
    /// que vai para o disco (`persist_osd_config`), para a comparação não acender sem diferença real.
    pub fn mark_osd(&mut self, cfg: &osd_config::Config) {
        self.osd = Some(cfg.normalized());
    }
}

/// O que o botão "Aplicar" da página Energia deve fazer.
///
/// - plano editado diferente do ativo: oferece ativar e aplicar (só a barra da página usa isto;
///   contagem, ponto do trilho e alerta do Início usam `power_dirty`);
/// - plano editado é o ativo, ou não há plano ativo conhecido (daemon parado/ausente): só há
///   pendência de hardware se existe marca e o tuning atual difere dela em campos de hardware;
///   sem marca não há base de comparação -> `None`.
pub fn power_pending(
    cfg: &PowerProfilesConfig,
    applied: &Applied,
    editing: PowerProfileKind,
    active: Option<PowerProfileKind>,
) -> Option<ApplyAction> {
    if active.is_some_and(|a| a != editing) {
        return Some(ApplyAction::ActivateAndApply(editing));
    }
    power_dirty(cfg, applied, editing).then_some(ApplyAction::ApplyHardware)
}

/// `true` se o plano editado tem ajuste de hardware gravado que ainda não foi aplicado: existe
/// marca e o tuning atual difere dela em campo de hardware. Olhar outro plano não conta.
pub fn power_dirty(cfg: &PowerProfilesConfig, applied: &Applied, editing: PowerProfileKind) -> bool {
    applied.power[slot(editing)]
        .as_ref()
        .is_some_and(|mark| hardware_fields_differ(mark, cfg.get(editing)))
}

/// `true` se o OSD editado difere do que estava no último reinício. Compara pela forma
/// normalizada (a que vai para o disco). Sem marca, não há pendência.
pub fn osd_pending(cfg: &osd_config::Config, applied: &Applied) -> bool {
    applied.osd.as_ref().is_some_and(|mark| *mark != cfg.normalized())
}

/// Compara dois tunings ignorando os 4 tempos de tela (aplicados ao Idle na hora).
/// Todo o resto conta como hardware: um campo novo em `ProfileTuning` é hardware por padrão.
pub fn hardware_fields_differ(a: &ProfileTuning, b: &ProfileTuning) -> bool {
    fn without_idle(t: &ProfileTuning) -> ProfileTuning {
        ProfileTuning {
            screen_off_mins: 0,
            lock_mins: 0,
            suspend_ac_mins: 0,
            suspend_battery_mins: 0,
            ..t.clone()
        }
    }
    without_idle(a) != without_idle(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_plan_pending_only_after_hardware_change() {
        let mut cfg = PowerProfilesConfig::defaults_for(false, false);
        let mut ap = Applied::default();
        let k = PowerProfileKind::Balanced;
        ap.mark_power(k, cfg.get(k));
        assert!(power_pending(&cfg, &ap, k, Some(k)).is_none());
        cfg.get_mut(k).screen_off_mins = 99; // tempo de tela: não é hardware
        assert!(power_pending(&cfg, &ap, k, Some(k)).is_none());
        cfg.get_mut(k).boost = !cfg.get(k).boost;
        assert!(matches!(
            power_pending(&cfg, &ap, k, Some(k)),
            Some(ApplyAction::ApplyHardware)
        ));
    }

    #[test]
    fn editing_inactive_plan_offers_activate() {
        let cfg = PowerProfilesConfig::defaults_for(false, false);
        let ap = Applied::default();
        assert!(matches!(
            power_pending(
                &cfg,
                &ap,
                PowerProfileKind::Performance,
                Some(PowerProfileKind::PowerSaver)
            ),
            Some(ApplyAction::ActivateAndApply(PowerProfileKind::Performance))
        ));
    }

    #[test]
    fn viewing_inactive_plan_without_change_is_not_dirty() {
        let mut cfg = PowerProfilesConfig::defaults_for(false, false);
        let mut ap = Applied::default();
        for k in [PowerProfileKind::PowerSaver, PowerProfileKind::Balanced, PowerProfileKind::Performance] {
            ap.mark_power(k, cfg.get(k));
        }
        let editing = PowerProfileKind::Performance;
        assert!(!power_dirty(&cfg, &ap, editing));
        // Só a barra da página Energia oferece ativar o plano que está sendo olhado.
        assert_eq!(
            power_pending(&cfg, &ap, editing, Some(PowerProfileKind::PowerSaver)),
            Some(ApplyAction::ActivateAndApply(editing))
        );
        cfg.get_mut(editing).boost = !cfg.get(editing).boost;
        assert!(power_dirty(&cfg, &ap, editing));
    }

    #[test]
    fn without_daemon_hardware_change_offers_apply_hardware() {
        let mut cfg = PowerProfilesConfig::defaults_for(false, false);
        let mut ap = Applied::default();
        let k = PowerProfileKind::Balanced;
        assert!(power_pending(&cfg, &ap, k, None).is_none()); // sem marca: sem base
        ap.mark_power(k, cfg.get(k));
        assert!(power_pending(&cfg, &ap, k, None).is_none());
        cfg.get_mut(k).boost = !cfg.get(k).boost;
        assert_eq!(power_pending(&cfg, &ap, k, None), Some(ApplyAction::ApplyHardware));
    }

    #[test]
    fn external_switch_clears_pending() {
        let mut cfg = PowerProfilesConfig::defaults_for(false, false);
        let mut ap = Applied::default();
        let k = PowerProfileKind::Balanced;
        ap.mark_power(k, cfg.get(k));
        cfg.get_mut(k).boost = !cfg.get(k).boost;
        ap.forget_power(k);
        ap.mark_power(k, cfg.get(k)); // o que o update faz ao receber troca externa
        assert!(power_pending(&cfg, &ap, k, Some(k)).is_none());
    }

    #[test]
    fn osd_pending_after_change() {
        let mut c = osd_config::Config::default();
        let mut ap = Applied::default();
        ap.mark_osd(&c);
        assert!(!osd_pending(&c, &ap));
        c.duration += std::time::Duration::from_millis(100);
        assert!(osd_pending(&c, &ap));
    }

    #[test]
    fn osd_difference_only_before_normalization_is_not_pending() {
        let saved = osd_config::Config::default();
        let mut ap = Applied::default();
        ap.mark_osd(&saved);
        // Em memória o comando traz espaços; ao gravar vira `None`, igual ao que está em disco.
        let edited = osd_config::Config { emoji_command: Some("   ".into()), ..saved.clone() };
        assert_ne!(edited, saved);
        assert!(!osd_pending(&edited, &ap));
        // E a marca com valor fora da faixa também é guardada normalizada.
        let raw = osd_config::Config { duration: std::time::Duration::from_millis(10), ..saved };
        ap.mark_osd(&raw);
        assert!(!osd_pending(&raw, &ap));
    }
}
