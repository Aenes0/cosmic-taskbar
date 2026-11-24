// SPDX-License-Identifier: GPL-3.0-only

//! Central (direita): tema, seções recolhíveis, ladrilhamento e entrada.
use cosmic::{Element, theme, widget};

use super::row::{self, BtnStyle, Tone};
use super::shell::action_button;
use crate::central::{
    layout::Section as CentralSection,
    services::{self, Touchpad},
};
use crate::control_panel::{ControlPanel, Message};

fn section_name(sec: CentralSection) -> &'static str {
    match sec {
        CentralSection::Notifications => "Notificações",
        CentralSection::Media => "Reprodutor de mídia",
        CentralSection::Calendar => "Calendário",
        CentralSection::Clipboard => "Área de transferência",
    }
}

pub(super) fn central_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();

    let appearance = row::group(
        "Aparência",
        None,
        false,
        vec![
            row::toggle("Modo escuro", Some("Tema escuro do COSMIC."), app.dark_mode, |_| {
                Message::CentralToggleDarkMode
            }),
            row::toggle(
                "Não perturbe",
                Some("Silencia notificações, exceto as críticas."),
                app.do_not_disturb,
                |_| Message::CentralToggleDnd,
            ),
        ],
    );

    let section_rows: Vec<Element<'_, Message>> = CentralSection::ALL
        .into_iter()
        .map(|sec| {
            row::toggle(
                section_name(sec),
                None,
                !app.central_layout.is_folded(sec),
                move |_| Message::CentralToggleSectionFold(sec),
            )
        })
        .collect();
    let sections = row::group("Seções abertas por padrão", None, false, section_rows);

    let tiling = row::group(
        "Janelas lado a lado",
        None,
        false,
        vec![
            row::toggle(
                "Ladrilhar novos espaços de trabalho",
                None,
                app.tiling_config.new_workspaces_tile,
                |_| Message::CentralToggleNewWorkspacesTile,
            ),
            row::toggle(
                "Destacar a janela ativa",
                Some("Borda colorida na janela em foco."),
                app.tiling_config.active_hint,
                |_| Message::CentralToggleActiveHint,
            ),
        ],
    );

    let mut input_rows: Vec<Element<'_, Message>> = Vec::new();
    if let Some(tp) = app.touchpad {
        const MODES: [Touchpad; 3] = [
            Touchpad::Enabled,
            Touchpad::DisabledOnExternalMouse,
            Touchpad::Disabled,
        ];
        let selected = MODES.iter().position(|m| *m == tp).unwrap_or(0);
        input_rows.push(row::segmented(
            "Touchpad",
            None,
            &["Ativado", "Desligar c/ mouse", "Desativado"],
            selected,
            |i| Message::CentralSetTouchpad(MODES[i]),
        ));
    } else {
        input_rows.push(row::note(
            "Nenhuma configuração de touchpad disponível neste sistema.",
        ));
    }
    if services::hot_edge_installed() {
        input_rows.push(row::buttons(
            "Hot Edge",
            Some("cosmic-ext-hot-edge"),
            vec![
                row::button("Configurar", Message::CentralOpenHotEdge, BtnStyle::Normal),
                widget::toggler(app.hot_edge)
                    .on_toggle(|_| Message::CentralToggleHotEdge)
                    .into(),
            ],
        ));
    } else {
        input_rows.push(row::status(
            "Hot Edge",
            Some("cosmic-ext-hot-edge"),
            "Não instalado",
            Tone::Neutral,
            vec![],
        ));
    }
    let input = row::group("Entrada e extensões", None, false, input_rows);

    widget::column::with_children(vec![appearance, sections, tiling, input])
        .spacing(spacing.space_m)
        .into()
}

/// Botões do cabeçalho da página.
pub(super) fn central_actions(app: &ControlPanel) -> Vec<Element<'_, Message>> {
    vec![action_button("Restaurar tiles", Message::CentralResetTiles, BtnStyle::Normal, !app.busy)]
}

/// Selo ao lado do título: sem selo por enquanto.
pub(super) fn central_badge(_app: &ControlPanel) -> Option<(String, Tone)> {
    None
}
