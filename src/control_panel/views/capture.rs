// SPDX-License-Identifier: GPL-3.0-only

//! Captura de tela (Ferramentas): ajustes do recorte (Super+Shift+S).

use std::sync::OnceLock;

use cosmic::{Element, iced::Color, widget};

use super::{
    row::{self, BtnStyle, Tone},
    shell::action_button,
};
use crate::control_panel::{
    ControlPanel, Message,
    screenshot_config::{self, OpenMode},
};

const MODE_LABELS: [&str; 4] = ["Notificar", "Editar no Satty", "Prévia", "Silencioso"];
/// (segundos gravados, rótulo) do tempo das ações da notificação.
const TIMEOUTS: [(u32, &str); 4] = [(60, "1 min"), (180, "3 min"), (600, "10 min"), (1800, "30 min")];

fn timeout_labels() -> &'static [String] {
    static L: OnceLock<Vec<String>> = OnceLock::new();
    L.get_or_init(|| TIMEOUTS.iter().map(|(_, l)| (*l).to_owned()).collect())
}

fn tool_labels() -> &'static [String] {
    static L: OnceLock<Vec<String>> = OnceLock::new();
    L.get_or_init(|| {
        screenshot_config::SATTY_TOOLS
            .iter()
            .map(|(_, l)| (*l).to_owned())
            .collect()
    })
}

/// `#RRGGBB` → cor da amostra; cinza se o texto não for um hex válido.
fn hex_color(hex: &str) -> Color {
    let h = hex.trim_start_matches('#');
    let parse = |r: std::ops::Range<usize>| h.get(r).and_then(|s| u8::from_str_radix(s, 16).ok());
    match (parse(0..2), parse(2..4), parse(4..6)) {
        (Some(r), Some(g), Some(b)) => Color::from_rgb8(r, g, b),
        _ => Color::from_rgb8(128, 128, 128),
    }
}

pub(super) fn capture_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = cosmic::theme::spacing();
    let sc = &app.screenshot_config;

    let mode = OpenMode::ALL
        .iter()
        .position(|m| *m == sc.open_mode)
        .unwrap_or(0);
    let timeout = TIMEOUTS.iter().position(|(s, _)| *s == sc.notify_timeout_secs);
    let tool = screenshot_config::SATTY_TOOLS
        .iter()
        .position(|(id, _)| *id == sc.satty_tool);
    let colors: Vec<(&str, Color)> = screenshot_config::BORDER_COLOR_PRESETS
        .iter()
        .map(|&(hex, name)| (name, hex_color(hex)))
        .collect();
    let color_sel = screenshot_config::BORDER_COLOR_PRESETS
        .iter()
        .position(|&(hex, _)| sc.border_color.eq_ignore_ascii_case(hex));

    let after = row::group(
        "Depois de capturar",
        None,
        false,
        vec![
            row::segmented("Ação", None, &MODE_LABELS, mode, |i| {
                Message::ScreenshotOpenMode(OpenMode::ALL[i])
            }),
            row::toggle(
                "Copiar para a área de transferência",
                None,
                sc.copy_clipboard,
                |_| Message::ScreenshotToggleCopy,
            ),
            row::toggle("Tocar som", None, sc.play_sound, |_| Message::ScreenshotToggleSound),
            row::select(
                "Tempo das ações da notificação",
                None,
                timeout_labels(),
                timeout,
                |i| Message::ScreenshotTimeoutSecs(TIMEOUTS[i].0),
            ),
            row::input(
                "Pasta de destino",
                Some("Vazio = ~/Imagens/Screenshots."),
                "Ex: ~/Imagens/Screenshots",
                sc.save_dir.as_str(),
                Message::ScreenshotSaveDirInput,
                None,
            ),
        ],
    );

    let selection = row::group(
        "Seleção",
        None,
        false,
        vec![
            row::swatches("Cor da borda", None, &colors, color_sel, |i| {
                Message::ScreenshotBorderColor(screenshot_config::BORDER_COLOR_PRESETS[i].0.to_owned())
            }),
            row::slider(
                "Espessura da borda",
                None,
                1.0..=6.0,
                1.0,
                sc.border_width as f32,
                format!("{} px", sc.border_width),
                ("1 px".to_owned(), "6 px".to_owned()),
                |v| Message::ScreenshotBorderWidth(v.round() as u32),
                None,
            ),
            row::slider(
                "Escurecimento do fundo",
                None,
                0.0..=85.0,
                5.0,
                sc.overlay_opacity as f32,
                format!("{}%", sc.overlay_opacity),
                ("0%".to_owned(), "85%".to_owned()),
                |v| Message::ScreenshotOverlayOpacity(v.round() as u32),
                None,
            ),
        ],
    );

    let satty = row::group(
        "Editor Satty",
        None,
        false,
        vec![row::select(
            "Ferramenta inicial",
            None,
            tool_labels(),
            tool,
            |i| Message::ScreenshotSattyTool(screenshot_config::SATTY_TOOLS[i].0.to_owned()),
        )],
    );

    widget::column::with_children(vec![after, selection, satty])
        .spacing(spacing.space_m)
        .into()
}

pub(super) fn capture_actions(app: &ControlPanel) -> Vec<Element<'_, Message>> {
    let enabled = !app.busy;
    vec![
        action_button("Restaurar padrão", Message::ScreenshotReset, BtnStyle::Normal, enabled),
        action_button("Abrir pasta", Message::OpenScreenshotsDir, BtnStyle::Normal, enabled),
        action_button("Testar recorte", Message::RunScreenshot, BtnStyle::Primary, enabled),
    ]
}

pub(super) fn capture_badge(_app: &ControlPanel) -> Option<(String, Tone)> {
    None
}
