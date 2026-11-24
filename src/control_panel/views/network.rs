// SPDX-License-Identifier: GPL-3.0-only

//! Rede & VPN: conexão ativa, Wi-Fi, VPNs e a troca periódica de endereço MAC.

use cosmic::{Element, widget};

use super::{
    row::{self, BtnStyle, Tone},
    shell::action_button,
};
use crate::control_panel::{ControlPanel, Message};

pub(super) fn network_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = cosmic::theme::spacing();
    let mut items: Vec<Element<'_, Message>> = Vec::new();

    let mut net_rows: Vec<Element<'_, Message>> = Vec::new();
    if let Some(d) = &app.network.details {
        let kind = match d.kind.as_str() {
            "802-3-ethernet" => "Cabeada",
            "802-11-wireless" => "Wi-Fi",
            "vpn" | "wireguard" => "VPN",
            _ => "Rede",
        };
        net_rows.push(row::kv("Conexão", format!("{} ({kind} · {})", d.name, d.interface)));
        if let Some(ip4) = &d.ipv4 {
            net_rows.push(row::kv("IPv4", ip4.clone()));
        }
        if let Some(ip6) = &d.ipv6 {
            net_rows.push(row::kv("IPv6", ip6.clone()));
        }
        if let Some(gw) = &d.gateway {
            net_rows.push(row::kv("Gateway", gw.clone()));
        }
        if !d.dns.is_empty() {
            net_rows.push(row::kv("DNS", d.dns.join(", ")));
        }
        if let Some(mac) = &d.mac {
            net_rows.push(row::kv("Endereço MAC", mac.clone()));
        }
        if let Some(sec) = &d.security {
            net_rows.push(row::kv("Segurança", sec.clone()));
        }
    } else {
        net_rows.push(row::note("Desconectado"));
    }
    items.push(row::group("Conexão ativa", None, false, net_rows));

    if app.network.has_wifi {
        items.push(row::group(
            "Wi-Fi",
            None,
            false,
            vec![row::toggle(
                "Wi-Fi",
                Some("Liga ou desliga o rádio sem fio."),
                app.network.wifi_enabled,
                |_| Message::ToggleWifi,
            )],
        ));
    }

    if !app.network.vpns.is_empty() {
        let rows = app
            .network
            .vpns
            .iter()
            .enumerate()
            .map(|(i, vpn)| {
                let on = vpn.active.is_some();
                let (pill, tone) = if on {
                    ("Conectada", Tone::Ok)
                } else {
                    ("Desconectada", Tone::Neutral)
                };
                row::list_item(
                    vpn.name.as_str(),
                    "VPN".to_owned(),
                    Some((pill, tone)),
                    Some(row::button(
                        if on { "Desconectar" } else { "Conectar" },
                        Message::ConnectVpn(i),
                        if on { BtnStyle::Normal } else { BtnStyle::Primary },
                    )),
                )
            })
            .collect();
        items.push(row::group("VPNs", None, false, rows));
    }

    let mac_on = app.snapshot.mac_rotativo_active;
    let installed = app.snapshot.optional.mac_rotativo_installed;
    let (pill, tone) = if mac_on {
        ("Automático", Tone::Ok)
    } else if installed {
        ("Desativado", Tone::Neutral)
    } else {
        ("Não instalado", Tone::Neutral)
    };
    let (toggle_label, toggle_cmd) = if mac_on {
        ("Desativar", "systemctl disable --now mac-rotativo.timer")
    } else {
        ("Ativar", "systemctl enable --now mac-rotativo.timer")
    };
    items.push(row::group(
        "Privacidade",
        None,
        false,
        vec![row::status(
            "Trocar endereço MAC periodicamente",
            Some("Opcional, útil em Wi-Fi público. Muda o identificador da placa de rede e pode piscar a conexão ao trocar."),
            pill,
            tone,
            vec![
                row::button(
                    "Trocar agora",
                    Message::RunAction(
                        "Rotacionar MAC",
                        "systemctl start mac-rotativo.service && echo 'Endereço MAC trocado.'".to_owned(),
                        true,
                    ),
                    BtnStyle::Normal,
                ),
                row::button(
                    toggle_label,
                    Message::RunAction("MAC Rotativo", toggle_cmd.to_owned(), true),
                    BtnStyle::Normal,
                ),
            ],
        )],
    ));

    widget::column::with_children(items).spacing(spacing.space_m).into()
}

/// Botões do cabeçalho da página.
pub(super) fn network_actions(app: &ControlPanel) -> Vec<Element<'_, Message>> {
    vec![action_button(
        "Abrir no COSMIC",
        Message::OpenCosmicSettings(Some("network")),
        BtnStyle::Normal,
        !app.busy,
    )]
}

/// Selo ao lado do título: sem selo por enquanto.
pub(super) fn network_badge(_app: &ControlPanel) -> Option<(String, Tone)> {
    None
}
