// SPDX-License-Identifier: GPL-3.0-only

//! Dados do sistema mostrados pelos estilos (baterias e rede): os tipos, a ordenação estável e
//! os nomes de ícone. Quem preenche `SystemInfo` são `upower.rs` e `networkmanager.rs`.


#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DeviceKind {
    System,
    Mouse,
    Keyboard,
    Headset,
    Gamepad,
    Phone,
    Tablet,
    Other,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BatteryDevice {
    pub kind: DeviceKind,
    pub name: String,
    pub percent: u8,
    pub charging: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkKind {
    Wired,
    Wireless { strength: u8 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NetworkInfo {
    pub kind: NetworkKind,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SystemInfo {
    pub batteries: Vec<BatteryDevice>,
    pub network: Option<NetworkInfo>,
}

impl DeviceKind {
    /// Ícone simbólico do dispositivo. A bateria do sistema (e `Other`) usa o nível; periféricos
    /// usam o ícone do tipo de aparelho.
    pub fn icon_name(self, percent: u8, charging: bool) -> &'static str {
        match self {
            DeviceKind::Mouse => "input-mouse-symbolic",
            DeviceKind::Keyboard => "input-keyboard-symbolic",
            DeviceKind::Headset => "audio-headset-symbolic",
            DeviceKind::Gamepad => "input-gaming-symbolic",
            DeviceKind::Phone => "phone-symbolic",
            DeviceKind::Tablet => "input-tablet-symbolic",
            DeviceKind::System | DeviceKind::Other => battery_level_icon(percent, charging),
        }
    }

    /// Chave Fluent do rótulo (`battery-*` em `i18n/*/cosmic_greeter.ftl`).
    pub fn label_key(self) -> &'static str {
        match self {
            DeviceKind::System => "battery-system",
            DeviceKind::Mouse => "battery-mouse",
            DeviceKind::Keyboard => "battery-keyboard",
            DeviceKind::Headset => "battery-headset",
            DeviceKind::Gamepad => "battery-gamepad",
            DeviceKind::Phone | DeviceKind::Tablet | DeviceKind::Other => "battery-device",
        }
    }
}

/// `battery-level-N[-charging]-symbolic` com N em passos de 10 (arredondado para baixo).
fn battery_level_icon(percent: u8, charging: bool) -> &'static str {
    const LEVELS: [(&str, &str); 11] = [
        ("battery-level-0-symbolic", "battery-level-0-charging-symbolic"),
        ("battery-level-10-symbolic", "battery-level-10-charging-symbolic"),
        ("battery-level-20-symbolic", "battery-level-20-charging-symbolic"),
        ("battery-level-30-symbolic", "battery-level-30-charging-symbolic"),
        ("battery-level-40-symbolic", "battery-level-40-charging-symbolic"),
        ("battery-level-50-symbolic", "battery-level-50-charging-symbolic"),
        ("battery-level-60-symbolic", "battery-level-60-charging-symbolic"),
        ("battery-level-70-symbolic", "battery-level-70-charging-symbolic"),
        ("battery-level-80-symbolic", "battery-level-80-charging-symbolic"),
        ("battery-level-90-symbolic", "battery-level-90-charging-symbolic"),
        ("battery-level-100-symbolic", "battery-level-100-charged-symbolic"),
    ];
    let (idle, plugged) = LEVELS[usize::from(percent.min(100) / 10)];
    if charging { plugged } else { idle }
}

impl NetworkInfo {
    /// Mesmos nomes que o ícone de rede usava antes (`NetworkIcon::name`).
    pub fn icon_name(&self) -> &'static str {
        match self.kind {
            NetworkKind::Wired => "network-wired-symbolic",
            NetworkKind::Wireless { strength } => {
                if strength < 25 {
                    "network-wireless-signal-weak-symbolic"
                } else if strength < 50 {
                    "network-wireless-signal-ok-symbolic"
                } else if strength < 75 {
                    "network-wireless-signal-good-symbolic"
                } else {
                    "network-wireless-signal-excellent-symbolic"
                }
            }
        }
    }
}

/// Ícone da barra para a rede atual; sem conexão ativa, o de "desconectado".
pub fn network_icon_name(info: Option<&NetworkInfo>) -> &'static str {
    match info {
        Some(info) => info.icon_name(),
        None => "network-wired-disconnected-symbolic",
    }
}

/// Ordem estável por (tipo, nome); o percentual só desempata para a lista ser determinística
/// (a assinatura compara listas para reenviar apenas quando mudam).
pub fn sorted_batteries(mut v: Vec<BatteryDevice>) -> Vec<BatteryDevice> {
    v.sort_by(|a, b| {
        (a.kind, &a.name, a.percent, a.charging).cmp(&(b.kind, &b.name, b.percent, b.charging))
    });
    v
}

impl SystemInfo {
    /// Bateria do sistema, se houver; senão o periférico de menor percentual.
    pub fn glance_battery(&self) -> Option<&BatteryDevice> {
        self.batteries
            .iter()
            .find(|b| b.kind == DeviceKind::System)
            .or_else(|| self.peripherals().min_by_key(|b| b.percent))
    }

    /// Periféricos (tudo que não é a bateria do sistema), em ordem estável por (tipo, nome).
    pub fn peripherals(&self) -> impl Iterator<Item = &BatteryDevice> {
        let mut list: Vec<&BatteryDevice> =
            self.batteries.iter().filter(|b| b.kind != DeviceKind::System).collect();
        list.sort_by(|a, b| (a.kind, &a.name, a.percent).cmp(&(b.kind, &b.name, b.percent)));
        list.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(kind: DeviceKind, name: &str, percent: u8) -> BatteryDevice {
        BatteryDevice { kind, name: name.to_string(), percent, charging: false }
    }

    #[test]
    fn glance_prefers_system() {
        let info = SystemInfo {
            batteries: vec![dev(DeviceKind::Mouse, "MX", 10), dev(DeviceKind::System, "BAT0", 77)],
            network: None,
        };
        assert_eq!(info.glance_battery().map(|b| b.kind), Some(DeviceKind::System));
    }

    #[test]
    fn glance_falls_back_to_lowest_peripheral() {
        let info = SystemInfo {
            batteries: vec![dev(DeviceKind::Mouse, "MX", 80), dev(DeviceKind::Keyboard, "K3", 30)],
            network: None,
        };
        assert_eq!(info.glance_battery().map(|b| b.kind), Some(DeviceKind::Keyboard));
        assert!(SystemInfo::default().glance_battery().is_none());
    }

    #[test]
    fn peripherals_skip_system_and_are_stable() {
        let info = SystemInfo {
            batteries: vec![
                dev(DeviceKind::Keyboard, "B", 50),
                dev(DeviceKind::System, "BAT0", 90),
                dev(DeviceKind::Mouse, "Z", 20),
                dev(DeviceKind::Keyboard, "A", 60),
                dev(DeviceKind::Mouse, "Y", 40),
            ],
            network: None,
        };
        let order: Vec<(DeviceKind, &str)> =
            info.peripherals().map(|b| (b.kind, b.name.as_str())).collect();
        assert_eq!(
            order,
            vec![
                (DeviceKind::Mouse, "Y"),
                (DeviceKind::Mouse, "Z"),
                (DeviceKind::Keyboard, "A"),
                (DeviceKind::Keyboard, "B"),
            ]
        );
    }

    #[test]
    fn sorted_batteries_is_deterministic_regardless_of_input_order() {
        let a = vec![dev(DeviceKind::Mouse, "M", 5), dev(DeviceKind::System, "BAT0", 5), dev(DeviceKind::Mouse, "M", 3)];
        let mut b = a.clone();
        b.reverse();
        assert_eq!(sorted_batteries(a), sorted_batteries(b));
    }

    #[test]
    fn network_icon_names_match_legacy() {
        let wired = NetworkInfo { kind: NetworkKind::Wired, name: "Cabo".into() };
        assert_eq!(wired.icon_name(), "network-wired-symbolic");
        let wifi = |strength| NetworkInfo { kind: NetworkKind::Wireless { strength }, name: "x".into() };
        assert_eq!(wifi(80).icon_name(), "network-wireless-signal-excellent-symbolic");
        assert_eq!(wifi(60).icon_name(), "network-wireless-signal-good-symbolic");
        assert_eq!(wifi(30).icon_name(), "network-wireless-signal-ok-symbolic");
        assert_eq!(wifi(10).icon_name(), "network-wireless-signal-weak-symbolic");
        assert_eq!(network_icon_name(None), "network-wired-disconnected-symbolic");
        assert_eq!(network_icon_name(Some(&wired)), "network-wired-symbolic");
    }

    #[test]
    fn device_kind_icons_and_labels() {
        assert_eq!(DeviceKind::Mouse.label_key(), "battery-mouse");
        assert_eq!(DeviceKind::System.label_key(), "battery-system");
        assert_eq!(DeviceKind::Gamepad.label_key(), "battery-gamepad");
        assert_eq!(DeviceKind::Phone.label_key(), "battery-device");
        assert_eq!(DeviceKind::System.icon_name(100, false), "battery-level-100-symbolic");
        assert_eq!(DeviceKind::System.icon_name(55, true), "battery-level-50-charging-symbolic");
        assert_eq!(DeviceKind::System.icon_name(3, false), "battery-level-0-symbolic");
        assert_eq!(DeviceKind::Mouse.icon_name(40, false), "input-mouse-symbolic");
    }
}
