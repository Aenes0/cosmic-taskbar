// SPDX-License-Identifier: GPL-3.0-only

//! User-configurable appearance and behavior for the Central's custom
//! notifications panel, persisted via `cosmic_config` under
//! `crate::central::APP_ID` (version 1, key `"notifications_config"`).
//!
//! Position (`anchor`) and default timeout (`max_timeout_normal`) are also
//! synced with COSMIC's notification daemon (`com.system76.CosmicNotifications`
//! v1) so toast popups follow the user's choices.

use cosmic::cosmic_config::{Config, ConfigGet, ConfigSet};
use serde::{Deserialize, Serialize};

pub const CONFIG_KEY: &str = "notifications_config";
pub const CONFIG_VERSION: u64 = 1;
pub const COSMIC_NOTIFICATIONS_ID: &str = "com.system76.CosmicNotifications";

/// Preset timeout choices in seconds (`0` = persistent / uncapped).
pub const TIMEOUT_OPTIONS: [u32; 5] = [3, 5, 8, 12, 0];

pub const MIN_OPACITY: f32 = 0.40;
pub const MAX_OPACITY: f32 = 1.00;
pub const MIN_CORNER_RADIUS: f32 = 4.0;
pub const MAX_CORNER_RADIUS: f32 = 24.0;
pub const MIN_BODY_LINES: u32 = 1;
pub const MAX_BODY_LINES: u32 = 5;

/// Card density/layout format in the custom notifications panel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotifFormat {
    #[default]
    Full,
    Compact,
    Minimal,
}

impl NotifFormat {
    pub const ALL: [NotifFormat; 3] = [NotifFormat::Full, NotifFormat::Compact, NotifFormat::Minimal];

    pub fn label(self) -> &'static str {
        match self {
            NotifFormat::Full => "Completo",
            NotifFormat::Compact => "Compacto",
            NotifFormat::Minimal => "Mínimo",
        }
    }
}

/// On-screen popup position, synced to `com.system76.CosmicNotifications/v1/anchor`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotifPosition {
    #[default]
    TopRight,
    TopCenter,
    BottomRight,
    BottomLeft,
}

impl NotifPosition {
    pub const ALL: [NotifPosition; 4] = [
        NotifPosition::TopRight,
        NotifPosition::TopCenter,
        NotifPosition::BottomRight,
        NotifPosition::BottomLeft,
    ];

    pub fn label(self) -> &'static str {
        match self {
            NotifPosition::TopRight => "Topo direita",
            NotifPosition::TopCenter => "Topo centro",
            NotifPosition::BottomRight => "Base direita",
            NotifPosition::BottomLeft => "Base esquerda",
        }
    }

    /// Exact RON unit-variant representation expected by `cosmic-notifications`.
    pub fn cosmic_anchor_ron(self) -> &'static str {
        match self {
            NotifPosition::TopRight => "TopRight",
            NotifPosition::TopCenter => "Top",
            NotifPosition::BottomRight => "BottomRight",
            NotifPosition::BottomLeft => "BottomLeft",
        }
    }
}

/// Matches `cosmic_notifications_config::Anchor` for RON serialization via `cosmic_config`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum CosmicAnchor {
    Top,
    TopRight,
    BottomRight,
    BottomLeft,
}

impl From<NotifPosition> for CosmicAnchor {
    fn from(pos: NotifPosition) -> Self {
        match pos {
            NotifPosition::TopRight => CosmicAnchor::TopRight,
            NotifPosition::TopCenter => CosmicAnchor::Top,
            NotifPosition::BottomRight => CosmicAnchor::BottomRight,
            NotifPosition::BottomLeft => CosmicAnchor::BottomLeft,
        }
    }
}

/// Complete configuration for the custom notifications panel and synced daemon options.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NotifConfig {
    /// Popup display timeout in seconds (`0` = persistent / no cap; presets: 3, 5, 8, 12, 0).
    pub timeout_secs: u32,
    /// Card visual density (`Full`, `Compact`, `Minimal`).
    pub format: NotifFormat,
    /// Popup screen anchor (`TopRight`, `TopCenter`, `BottomRight`, `BottomLeft`).
    pub position: NotifPosition,
    /// Card background opacity (`0.40..=1.00`).
    pub opacity: f32,
    /// Card corner radius in logical pixels (`4.0..=24.0`).
    pub corner_radius: f32,
    /// Maximum body text lines shown on a card (`1..=5`).
    pub max_body_lines: u32,
    /// Whether to display the notification body text.
    pub show_body: bool,
    /// Whether to display notification action buttons.
    pub show_actions: bool,
    /// Whether to group notifications from the same app.
    pub group_by_app: bool,
    /// Custom highlight/accent color (`""` = COSMIC theme accent, or `"#RRGGBB"`).
    pub accent_hex: String,
    /// Custom card background color (`""` = COSMIC theme card base, or `"#RRGGBB"`).
    pub bg_hex: String,
}

impl Default for NotifConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 5,
            format: NotifFormat::Full,
            position: NotifPosition::TopRight,
            opacity: 0.95,
            corner_radius: 16.0,
            max_body_lines: 3,
            show_body: true,
            show_actions: true,
            group_by_app: true,
            accent_hex: String::new(),
            bg_hex: String::new(),
        }
    }
}

impl NotifConfig {
    /// Loads the saved notification configuration, falling back to defaults.
    pub fn load() -> Self {
        Config::new(crate::central::APP_ID, CONFIG_VERSION)
            .ok()
            .map(|cfg| Self::load_from(&cfg))
            .unwrap_or_default()
    }

    /// Loads from a specific `cosmic_config::Config` handle and clamps values.
    pub fn load_from(config: &Config) -> Self {
        config
            .get::<NotifConfig>(CONFIG_KEY)
            .unwrap_or_default()
            .clamped()
    }

    /// Returns a normalized copy with numeric fields clamped to valid bounds.
    pub fn clamped(mut self) -> Self {
        self.opacity = if self.opacity.is_finite() {
            self.opacity.clamp(MIN_OPACITY, MAX_OPACITY)
        } else {
            Self::default().opacity
        };
        self.corner_radius = if self.corner_radius.is_finite() {
            self.corner_radius.clamp(MIN_CORNER_RADIUS, MAX_CORNER_RADIUS)
        } else {
            Self::default().corner_radius
        };
        self.max_body_lines = self.max_body_lines.clamp(MIN_BODY_LINES, MAX_BODY_LINES);
        self.accent_hex = self.accent_hex.trim().to_owned();
        self.bg_hex = self.bg_hex.trim().to_owned();
        self
    }

    /// Persists the configuration to `cosmic_config` and syncs anchor & timeout
    /// with `com.system76.CosmicNotifications` v1.
    pub fn save(&self) -> Result<(), String> {
        let clamped = self.clone().clamped();
        let config = Config::new(crate::central::APP_ID, CONFIG_VERSION)
            .map_err(|err| format!("cannot open notifications config: {err}"))?;
        clamped.save_to(&config)?;
        clamped.sync_cosmic_daemon();
        Ok(())
    }

    /// Persists the clamped configuration into a specific `cosmic_config::Config` handle.
    pub fn save_to(&self, config: &Config) -> Result<(), String> {
        let clamped = self.clone().clamped();
        config
            .set(CONFIG_KEY, &clamped)
            .map_err(|err| format!("cannot save {CONFIG_KEY}: {err}"))
    }

    /// Syncs `anchor` and `max_timeout_normal` to `~/.config/cosmic/com.system76.CosmicNotifications/v1/`.
    pub fn sync_cosmic_daemon(&self) {
        let result = Config::new(COSMIC_NOTIFICATIONS_ID, CONFIG_VERSION)
            .map_err(|err| err.to_string())
            .and_then(|cfg| self.sync_cosmic_daemon_to(&cfg));
        if let Err(err) = result {
            tracing::warn!("cannot sync {COSMIC_NOTIFICATIONS_ID}: {err}");
        }
    }

    /// Syncs `anchor` and `max_timeout_normal` into a specific COSMIC Notifications config handle.
    pub fn sync_cosmic_daemon_to(&self, config: &Config) -> Result<(), String> {
        let anchor = CosmicAnchor::from(self.position);
        config
            .set("anchor", anchor)
            .map_err(|err| format!("cannot set anchor: {err}"))?;
        let timeout_ms: Option<u32> = (self.timeout_secs > 0)
            .then(|| self.timeout_secs.saturating_mul(1000));
        config
            .set("max_timeout_normal", timeout_ms)
            .map_err(|err| format!("cannot set max_timeout_normal: {err}"))?;
        Ok(())
    }

    /// Parsed custom accent color (`0.0..=1.0` per channel), or `None` for the COSMIC theme accent.
    pub fn accent_rgb(&self) -> Option<[f32; 3]> {
        parse_hex_color(&self.accent_hex)
    }

    /// Parsed custom card background color (`0.0..=1.0` per channel), or `None` for the COSMIC theme base.
    pub fn bg_rgb(&self) -> Option<[f32; 3]> {
        parse_hex_color(&self.bg_hex)
    }
}

/// Parses a `#RRGGBB` (or `RRGGBB`) hex color string into normalized `[r, g, b]` floats in `0.0..=1.0`.
pub fn parse_hex_color(hex: &str) -> Option<[f32; 3]> {
    let trimmed = hex.trim();
    let digits = trimmed.strip_prefix('#').unwrap_or(trimmed);
    if digits.len() != 6 || !digits.is_ascii() {
        return None;
    }
    let r = u8::from_str_radix(&digits[0..2], 16).ok()?;
    let g = u8::from_str_radix(&digits[2..4], 16).ok()?;
    let b = u8::from_str_radix(&digits[4..6], 16).ok()?;
    Some([f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("central-notif-config-{name}-{}", std::process::id()));
        _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn parse_hex_color_valid_variants() {
        assert_eq!(parse_hex_color("#000000"), Some([0.0, 0.0, 0.0]));
        assert_eq!(parse_hex_color("#FFFFFF"), Some([1.0, 1.0, 1.0]));
        assert_eq!(parse_hex_color("#ff0080"), Some([1.0, 0.0, 128.0 / 255.0]));
        assert_eq!(parse_hex_color("  #1A2b3C \n"), Some([0x1a as f32 / 255.0, 0x2b as f32 / 255.0, 0x3c as f32 / 255.0]));
        assert_eq!(parse_hex_color("336699"), Some([0x33 as f32 / 255.0, 0x66 as f32 / 255.0, 0x99 as f32 / 255.0]));
    }

    #[test]
    fn parse_hex_color_rejects_invalid() {
        for bad in ["", "   ", "#", "#fff", "#12345", "#1234567", "#GGGGGG", "zzzzzz", "##112233", "ééé"] {
            assert_eq!(parse_hex_color(bad), None, "expected None for {bad:?}");
        }
    }

    #[test]
    fn defaults_match_specification() {
        let cfg = NotifConfig::default();
        assert_eq!(cfg.timeout_secs, 5);
        assert_eq!(cfg.format, NotifFormat::Full);
        assert_eq!(cfg.position, NotifPosition::TopRight);
        assert!((cfg.opacity - 0.95).abs() < f32::EPSILON);
        assert!((cfg.corner_radius - 16.0).abs() < f32::EPSILON);
        assert_eq!(cfg.max_body_lines, 3);
        assert!(cfg.show_body);
        assert!(cfg.show_actions);
        assert!(cfg.group_by_app);
        assert!(cfg.accent_hex.is_empty());
        assert!(cfg.bg_hex.is_empty());
        assert_eq!(cfg.accent_rgb(), None);
        assert_eq!(cfg.bg_rgb(), None);
    }

    #[test]
    fn clamped_enforces_ranges_and_handles_nan() {
        let low = NotifConfig {
            opacity: 0.10,
            corner_radius: 1.0,
            max_body_lines: 0,
            accent_hex: "  #FF8800  ".into(),
            bg_hex: " #101010 ".into(),
            ..NotifConfig::default()
        }
        .clamped();
        assert!((low.opacity - MIN_OPACITY).abs() < f32::EPSILON);
        assert!((low.corner_radius - MIN_CORNER_RADIUS).abs() < f32::EPSILON);
        assert_eq!(low.max_body_lines, MIN_BODY_LINES);
        assert_eq!(low.accent_hex, "#FF8800");
        assert_eq!(low.bg_hex, "#101010");

        let high = NotifConfig {
            opacity: 2.5,
            corner_radius: 99.0,
            max_body_lines: 20,
            ..NotifConfig::default()
        }
        .clamped();
        assert!((high.opacity - MAX_OPACITY).abs() < f32::EPSILON);
        assert!((high.corner_radius - MAX_CORNER_RADIUS).abs() < f32::EPSILON);
        assert_eq!(high.max_body_lines, MAX_BODY_LINES);

        let nan = NotifConfig {
            opacity: f32::NAN,
            corner_radius: f32::INFINITY,
            ..NotifConfig::default()
        }
        .clamped();
        assert!((nan.opacity - 0.95).abs() < f32::EPSILON);
        assert!((nan.corner_radius - 16.0).abs() < f32::EPSILON);
    }

    #[test]
    fn cosmic_config_round_trip() {
        let dir = scratch("roundtrip");
        let handle = Config::with_custom_path(crate::central::APP_ID, CONFIG_VERSION, dir.clone()).unwrap();
        assert_eq!(NotifConfig::load_from(&handle), NotifConfig::default());

        let custom = NotifConfig {
            timeout_secs: 12,
            format: NotifFormat::Compact,
            position: NotifPosition::BottomLeft,
            opacity: 0.80,
            corner_radius: 10.0,
            max_body_lines: 2,
            show_body: false,
            show_actions: false,
            group_by_app: false,
            accent_hex: "#4A90E2".into(),
            bg_hex: "#1C1E26".into(),
        };
        custom.save_to(&handle).unwrap();
        assert_eq!(NotifConfig::load_from(&handle), custom);
        _ = std::fs::remove_dir_all(dir);
    }

    fn find_file(root: &std::path::Path, name: &str) -> Option<PathBuf> {
        for entry in std::fs::read_dir(root).ok()?.flatten() {
            let path = entry.path();
            if path.is_file() && path.file_name().is_some_and(|n| n == name) {
                return Some(path);
            }
            if path.is_dir()
                && let Some(found) = find_file(&path, name)
            {
                return Some(found);
            }
        }
        None
    }

    #[test]
    fn sync_cosmic_daemon_writes_ron_anchor_and_timeout() {
        let dir = scratch("daemon-sync");
        let handle = Config::with_custom_path(COSMIC_NOTIFICATIONS_ID, CONFIG_VERSION, dir.clone()).unwrap();

        for (pos, expected_ron) in [
            (NotifPosition::TopRight, "TopRight"),
            (NotifPosition::TopCenter, "Top"),
            (NotifPosition::BottomRight, "BottomRight"),
            (NotifPosition::BottomLeft, "BottomLeft"),
        ] {
            assert_eq!(pos.cosmic_anchor_ron(), expected_ron);
            let cfg = NotifConfig {
                position: pos,
                timeout_secs: 8,
                ..NotifConfig::default()
            };
            cfg.sync_cosmic_daemon_to(&handle).unwrap();
            let anchor: CosmicAnchor = handle.get("anchor").unwrap();
            assert_eq!(anchor, CosmicAnchor::from(pos));
            let anchor_file = find_file(&dir, "anchor").expect("anchor file written");
            let raw_anchor = std::fs::read_to_string(anchor_file).unwrap();
            assert_eq!(raw_anchor.trim(), expected_ron);
            let timeout: Option<u32> = handle.get("max_timeout_normal").unwrap();
            assert_eq!(timeout, Some(8000));
        }

        let persistent = NotifConfig {
            timeout_secs: 0,
            ..NotifConfig::default()
        };
        persistent.sync_cosmic_daemon_to(&handle).unwrap();
        let timeout: Option<u32> = handle.get("max_timeout_normal").unwrap();
        assert_eq!(timeout, None);

        _ = std::fs::remove_dir_all(dir);
    }
}
