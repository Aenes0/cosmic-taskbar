// SPDX-License-Identifier: GPL-3.0-only

//! Câmeras & Áudio: chaves de privacidade, câmeras detectadas e celular como webcam (IriunCam).

use cosmic::{Element, widget};

use super::{
    row::{self, BtnStyle, Tone},
    shell::action_button,
};
use crate::central::services::{CameraState, PcWindow};
use crate::control_panel::{ControlPanel, Message};

pub(super) fn camera_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = cosmic::theme::spacing();
    let cam = &app.camera;
    let mut items: Vec<Element<'_, Message>> = Vec::new();

    let mut priv_rows: Vec<Element<'_, Message>> = Vec::new();
    match cam.hw_switch {
        Some(hw_on) => priv_rows.push(row::status(
            "Chave física da câmera",
            Some("Chave de privacidade do hardware (Lenovo)."),
            if hw_on { "Ligada" } else { "Desligada" },
            if hw_on { Tone::Ok } else { Tone::Warn },
            vec![],
        )),
        None => priv_rows.push(row::status(
            "Chave física da câmera",
            Some("Nenhuma chave física detectada nesta máquina."),
            "Ausente",
            Tone::Neutral,
            vec![],
        )),
    }
    if let Some(muted) = app.microphone {
        priv_rows.push(row::toggle(
            "Microfone interno",
            Some("Sincronizado com a tecla física de mudo."),
            !muted,
            |_| Message::ToggleMicrophone,
        ));
    }
    items.push(row::group("Privacidade", None, false, priv_rows));

    let cam_rows: Vec<Element<'_, Message>> = if cam.devices.is_empty() {
        vec![row::note("Nenhuma câmera V4L2 detectada no momento.")]
    } else {
        let using = (!app.privacy.camera.is_empty()).then(|| app.privacy.camera.join(", "));
        cam.devices
            .iter()
            .map(|dev| {
                let meta = match &using {
                    Some(u) => format!("{} · Em uso: {u}", dev.path),
                    None => dev.path.to_string(),
                };
                let (pill, tone) = if dev.is_virtual {
                    ("Virtual", Tone::Info)
                } else {
                    ("Hardware", Tone::Neutral)
                };
                row::list_item(dev.name.as_str(), meta, Some((pill, tone)), None)
            })
            .collect()
    };
    items.push(row::group("Câmeras detectadas", None, false, cam_rows));

    let iriun_rows: Vec<Element<'_, Message>> = if cam.iriun_installed {
        let state = cam.status.as_ref().and_then(|s| s.state);
        let (pill, tone) = match state {
            Some(CameraState::Starting) => ("Iniciando...", Tone::Warn),
            Some(CameraState::WaitingPhone) => ("Aguardando celular", Tone::Warn),
            Some(CameraState::Active) => ("Transmitindo", Tone::Ok),
            Some(CameraState::Error) => ("Erro", Tone::Bad),
            Some(CameraState::Stopped) => ("Parado", Tone::Neutral),
            None => ("Desligado", Tone::Neutral),
        };
        let mut btns = vec![row::button(
            if cam.on { "Desligar" } else { "Ligar" },
            Message::ToggleCamera,
            if cam.on { BtnStyle::Normal } else { BtnStyle::Primary },
        )];
        if cam.on {
            btns.push(row::button("Reiniciar", Message::RestartCamera, BtnStyle::Normal));
            if let Some(win) = cam.status.as_ref().and_then(|s| s.pc_window) {
                let (lbl, show) = match win {
                    PcWindow::Hidden => ("Mostrar janela", true),
                    PcWindow::Shown => ("Ocultar janela", false),
                };
                btns.push(row::button(lbl, Message::IriunWindow(show), BtnStyle::Normal));
            }
        }
        vec![row::status("Estado", None, pill, tone, btns)]
    } else {
        vec![
            row::status("Estado", None, "Não instalado", Tone::Neutral, vec![]),
            row::note(
                "Pacote opcional: instale pela aba \"Pacotes & Opcionais\" ou com ./scripts/bootstrap-pc.sh --with-iriun.",
            ),
        ]
    };
    items.push(row::group("IriunCam · celular como webcam", None, false, iriun_rows));

    widget::column::with_children(items).spacing(spacing.space_m).into()
}

/// Botões do cabeçalho da página.
pub(super) fn camera_actions(app: &ControlPanel) -> Vec<Element<'_, Message>> {
    vec![action_button("Reescanear", Message::RefreshCamera, BtnStyle::Normal, !app.busy)]
}

/// Selo ao lado do título: sem selo por enquanto.
pub(super) fn camera_badge(_app: &ControlPanel) -> Option<(String, Tone)> {
    None
}
