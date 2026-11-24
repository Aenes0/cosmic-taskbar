// SPDX-License-Identifier: GPL-3.0-only

//! Barra: espaçamento, ícones (arrastar/ciclar) e apps ocultos da bandeja.
use cosmic::{Element, theme, widget};

use super::legacy::{TileTone, draggable_grid_tile, tile_grid, tone_pill};
use super::row::{self, BtnStyle, Tone};
use super::shell::action_button;
use crate::central::{bar_spacing::BarSpacing, layout::Show as BarShow};
use crate::control_panel::{ControlPanel, Message};

const GRID_COLUMNS: usize = 4;

pub(super) fn bar_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let dragging = app.dragging_bar;

    let spacing_names: Vec<&str> = BarSpacing::ALL.iter().map(|s| s.name()).collect();
    let spacing_sel = BarSpacing::ALL
        .iter()
        .position(|s| *s == app.bar_spacing)
        .unwrap_or(0);

    let make_tile = |item: crate::central::layout::BarItem| -> Element<'_, Message> {
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

    let ordered = app.panel_icons.ordered(&app.reorderable_bar_items());
    let (visible, hidden): (Vec<_>, Vec<_>) = ordered
        .into_iter()
        .partition(|&item| app.panel_icons.get(item) != BarShow::Never);

    let legend = widget::row::with_children(vec![
        tone_pill("Sempre", TileTone::Teal),
        tone_pill("Ativo", TileTone::Amber),
        tone_pill("Nunca", TileTone::Muted),
    ])
    .spacing(spacing.space_xxs);

    let mut icon_rows: Vec<Element<'_, Message>> = vec![row::segmented(
        "Espaçamento entre ícones",
        None,
        &spacing_names,
        spacing_sel,
        |i| Message::SetBarSpacing(BarSpacing::ALL[i]),
    )];
    icon_rows.push(row::buttons(
        "Ícones na barra, na ordem exibida",
        Some("Clique no cartão para alternar; arraste pelos pontos para reordenar."),
        vec![legend.into()],
    ));
    if visible.is_empty() {
        icon_rows.push(row::note(
            "Nenhum ícone ativo. Clique em um cartão abaixo para adicionar.",
        ));
    } else {
        icon_rows.push(
            widget::container(tile_grid(
                visible.into_iter().map(&make_tile).collect(),
                GRID_COLUMNS,
            ))
            .padding(spacing.space_s)
            .into(),
        );
    }
    if !hidden.is_empty() {
        icon_rows.push(row::note("Ocultos da barra (clique para ativar)"));
        icon_rows.push(
            widget::container(tile_grid(
                hidden.into_iter().map(&make_tile).collect(),
                GRID_COLUMNS,
            ))
            .padding(spacing.space_s)
            .into(),
        );
    }

    let mut tray_rows: Vec<Element<'_, Message>> = Vec::new();
    if app.tray_config.hidden.is_empty() {
        tray_rows.push(row::note(
            "Nenhum app oculto. Apps novos aparecem na barra por padrão.",
        ));
    } else {
        for key in &app.tray_config.hidden {
            tray_rows.push(row::list_item(
                key.as_str(),
                "Oculto da barra".to_owned(),
                None,
                Some(row::button(
                    "Mostrar na barra",
                    Message::ToggleTrayHidden(key.clone()),
                    BtnStyle::Normal,
                )),
            ));
        }
    }

    widget::column::with_children(vec![
        row::group("Ícones na barra", None, false, icon_rows),
        row::group("Apps em segundo plano ocultos", None, false, tray_rows),
    ])
    .spacing(spacing.space_m)
    .into()
}

/// Botões do cabeçalho da página.
pub(super) fn bar_actions(app: &ControlPanel) -> Vec<Element<'_, Message>> {
    vec![action_button("Reiniciar painel", Message::RestartPanel, BtnStyle::Normal, !app.busy)]
}

/// Selo ao lado do título: sem selo por enquanto.
pub(super) fn bar_badge(_app: &ControlPanel) -> Option<(String, Tone)> {
    None
}
