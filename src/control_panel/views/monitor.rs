// SPDX-License-Identifier: GPL-3.0-only

//! Monitor (esquerda): métricas da barra, cartões, processos, clima e consumo.
use std::sync::OnceLock;

use cosmic::{
    Element,
    iced::{Alignment, Length},
    theme, widget,
};

use super::legacy::{
    TileTone, draggable_grid_tile, monitor_panel_icon, monitor_poll_for, tile_grid, tone_pill,
};
use super::row::{self, BtnStyle, Tone};
use super::shell::action_button;
use crate::central::monitor::config::{
    BarItem as MonitorBarItem, BarMode as MonitorBarMode, CardMode as MonitorCardMode,
    PROCS_COUNT_OPTIONS, PollComponent as MonitorPollComponent, ProcSort as MonitorProcSort,
    WEATHER_MINS_OPTIONS,
};
use crate::control_panel::{ControlPanel, Message};

const POLL_OPTIONS: [(u64, &str); 4] = [(1, "1 s"), (2, "2 s"), (5, "5 s (padrão)"), (10, "10 s")];
const TILE_COLUMNS: usize = 3;

fn poll_labels() -> &'static [String] {
    static L: OnceLock<Vec<String>> = OnceLock::new();
    L.get_or_init(|| POLL_OPTIONS.iter().map(|(_, l)| (*l).to_owned()).collect())
}

fn weather_labels() -> &'static [String] {
    static L: OnceLock<Vec<String>> = OnceLock::new();
    L.get_or_init(|| WEATHER_MINS_OPTIONS.iter().map(|m| format!("{m} min")).collect())
}

fn count_labels() -> &'static [String] {
    static L: OnceLock<Vec<String>> = OnceLock::new();
    L.get_or_init(|| PROCS_COUNT_OPTIONS.iter().map(|n| n.to_string()).collect())
}

fn sort_labels() -> &'static [String] {
    static L: OnceLock<Vec<String>> = OnceLock::new();
    L.get_or_init(|| MonitorProcSort::ALL.iter().map(|s| s.name().to_owned()).collect())
}

fn bar_mode_names() -> Vec<&'static str> {
    MonitorBarMode::ALL.iter().map(|m| m.name()).collect()
}

fn bar_mode_index(mode: MonitorBarMode) -> usize {
    MonitorBarMode::ALL.iter().position(|m| *m == mode).unwrap_or(0)
}

fn bar_metrics_group(app: &ControlPanel) -> Element<'_, Message> {
    let mut rows: Vec<Element<'_, Message>> = Vec::new();
    for item in MonitorBarItem::ALL {
        if item == MonitorBarItem::Power {
            rows.push(row::status(item.name(), None, "Sempre visível", Tone::Ok, vec![]));
        } else if item.has_modes() {
            let current = app.monitor_settings.bar_mode(item);
            rows.push(row::segmented(
                item.name(),
                None,
                &bar_mode_names(),
                bar_mode_index(current),
                move |i| Message::MonitorBarMode(item, MonitorBarMode::ALL[i]),
            ));
        } else {
            rows.push(row::toggle(
                item.name(),
                None,
                app.monitor_settings.shows_on_bar(item),
                move |_| Message::MonitorToggleBar(item),
            ));
        }
    }
    rows.push(row::segmented(
        "Termômetro",
        Some("Termômetro vermelho ao lado dos ícones de CPU e GPU."),
        &bar_mode_names(),
        bar_mode_index(app.monitor_settings.thermometer),
        |i| Message::MonitorThermometer(MonitorBarMode::ALL[i]),
    ));
    rows.push(row::select(
        "Intervalo de leitura",
        Some("Com o popup fechado."),
        poll_labels(),
        POLL_OPTIONS
            .iter()
            .position(|(s, _)| *s == app.monitor_settings.poll_secs),
        |i| Message::MonitorPollSecs(POLL_OPTIONS[i].0),
    ));
    rows.push(row::note(
        "Em \"Quando alto\", a métrica aparece com 70% de uso ou mais, ou 65 °C ou mais (memória: 70% ou mais), e some 10 s após normalizar.",
    ));
    row::group("Métricas na barra", None, false, rows)
}

fn cards_group(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let dragging = app.dragging_monitor_card;
    let legend = widget::row::with_children(vec![
        tone_pill("Expandido", TileTone::Teal),
        tone_pill("Compacto", TileTone::Blue),
        tone_pill("Reduzido", TileTone::Amber),
        tone_pill("Oculto", TileTone::Muted),
    ])
    .spacing(spacing.space_xxs);

    let tiles: Vec<Element<'_, Message>> = app
        .monitor_settings
        .order
        .iter()
        .copied()
        .map(|panel| {
            let current = app.monitor_settings.card_mode(panel);
            let next = match current {
                MonitorCardMode::Full => MonitorCardMode::Compact,
                MonitorCardMode::Compact => MonitorCardMode::Minimal,
                MonitorCardMode::Minimal => MonitorCardMode::Hidden,
                MonitorCardMode::Hidden => MonitorCardMode::Full,
            };
            let (tone, status_label) = match current {
                MonitorCardMode::Full => (TileTone::Teal, "Expandido"),
                MonitorCardMode::Compact => (TileTone::Blue, "Compacto"),
                MonitorCardMode::Minimal => (TileTone::Amber, "Reduzido"),
                MonitorCardMode::Hidden => (TileTone::Muted, "Oculto"),
            };
            let poll_footer = monitor_poll_for(panel).map(|comp| {
                let secs = app.monitor_settings.component_poll(comp).clamp(1, 10);
                widget::row::with_children(vec![
                    widget::text::caption(format!("{secs}s"))
                        .width(Length::Fixed(26.0))
                        .into(),
                    widget::slider(1.0_f32..=10.0_f32, secs as f32, move |v| {
                        Message::MonitorComponentPoll(comp, v.round() as u64)
                    })
                    .step(1.0_f32)
                    .width(Length::Fill)
                    .into(),
                ])
                .spacing(spacing.space_xxs)
                .align_y(Alignment::Center)
                .into()
            });
            draggable_grid_tile(
                monitor_panel_icon(panel),
                panel.name(),
                status_label,
                tone,
                dragging == Some(panel),
                dragging.is_some(),
                Message::MonitorCardMode(panel, next),
                Message::MonitorDragStart(panel),
                Message::MonitorDragOver(panel),
                poll_footer,
            )
        })
        .collect();

    row::group(
        "Cartões do painel",
        None,
        false,
        vec![
            row::buttons(
                "Cartões do painel da esquerda",
                Some("Clique no cartão para alternar o modo; arraste pelos pontos para reordenar."),
                vec![legend.into()],
            ),
            widget::container(tile_grid(tiles, TILE_COLUMNS))
                .padding(spacing.space_s)
                .into(),
        ],
    )
}

fn processes_group(app: &ControlPanel) -> Element<'_, Message> {
    let settings = &app.monitor_settings;
    let (dir_label, dir_hint) = if settings.proc_sort_asc {
        ("▲ Crescente", "Clique para inverter a ordem.")
    } else {
        ("▼ Decrescente", "Clique para inverter a ordem.")
    };
    row::group(
        "Processos",
        None,
        false,
        vec![
            row::select(
                "Quantidade exibida",
                Some("Quantidade de processos exibidos."),
                count_labels(),
                PROCS_COUNT_OPTIONS.iter().position(|n| *n == settings.procs_count),
                |i| Message::MonitorProcsCount(PROCS_COUNT_OPTIONS[i]),
            ),
            row::select(
                "Ordenar por",
                None,
                sort_labels(),
                MonitorProcSort::ALL.iter().position(|s| *s == settings.proc_sort),
                |i| Message::MonitorProcSort(MonitorProcSort::ALL[i]),
            ),
            row::buttons(
                "Sentido da ordem",
                Some(dir_hint),
                vec![row::button(
                    dir_label,
                    Message::MonitorProcSort(settings.proc_sort),
                    BtnStyle::Normal,
                )],
            ),
        ],
    )
}

fn weather_group(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let place = &app.weather_place;
    let current_place = if place.detail.is_empty() {
        format!("{} ({:.3}, {:.3})", place.name, place.lat, place.lon)
    } else {
        format!("{} — {} ({:.3}, {:.3})", place.name, place.detail, place.lat, place.lon)
    };
    let weather_poll = app.monitor_settings.component_poll(MonitorPollComponent::Weather);
    let mode_names: Vec<&str> = MonitorCardMode::ALL.iter().map(|m| m.name()).collect();
    let mode_sel = MonitorCardMode::ALL
        .iter()
        .position(|m| *m == app.monitor_settings.weather_mode())
        .unwrap_or(0);

    let search = widget::row::with_children(vec![
        widget::text_input("CEP ou nome da cidade", app.weather_query.as_str())
            .on_input(Message::WeatherQueryInput)
            .on_submit(|_| Message::WeatherSearch)
            .width(Length::Fill)
            .into(),
        row::button("Buscar", Message::WeatherSearch, BtnStyle::Primary),
    ])
    .spacing(spacing.space_xxs)
    .align_y(Alignment::Center);

    let mut rows: Vec<Element<'_, Message>> = vec![
        row::kv("Local", current_place),
        row::buttons(
            "Buscar local",
            Some("CEP (ex.: 30330-330) ou nome da cidade."),
            vec![widget::container(search).width(Length::Fill).max_width(320.0).into()],
        ),
    ];
    if app.weather_searching {
        rows.push(row::note("Procurando localidade..."));
    } else if let Some(results) = &app.weather_results {
        if results.is_empty() {
            rows.push(row::note("Nenhum local encontrado para essa busca."));
        } else {
            for candidate in results {
                rows.push(row::list_item(
                    candidate.name.as_str(),
                    candidate.detail.clone(),
                    None,
                    Some(row::button(
                        "Usar",
                        Message::WeatherPickPlace(candidate.clone()),
                        BtnStyle::Normal,
                    )),
                ));
            }
        }
    }
    rows.push(row::segmented(
        "Cartão de clima",
        None,
        &mode_names,
        mode_sel,
        |i| Message::MonitorWeatherMode(MonitorCardMode::ALL[i]),
    ));
    rows.push(row::select(
        "Atualização do clima",
        None,
        weather_labels(),
        WEATHER_MINS_OPTIONS.iter().position(|m| *m == weather_poll),
        |i| {
            Message::MonitorComponentPoll(MonitorPollComponent::Weather, WEATHER_MINS_OPTIONS[i])
        },
    ));
    row::group("Clima", None, false, rows)
}

pub(super) fn monitor_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let power = row::group(
        "Consumo elétrico",
        None,
        false,
        vec![
            row::input(
                "Tarifa (R$ por kWh)",
                None,
                "0,95",
                &app.tariff_input,
                Message::MonitorTariffInput,
                None,
            ),
            row::input(
                "Resto do PC (W)",
                Some("Placa-mãe, discos e ventoinhas, somados à CPU e GPU medidas."),
                "40",
                &app.rest_watts_input,
                Message::MonitorRestWattsInput,
                None,
            ),
        ],
    );
    widget::column::with_children(vec![
        bar_metrics_group(app),
        cards_group(app),
        processes_group(app),
        weather_group(app),
        power,
    ])
    .spacing(spacing.space_m)
    .into()
}

/// Botões do cabeçalho da página.
pub(super) fn monitor_actions(app: &ControlPanel) -> Vec<Element<'_, Message>> {
    vec![action_button("Restaurar padrão", Message::MonitorReset, BtnStyle::Normal, !app.busy)]
}

/// Selo ao lado do título: sem selo por enquanto.
pub(super) fn monitor_badge(_app: &ControlPanel) -> Option<(String, Tone)> {
    None
}
