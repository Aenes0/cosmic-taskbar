// SPDX-License-Identifier: GPL-3.0-only

//! Monitores & Janelas: um cartão por monitor, folgas e foco das janelas, utilitários visuais.
//! O recorte de tela mora em `capture.rs`.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use cosmic::{Element, widget};

use super::{
    row::{self, BtnStyle, Tone},
    shell::action_button,
};
use crate::control_panel::{ControlPanel, Message};

/// Pares (margem externa, espaço interno) do seletor "Espaçamento".
const GAP_PRESETS: [(u32, u32); 4] = [(0, 0), (0, 4), (4, 8), (8, 12)];
const GAP_PRESET_LABELS: [&str; 4] = ["0 px", "4 px", "8 px", "12 px"];
const PX_OPTIONS: [u32; 4] = [0, 4, 8, 12];
/// (id gravado, rótulo) da prioridade de foco.
const FOCUS_POLICIES: [(&str, &str); 3] = [
    ("Urgent", "Não roubar foco (Recomendado)"),
    ("FocusIfActiveWorkspace", "Só na área atual"),
    ("Focus", "Sempre focar"),
];

fn px_labels() -> &'static [String] {
    static L: OnceLock<Vec<String>> = OnceLock::new();
    L.get_or_init(|| PX_OPTIONS.iter().map(|p| format!("{p} px")).collect())
}

fn focus_labels() -> &'static [String] {
    static L: OnceLock<Vec<String>> = OnceLock::new();
    L.get_or_init(|| FOCUS_POLICIES.iter().map(|(_, l)| (*l).to_owned()).collect())
}

/// Título de cartão que precisa de `&str` com a vida da tela: guarda cada texto distinto uma
/// única vez (o conjunto é limitado pelos monitores já vistos nesta sessão).
fn intern(s: String) -> &'static str {
    static SET: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let mut set = SET
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some(found) = set.get(s.as_str()) {
        return found;
    }
    let leaked: &'static str = Box::leak(s.into_boxed_str());
    set.insert(leaked);
    leaked
}

pub(super) fn displays_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = cosmic::theme::spacing();
    let mut items: Vec<Element<'_, Message>> = Vec::new();

    match &app.displays {
        Some(displays) => {
            for (idx, d) in displays.iter().enumerate() {
                let title = if d.internal() {
                    "Tela interna".to_owned()
                } else {
                    format!("{} {} ({})", d.make, d.model, d.name).trim().to_owned()
                };
                let is_primary = app.primary_display.as_deref() == Some(d.name.as_str());
                let mut rows: Vec<Element<'_, Message>> = Vec::new();
                if is_primary {
                    rows.push(row::status(
                        "Monitor principal",
                        Some("Recebe a barra e as janelas novas."),
                        "Principal",
                        Tone::Info,
                        vec![],
                    ));
                } else if d.enabled && d.ddc.is_some() {
                    rows.push(row::buttons(
                        "Monitor principal",
                        None,
                        vec![row::button(
                            "Tornar principal",
                            Message::SetPrimaryDisplay(d.name.clone()),
                            BtnStyle::Normal,
                        )],
                    ));
                } else {
                    rows.push(row::status("Monitor principal", None, "Secundário", Tone::Neutral, vec![]));
                }
                rows.push(row::kv(
                    "Modo",
                    format!(
                        "{} × {} · {:.0} Hz · Escala {:.0}%",
                        d.width,
                        d.height,
                        d.refresh as f32 / 1000.0,
                        d.scale * 100.0
                    ),
                ));
                if d.internal()
                    && let Some((cur, max)) = app.brightness
                {
                    let pct = (cur as f32 / max.max(1) as f32 * 100.0).clamp(1.0, 100.0);
                    rows.push(row::slider(
                        "Brilho",
                        None,
                        1.0..=100.0,
                        1.0,
                        pct,
                        format!("{pct:.0}%"),
                        ("1%".to_owned(), "100%".to_owned()),
                        Message::SetInternalBrightness,
                        None,
                    ));
                } else if let Some((_, cur, max)) = d.ddc {
                    let pct = (cur as f32 / max.max(1) as f32 * 100.0).clamp(0.0, 100.0);
                    rows.push(row::slider(
                        "Brilho (DDC)",
                        None,
                        0.0..=100.0,
                        1.0,
                        pct,
                        format!("{pct:.0}%"),
                        ("0%".to_owned(), "100%".to_owned()),
                        move |v| Message::SetDdcBrightness(idx, v),
                        Some(Message::ApplyDdcBrightness(idx)),
                    ));
                }
                items.push(row::group(intern(title), None, false, rows));
            }
        }
        None => items.push(row::group(
            "Monitores",
            None,
            false,
            vec![row::note("Lendo monitores...")],
        )),
    }

    let (gap_outer, gap_inner) = app.window_gaps;
    let preset = GAP_PRESETS
        .iter()
        .position(|&(o, i)| o == gap_outer && i == gap_inner)
        .unwrap_or(usize::MAX);
    let px_index = |v: u32| PX_OPTIONS.iter().position(|&p| p == v);
    let focus_index = FOCUS_POLICIES
        .iter()
        .position(|(id, _)| *id == app.activation_policy);
    let policy_desc = match app.activation_policy.as_str() {
        "Urgent" => "Recomendado: comandos automáticos e apps em segundo plano só acendem na barra.",
        "FocusIfActiveWorkspace" => "Traz a janela para frente apenas se ela já estiver na sua área de trabalho atual.",
        _ => "Padrão COSMIC: qualquer app ou script que abrir janela pode roubar o foco na hora.",
    };
    items.push(row::group(
        "Janelas",
        None,
        false,
        vec![
            row::segmented(
                "Espaçamento",
                Some("Sem folga, compacto, padrão COSMIC ou folgado."),
                &GAP_PRESET_LABELS,
                preset,
                |i| {
                    let (o, n) = GAP_PRESETS[i];
                    Message::SetWindowGaps(o, n)
                },
            ),
            row::select(
                "Margem externa da tela",
                None,
                px_labels(),
                px_index(gap_outer),
                move |i| Message::SetWindowGaps(PX_OPTIONS[i], gap_inner),
            ),
            row::select(
                "Espaço entre janelas divididas",
                None,
                px_labels(),
                px_index(gap_inner),
                move |i| Message::SetWindowGaps(gap_outer, PX_OPTIONS[i]),
            ),
            row::select(
                "Prioridade de foco",
                Some(policy_desc),
                focus_labels(),
                focus_index,
                |i| Message::SetActivationPolicy(FOCUS_POLICIES[i].0),
            ),
        ],
    ));

    let (wp_text, wp_tone) = if app.snapshot.wallpapers_watcher_active {
        ("Vigiando", Tone::Ok)
    } else {
        ("Inativo", Tone::Neutral)
    };
    let (pwa_text, pwa_tone) = if app.snapshot.pwa_watcher_active {
        ("Vigiando", Tone::Ok)
    } else {
        ("Inativo", Tone::Neutral)
    };
    items.push(row::group(
        "Utilitários visuais",
        None,
        false,
        vec![
            row::status(
                "Wallpapers por monitor",
                Some("Slideshow com recorte por monitor."),
                wp_text,
                wp_tone,
                vec![row::button("Reaplicar agora", Message::RunWallpapers, BtnStyle::Normal)],
            ),
            row::status(
                "Ícones PWA do Brave",
                Some("Corrige ícones de apps instalados pelo Brave."),
                pwa_text,
                pwa_tone,
                vec![row::button("Corrigir agora", Message::RunBravePwaFix, BtnStyle::Normal)],
            ),
        ],
    ));

    widget::column::with_children(items).spacing(spacing.space_m).into()
}

/// Botões do cabeçalho da página.
pub(super) fn displays_actions(app: &ControlPanel) -> Vec<Element<'_, Message>> {
    vec![action_button("Ler monitores", Message::RefreshDisplays, BtnStyle::Normal, !app.busy)]
}

/// Selo ao lado do título: sem selo por enquanto.
pub(super) fn displays_badge(_app: &ControlPanel) -> Option<(String, Tone)> {
    None
}
