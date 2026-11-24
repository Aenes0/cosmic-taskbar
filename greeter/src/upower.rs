use cosmic::iced::futures::channel::mpsc;
use cosmic::iced::futures::{SinkExt, StreamExt};
use cosmic::iced::{Subscription, stream};
use futures_util::select;
use std::any::TypeId;
use std::time::Duration;
use upower_dbus::{BatteryState, BatteryType, DeviceProxy, UPowerProxy};
use zbus::{Connection, Result};

use crate::sysinfo::{BatteryDevice, DeviceKind, sorted_batteries};

/// Reconsulta periódica dos periféricos (além de `DeviceAdded`/`DeviceRemoved`).
const DEVICES_REFRESH: Duration = Duration::from_secs(300);
/// Junta rajadas de `DeviceAdded`/`DeviceRemoved` (um fone Bluetooth gera vários) numa consulta.
const DEVICES_DEBOUNCE: Duration = Duration::from_secs(1);

pub fn subscription() -> Subscription<Option<(f64, bool, bool)>> {
    struct PowerSubscription;

    Subscription::run_with(TypeId::of::<PowerSubscription>(), |_| {
        stream::channel(16, |mut msg_tx| async move {
            match handler(&mut msg_tx).await {
                Ok(()) => {}
                Err(err) => {
                    tracing::warn!("upower error: {}", err);
                    //TODO: send error
                }
            }

            // If reading power status failed, clear power icon
            msg_tx.send(None).await.unwrap();

            //TODO: should we retry on error?
            futures_util::future::pending().await
        })
    })
}

//TODO: use never type?
pub async fn handler(msg_tx: &mut mpsc::Sender<Option<(f64, bool, bool)>>) -> Result<()> {
    let zbus = Connection::system().await?;
    let upower = UPowerProxy::new(&zbus).await?;
    let dev = upower.get_display_device().await?;

    let mut percentage_changed = dev.receive_percentage_changed().await.boxed().fuse();
    let mut state_changed = dev.receive_state_changed().await.boxed().fuse();
    let mut charge_threshold_enabled_changed = dev
        .receive_charge_threshold_enabled_changed()
        .await
        .boxed()
        .fuse();
    let mut interval = tokio::time::interval(Duration::from_secs(1));

    let has_battery = dev.type_().await? == BatteryType::Battery && dev.power_supply().await?;
    if !has_battery {
        return Ok(());
    }
    loop {
        let mut info_opt = None;

        if let Ok(mut percent) = dev.percentage().await
            && let Ok(state) = dev.state().await
        {
            let threshold_enabled = dev.charge_threshold_enabled().await.unwrap_or_default();
            let mut capacity = dev.capacity().await.unwrap_or(100.);
            if capacity <= 1. {
                capacity = 100.;
            }

            // compensate for declining battery capacity
            percent = percent * 100. / capacity;
            if matches!(state, BatteryState::FullyCharged) || percent >= 100. {
                percent = 100.;
            }

            info_opt = Some((
                percent,
                state == BatteryState::Discharging,
                threshold_enabled,
            ));
        }

        msg_tx.send(info_opt).await.unwrap();

        // Waits until icon or percentage have changed, and at least one second has passed.
        select! {
            _ = state_changed.next() => {},
            _ = percentage_changed.next() => {},
            _ = charge_threshold_enabled_changed.next() => {}
        }
        interval.tick().await;
    }
}

/// Decide se um dispositivo do UPower entra na lista e com que tipo. Fonte de energia, monitor e
/// UPS nunca entram; a bateria do sistema exige `power_supply && is_present`; periférico exige
/// percentual > 0 (sem leitura, não aparece).
pub fn classify(
    t: BatteryType,
    power_supply: bool,
    is_present: bool,
    percentage: f64,
) -> Option<DeviceKind> {
    let kind = match t {
        BatteryType::LinePower | BatteryType::Unknown | BatteryType::Monitor | BatteryType::Ups => {
            return None;
        }
        BatteryType::Battery => {
            return (power_supply && is_present).then_some(DeviceKind::System);
        }
        BatteryType::Mouse => DeviceKind::Mouse,
        BatteryType::Keyboard => DeviceKind::Keyboard,
        BatteryType::Headset | BatteryType::Headphones => DeviceKind::Headset,
        BatteryType::GamingInput => DeviceKind::Gamepad,
        BatteryType::Phone => DeviceKind::Phone,
        BatteryType::Tablet | BatteryType::Pen => DeviceKind::Tablet,
        _ => DeviceKind::Other,
    };
    (percentage > 0.).then_some(kind)
}

/// Lista de baterias (sistema + periféricos), ordenada. Reenvia só quando a lista muda.
pub fn devices_subscription() -> Subscription<Vec<BatteryDevice>> {
    struct DevicesSubscription;

    Subscription::run_with(TypeId::of::<DevicesSubscription>(), |_| {
        stream::channel(4, |mut msg_tx| async move {
            if let Err(err) = devices_handler(&mut msg_tx).await {
                tracing::warn!("upower devices error: {}", err);
                // Sem dado: lista vazia (nenhum widget de bateria aparece).
                let _ = msg_tx.send(Vec::new()).await;
            }

            // Sem retry: um erro aqui é serviço ausente, não vale laço de tentativas.
            futures_util::future::pending().await
        })
    })
}

async fn devices_handler(msg_tx: &mut mpsc::Sender<Vec<BatteryDevice>>) -> Result<()> {
    let zbus = Connection::system().await?;
    let upower = UPowerProxy::new(&zbus).await?;
    let mut added = upower.receive_device_added().await?;
    let mut removed = upower.receive_device_removed().await?;
    let mut last: Option<Vec<BatteryDevice>> = None;

    loop {
        match read_devices(&zbus, &upower).await {
            Ok(devices) => {
                let list = sorted_batteries(devices);
                if last.as_ref() != Some(&list) {
                    if msg_tx.send(list.clone()).await.is_err() {
                        return Ok(()); // quem escutava foi embora
                    }
                    last = Some(list);
                }
            }
            // Falha passageira: mantém a última lista e tenta no próximo evento ou em 5 min.
            Err(err) => tracing::warn!("upower enumerate_devices falhou: {err}"),
        }

        // Um fluxo que termina deixaria o laço girando: sai e deixa o chamador limpar a lista.
        let ended = tokio::select! {
            ev = added.next() => ev.is_none(),
            ev = removed.next() => ev.is_none(),
            _ = tokio::time::sleep(DEVICES_REFRESH) => false,
        };
        if ended {
            return Err(zbus::Error::Failure("sinal do UPower encerrou".into()));
        }
        tokio::time::sleep(DEVICES_DEBOUNCE).await;
    }
}

async fn read_devices(zbus: &Connection, upower: &UPowerProxy<'_>) -> Result<Vec<BatteryDevice>> {
    let mut out = Vec::new();
    for path in upower.enumerate_devices().await? {
        match read_device(zbus, path.clone()).await {
            Ok(Some(device)) => out.push(device),
            Ok(None) => {}
            // O dispositivo pode ter sumido entre a enumeração e a leitura.
            Err(err) => tracing::debug!("upower device {path} ignorado: {err}"),
        }
    }
    Ok(out)
}

async fn read_device(
    zbus: &Connection,
    path: zbus::zvariant::OwnedObjectPath,
) -> Result<Option<BatteryDevice>> {
    let dev = DeviceProxy::builder(zbus).path(path)?.build().await?;
    let percentage = dev.percentage().await?;
    let kind = classify(
        dev.type_().await?,
        dev.power_supply().await.unwrap_or(false),
        dev.is_present().await.unwrap_or(false),
        percentage,
    );
    let Some(kind) = kind else {
        return Ok(None);
    };
    Ok(Some(BatteryDevice {
        kind,
        name: dev.model().await.unwrap_or_default().trim().to_string(),
        percent: percentage.round().clamp(0., 100.) as u8,
        charging: matches!(dev.state().await, Ok(BatteryState::Charging)),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_filters_line_power_and_empty() {
        assert_eq!(classify(BatteryType::LinePower, true, true, 100.), None);
        assert_eq!(classify(BatteryType::Mouse, false, true, 0.), None);
        assert_eq!(classify(BatteryType::Mouse, false, true, 87.), Some(DeviceKind::Mouse));
        assert_eq!(classify(BatteryType::Battery, false, true, 50.), None);
        assert_eq!(classify(BatteryType::Battery, true, false, 50.), None);
        assert_eq!(classify(BatteryType::Battery, true, true, 50.), Some(DeviceKind::System));
    }

    #[test]
    fn classify_maps_peripheral_families() {
        assert_eq!(classify(BatteryType::Keyboard, false, true, 10.), Some(DeviceKind::Keyboard));
        assert_eq!(classify(BatteryType::Headset, false, true, 10.), Some(DeviceKind::Headset));
        assert_eq!(classify(BatteryType::Headphones, false, true, 10.), Some(DeviceKind::Headset));
        assert_eq!(classify(BatteryType::GamingInput, false, true, 10.), Some(DeviceKind::Gamepad));
        assert_eq!(classify(BatteryType::Phone, false, true, 10.), Some(DeviceKind::Phone));
        assert_eq!(classify(BatteryType::Tablet, false, true, 10.), Some(DeviceKind::Tablet));
        assert_eq!(classify(BatteryType::Pen, false, true, 10.), Some(DeviceKind::Tablet));
        assert_eq!(classify(BatteryType::Touchpad, false, true, 10.), Some(DeviceKind::Other));
        for t in [BatteryType::Unknown, BatteryType::Monitor, BatteryType::Ups] {
            assert_eq!(classify(t, true, true, 90.), None);
        }
    }
}
