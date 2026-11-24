// SPDX-License-Identifier: GPL-3.0-only

//! Lógica pura da moldura do Painel de Controle (entrega painel-b, Task 9): resultados da
//! paleta numa lista só, grupos de "Ir para", ação da paleta → `Message`, alertas por categoria.
//! Nada aqui toca disco, rede ou widgets; tudo é testável.

use super::{
    Message, Section,
    health::{self, UnitHealth},
    nav::Category,
    search_index::{self, Entry, EntryKind, INDEX},
};

/// Resultados da paleta como uma lista só, na ordem em que a paleta os mostra:
/// Ajustes, Ações, COSMIC Settings. O cursor da paleta indexa esta lista.
pub fn flat_results(query: &str) -> Vec<&'static Entry> {
    let r = search_index::search(query);
    r.settings
        .into_iter()
        .chain(r.actions)
        .chain(r.native)
        .collect()
}

/// Grupos distintos de uma seção no índice, na ordem do índice: `(grupo, avançado)`.
/// Só entradas de ajuste contam; "Ações da página" e páginas nativas não são grupos da página.
pub fn section_groups(section: Section) -> Vec<(&'static str, bool)> {
    let mut out: Vec<(&'static str, bool)> = Vec::new();
    for e in INDEX
        .iter()
        .filter(|e| e.section == section && e.kind == EntryKind::Setting)
    {
        if !out.iter().any(|(g, _)| *g == e.group) {
            out.push((e.group, e.advanced));
        }
    }
    out
}

/// Nomes dos grupos avançados de uma seção (ordem do índice).
pub fn advanced_groups(section: Section) -> Vec<&'static str> {
    section_groups(section)
        .into_iter()
        .filter_map(|(g, adv)| adv.then_some(g))
        .collect()
}

/// Posição aproximada de um grupo no rolável da página: `índice / total`, contando só os grupos
/// visíveis (os avançados só entram com o avançado aberto). `None` se o grupo não está visível.
/// É uma aproximação: o iced não informa a posição exata de um bloco dentro do scrollable.
pub fn jump_fraction(groups: &[(&'static str, bool)], group: &str, advanced_open: bool) -> Option<f32> {
    let visible: Vec<&str> = groups
        .iter()
        .filter(|(_, adv)| advanced_open || !adv)
        .map(|(g, _)| *g)
        .collect();
    let idx = visible.iter().position(|g| *g == group)?;
    Some(idx as f32 / visible.len() as f32)
}

/// `true` se o grupo é avançado na seção.
pub fn is_advanced_group(section: Section, group: &str) -> bool {
    section_groups(section)
        .iter()
        .any(|(g, adv)| *g == group && *adv)
}

/// `Message` que executa a ação da paleta (Ctrl+Enter) quando existe um mapeamento simples.
/// Sem mapeamento, o chamador só leva o usuário à seção.
pub fn action_message(entry: &Entry) -> Option<Message> {
    if entry.kind != EntryKind::Action {
        return None;
    }
    Some(match (entry.section, entry.label) {
        (Section::Power, "Restaurar padrões") => Message::PowerResetDefaults,
        (Section::Displays, "Ler monitores") => Message::RefreshDisplays,
        (Section::Network, "Abrir no COSMIC") => Message::OpenCosmicSettings(Some("network")),
        (Section::Camera, "Reescanear") => Message::RefreshCamera,
        (Section::Bar, "Reiniciar painel") => Message::RestartPanel,
        (Section::Central, "Restaurar tiles") => Message::CentralResetTiles,
        (Section::Monitor, "Restaurar padrão") => Message::MonitorReset,
        (Section::Notifications, "Restaurar padrão") => Message::NotifReset,
        (Section::Notifications, "Enviar teste") => Message::NotifSendTest,
        (Section::DevicesOsd, "Testar OSD") => Message::OsdRunDemo,
        (Section::Capture, "Abrir pasta") => Message::OpenScreenshotsDir,
        (Section::Capture, "Testar recorte") => Message::RunScreenshot,
        (Section::Security, "Verificar agora") => Message::RefreshAll,
        (Section::Backup, "Executar backup") => Message::OpenInTerminal(
            "Backup Restic",
            "~/.local/bin/backup-restic".to_owned(),
        ),
        (Section::System, "Verificar") => Message::CheckUpdates,
        (Section::System, "Atualizar agora") => Message::RunUpdateNow,
        _ => return None,
    })
}

/// Unidade rastreada que importa para a seção (a seção "Atualizações & Serviços" lista todas).
fn unit_matters(section: Section, unit: &str) -> bool {
    match section {
        Section::System => true,
        Section::Power => matches!(unit, "power-profiles-daemon.service" | "economia-energia.service"),
        Section::Central => matches!(unit, "central-panels.service" | "central-clipboard.service"),
        Section::Monitor => unit == "central-panels.service",
        Section::Notifications => unit == "central-notifications.service",
        Section::DevicesOsd => unit == "logi-osd.service",
        Section::Optional => unit == "cosmic-switcher.service",
        _ => false,
    }
}

/// Alguma unidade rastreada da seção está carregada e parada (unidade ausente não conta).
pub fn section_has_stopped(section: Section, units: &[(&'static str, UnitHealth)]) -> bool {
    units
        .iter()
        .any(|(unit, h)| unit_matters(section, unit) && health::is_stopped(h))
}

/// Ponto âmbar do trilho: alguma seção da categoria tem serviço parado ou pendência de aplicar.
pub fn category_alert(
    cat: Category,
    units: &[(&'static str, UnitHealth)],
    power_pending: bool,
    osd_pending: bool,
) -> bool {
    cat.sections().iter().any(|&s| {
        section_has_stopped(s, units)
            || (s == Section::Power && power_pending)
            || (s == Section::DevicesOsd && osd_pending)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stopped() -> UnitHealth {
        UnitHealth { loaded: true, active: false }
    }

    #[test]
    fn flat_results_keep_settings_then_actions_then_native() {
        let r = search_index::search("tela");
        let flat = flat_results("tela");
        assert_eq!(flat.len(), r.settings.len() + r.actions.len() + r.native.len());
        let kinds: Vec<EntryKind> = flat.iter().map(|e| e.kind).collect();
        let first_action = kinds.iter().position(|k| *k == EntryKind::Action);
        let first_native = kinds.iter().position(|k| *k == EntryKind::Native);
        if let (Some(a), Some(n)) = (first_action, first_native) {
            assert!(a < n);
        }
        assert!(flat_results("   ").is_empty());
    }

    #[test]
    fn power_groups_mark_cpu_gpu_platform_kernel_as_advanced() {
        let g = section_groups(Section::Power);
        assert_eq!(g.first(), Some(&("Plano ativo", false)));
        for adv in ["CPU", "GPU", "Plataforma", "Kernel, módulos e regras"] {
            assert!(g.contains(&(adv, true)), "{adv}");
        }
        assert!(!g.iter().any(|(name, _)| *name == "Ações da página"));
        assert_eq!(advanced_groups(Section::Power).len(), 4);
        assert!(is_advanced_group(Section::Power, "CPU"));
        assert!(!is_advanced_group(Section::Power, "Plano ativo"));
        assert!(advanced_groups(Section::Bar).is_empty());
    }

    #[test]
    fn jump_fraction_counts_only_visible_groups() {
        let g = section_groups(Section::Power);
        let basic = g.iter().filter(|(_, a)| !*a).count() as f32;
        assert_eq!(jump_fraction(&g, "Plano ativo", false), Some(0.0));
        assert_eq!(jump_fraction(&g, "Tela e bloqueio", false), Some(1.0 / basic));
        // avançado fechado: o grupo não está na página
        assert_eq!(jump_fraction(&g, "CPU", false), None);
        let total = g.len() as f32;
        assert_eq!(jump_fraction(&g, "CPU", true), Some(basic / total));
        assert_eq!(jump_fraction(&g, "Inexistente", true), None);
    }

    #[test]
    fn every_action_entry_is_mapped_or_knowingly_not() {
        let unmapped: Vec<_> = INDEX
            .iter()
            .filter(|e| e.kind == EntryKind::Action && action_message(e).is_none())
            .map(|e| (e.section, e.label))
            .collect();
        // Só a "Reaplicar recomendado" da Segurança (comando montado na própria seção) fica sem mapa.
        assert_eq!(unmapped, vec![(Section::Security, "Reaplicar recomendado")]);
        let setting = INDEX.iter().find(|e| e.kind == EntryKind::Setting).unwrap();
        assert!(action_message(setting).is_none());
    }

    #[test]
    fn stopped_units_alert_only_their_sections() {
        let units = [("logi-osd.service", stopped())];
        assert!(section_has_stopped(Section::DevicesOsd, &units));
        assert!(section_has_stopped(Section::System, &units));
        assert!(!section_has_stopped(Section::Power, &units));
        // unidade ausente ou ativa não alerta
        let ok = [
            ("logi-osd.service", UnitHealth { loaded: false, active: false }),
            ("cosmic-switcher.service", UnitHealth { loaded: true, active: true }),
        ];
        assert!(!section_has_stopped(Section::System, &ok));
    }

    #[test]
    fn category_dot_comes_from_stopped_units_or_pending_apply() {
        let none: [(&'static str, UnitHealth); 0] = [];
        assert!(!category_alert(Category::System, &none, false, false));
        assert!(category_alert(Category::System, &none, true, false)); // Energia pendente
        assert!(category_alert(Category::Appearance, &none, false, true)); // OSD pendente
        assert!(!category_alert(Category::Protection, &none, true, true));
        let units = [("cosmic-switcher.service", stopped())];
        assert!(category_alert(Category::Maintenance, &units, false, false));
        assert!(!category_alert(Category::Home, &units, true, true));
    }
}
