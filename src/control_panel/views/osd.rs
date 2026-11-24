// SPDX-License-Identifier: GPL-3.0-only

//! OSD de Dispositivos: estado do serviço, duração e alertas de bateria, tecla de emoji.
//! Cada controle grava pelas mesmas `Message` de sempre; quem reinicia o serviço é a barra
//! "Aplicar" da moldura.

use std::sync::LazyLock;

use cosmic::{Element, theme, widget};

use super::row::{self, BtnStyle, Tone};
use super::shell::action_button;
use crate::control_panel::{ControlPanel, Message, Section};
use crate::logi::config as osd_config;

/// Intervalos de verificação de bateria oferecidos (minutos), os mesmos de antes.
const INTERVAL_MINS: &[u64] = &[5, 15, 30, 60];

static INTERVAL_OPTIONS: LazyLock<Vec<String>> = LazyLock::new(|| {
    INTERVAL_MINS
        .iter()
        .map(|&m| {
            if m >= 60 && m % 60 == 0 {
                format!("{} h", m / 60)
            } else {
                format!("{m} min")
            }
        })
        .collect()
});

fn service_group(app: &ControlPanel) -> Element<'_, Message> {
    let active = app
        .snapshot
        .units
        .iter()
        .find(|u| u.unit == "logi-osd.service")
        .is_some_and(|u| u.active);
    let rows: Vec<Element<'_, Message>> = vec![
        row::status(
            "logi-osd",
            Some("Avisos de Caps/Num/Scroll Lock, troca de host/DPI, bateria de periféricos e atalhos. Posição e estilo vêm de Notificações."),
            if active { "Ativo" } else { "Inativo" },
            if active { Tone::Ok } else { Tone::Warn },
            vec![
                action_button(
                    "Reiniciar",
                    Message::RestartUserUnit("logi-osd.service"),
                    BtnStyle::Normal,
                    !app.busy,
                ),
                action_button(
                    "Abrir Notificações",
                    Message::SelectSection(Section::Notifications),
                    BtnStyle::Normal,
                    true,
                ),
            ],
        ),
        row::kv(
            "Arquivo de configuração",
            osd_config::config_path().to_string_lossy().into_owned(),
        ),
    ];
    row::group("Serviço", None, false, rows)
}

fn duration_group(app: &ControlPanel) -> Element<'_, Message> {
    let cfg = &app.osd_config;
    let dur_ms = cfg.duration.as_millis() as u64;
    let interval_mins = (cfg.legacy_battery_interval.as_secs() / 60).max(1);
    let max_critical = cfg.battery_low.saturating_sub(1).max(1);
    let rows: Vec<Element<'_, Message>> = vec![
        row::slider(
            "Duração da pílula",
            Some("Quanto tempo cada aviso fica na tela."),
            200.0..=10000.0,
            100.0,
            dur_ms.min(10000) as f32,
            format!("{dur_ms} ms"),
            ("200 ms".to_owned(), "10000 ms".to_owned()),
            |v| Message::OsdDurationMs(((v / 100.0).round() as u64) * 100),
            None,
        ),
        row::slider(
            "Alerta de bateria baixa",
            Some("Avisa quando a bateria de um periférico chega a este nível."),
            5.0..=50.0,
            1.0,
            cfg.battery_low.clamp(5, 50) as f32,
            format!("{}%", cfg.battery_low),
            ("5%".to_owned(), "50%".to_owned()),
            |v| Message::OsdBatteryLow(v.round() as u8),
            None,
        ),
        row::slider(
            "Alerta de bateria crítica",
            Some("Sempre abaixo do alerta de bateria baixa."),
            1.0..=max_critical as f32,
            1.0,
            cfg.battery_critical as f32,
            format!("{}%", cfg.battery_critical),
            ("1%".to_owned(), format!("{max_critical}%")),
            |v| Message::OsdBatteryCritical(v.round() as u8),
            None,
        ),
        row::select(
            "Verificar bateria a cada",
            Some("Intervalo entre as leituras da bateria dos periféricos."),
            &INTERVAL_OPTIONS,
            INTERVAL_MINS.iter().position(|&m| m == interval_mins),
            |i| Message::OsdBatteryIntervalMins(INTERVAL_MINS[i]),
        ),
    ];
    row::group("Duração e alertas", None, false, rows)
}

fn emoji_group(app: &ControlPanel) -> Element<'_, Message> {
    let rows: Vec<Element<'_, Message>> = vec![row::input(
        "Comando da tecla Emoji",
        Some("Executado ao pressionar a tecla Emoji de periféricos suportados. Vazio = padrão."),
        "Ex.: smile ou cosmic-launcher",
        app.osd_config.emoji_command.as_deref().unwrap_or(""),
        Message::OsdEmojiCommand,
        None,
    )];
    row::group("Tecla de emoji", None, false, rows)
}

pub(super) fn devices_osd_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    widget::column::with_children(vec![
        service_group(app),
        duration_group(app),
        emoji_group(app),
    ])
    .spacing(spacing.space_m)
    .into()
}

/// Botões do cabeçalho da página (o "Aplicar (reinicia o OSD)" vive na barra do rodapé).
pub(super) fn devices_osd_actions(app: &ControlPanel) -> Vec<Element<'_, Message>> {
    vec![
        action_button("Restaurar padrão", Message::OsdReset, BtnStyle::Normal, !app.busy),
        action_button("Testar OSD", Message::OsdRunDemo, BtnStyle::Primary, !app.busy),
    ]
}

/// Selo ao lado do título: sem selo por enquanto.
pub(super) fn devices_osd_badge(_app: &ControlPanel) -> Option<(String, Tone)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control_panel::chrome;

    #[test]
    fn grupos_do_osd_existem_no_indice() {
        let grupos: Vec<&str> = chrome::section_groups(Section::DevicesOsd)
            .into_iter()
            .map(|(g, _)| g)
            .collect();
        for g in ["Serviço", "Duração e alertas", "Tecla de emoji"] {
            assert!(grupos.contains(&g), "{g} não está no índice de busca");
        }
    }

    #[test]
    fn intervalos_de_verificacao_preservam_as_opcoes_de_antes() {
        assert_eq!(*INTERVAL_OPTIONS, vec!["5 min", "15 min", "30 min", "1 h"]);
    }
}
