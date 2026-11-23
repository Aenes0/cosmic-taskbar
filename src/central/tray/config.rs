// Copyright 2023 System76 <info@system76.com>, 2026 Aenes0
// SPDX-License-Identifier: GPL-3.0-only

use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use serde::{Deserialize, Serialize};

/// The tray's own applet id, as when it was its own program: the config
/// (the `visible` list) stays where it was.
pub const CONFIG_ID: &str = super::APP_ID;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, CosmicConfigEntry)]
#[version = 1]
pub struct TrayConfig {
    /// Preferred order of background apps on the bar (and legacy visible list).
    pub visible: Vec<String>,
    /// Background apps explicitly hidden from the bar (new apps show on the bar by default).
    pub hidden: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic::cosmic_config::Config;

    #[test]
    fn config_round_trip() {
        let dir = std::env::temp_dir().join(format!("tray-cfg-{}", std::process::id()));
        let cfg = Config::with_custom_path(CONFIG_ID, TrayConfig::VERSION, dir.clone()).unwrap();
        let missing = TrayConfig::get_entry(&cfg).unwrap_or_else(|(_, d)| d);
        assert!(missing.visible.is_empty());
        assert!(missing.hidden.is_empty());
        let c = TrayConfig {
            visible: vec!["Σ".into(), "keyboard".into()],
            hidden: vec!["slack".into()],
        };
        c.write_entry(&cfg).unwrap();
        assert_eq!(TrayConfig::get_entry(&cfg).unwrap(), c);
        let _ = std::fs::remove_dir_all(dir);
    }
}
