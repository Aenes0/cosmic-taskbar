use crate::sysinfo::{NetworkInfo, NetworkKind};
use cosmic::iced::Subscription;
use cosmic::iced::futures::channel::mpsc;
use cosmic::iced::futures::{SinkExt, StreamExt};
use cosmic_dbus_networkmanager::device::SpecificDevice;
use cosmic_dbus_networkmanager::nm::NetworkManager;
use std::any::TypeId;
use std::time::Duration;
use zbus::{Connection, Result};

/// Rede atual: `None` = sem conexão ativa. Só reenvia quando muda. Se o NetworkManager não
/// responder, nada é enviado (a barra segue sem ícone de rede, como antes).
pub fn subscription() -> Subscription<Option<NetworkInfo>> {
    struct NetworkSubscription;

    Subscription::run_with(TypeId::of::<NetworkSubscription>(), |_| {
        cosmic::iced::stream::channel(4, |mut msg_tx| async move {
            if let Err(err) = handler(&mut msg_tx).await {
                tracing::warn!("networkmanager error: {}", err);
            }

            // Sem retry: o erro só acontece ao conectar (serviço ausente).
            futures_util::future::pending().await
        })
    })
}

/// Junta as leituras das conexões ativas: Wi-Fi vence cabo (o de maior sinal), como antes.
pub(crate) fn pick_network(
    wired: Option<String>,
    wireless: Vec<(u8, String)>,
) -> Option<NetworkInfo> {
    if let Some((strength, name)) = wireless.into_iter().max_by_key(|(strength, _)| *strength) {
        return Some(NetworkInfo { kind: NetworkKind::Wireless { strength }, name });
    }
    wired.map(|name| NetworkInfo { kind: NetworkKind::Wired, name })
}

//TODO: use never type?
pub async fn handler(msg_tx: &mut mpsc::Sender<Option<NetworkInfo>>) -> Result<()> {
    let zbus = Connection::system().await?;
    let nm = NetworkManager::new(&zbus).await?;
    let mut active_conns_changed = nm.receive_active_connections_changed().await;
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    let mut last: Option<Option<NetworkInfo>> = None;

    loop {
        let mut wired: Option<String> = None;
        let mut wireless: Vec<(u8, String)> = Vec::new();

        for conn in nm.active_connections().await.unwrap_or_default() {
            let conn_id = conn.id().await.unwrap_or_default();
            for dev in conn.devices().await.unwrap_or_default() {
                match dev.downcast_to_device().await.unwrap_or_default() {
                    //TODO: more specific devices
                    Some(SpecificDevice::Wired(_)) => {
                        wired.get_or_insert_with(|| conn_id.clone());
                    }
                    Some(SpecificDevice::Wireless(wifi)) => {
                        if let Ok(ap) = wifi.active_access_point().await
                            && let Ok(strength) = ap.strength().await
                        {
                            let ssid = ap.ssid().await.unwrap_or_default();
                            let ssid = String::from_utf8_lossy(&ssid).trim().to_string();
                            let name = if ssid.is_empty() { conn_id.clone() } else { ssid };
                            wireless.push((strength, name));
                        }
                    }
                    _ => {}
                }
            }
        }

        let info = pick_network(wired, wireless);
        if last.as_ref() != Some(&info) {
            if msg_tx.send(info.clone()).await.is_err() {
                return Ok(()); // quem escutava foi embora
            }
            last = Some(info);
        }

        // Waits until active connections have changed and at least one second has passed
        if active_conns_changed.next().await.is_none() {
            // O fluxo terminou: seguir giraria sem parar. Sai sem apagar o último valor.
            tracing::warn!("networkmanager: fluxo de conexões ativas encerrou");
            return Ok(());
        }
        interval.tick().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wifi_wins_over_cable_with_highest_signal() {
        let info = pick_network(Some("Cabo".into()), vec![(40, "Casa".into()), (80, "Mesh".into())]);
        assert_eq!(
            info,
            Some(NetworkInfo { kind: NetworkKind::Wireless { strength: 80 }, name: "Mesh".into() })
        );
    }

    #[test]
    fn cable_alone_uses_connection_id_and_none_when_offline() {
        assert_eq!(
            pick_network(Some("Ethernet 1".into()), vec![]),
            Some(NetworkInfo { kind: NetworkKind::Wired, name: "Ethernet 1".into() })
        );
        assert_eq!(pick_network(None, vec![]), None);
    }
}
