// SPDX-License-Identifier: GPL-3.0-only

//! Configuration for login/lock screen style, typography, and rotating wallpapers.
//!
//! Reads `~/.config/cosmic-taskbar/greeter.toml` (falling back to
//! `/var/lib/aenes-greeter/greeter.toml` when running as the `cosmic-greeter` user
//! or when the user config file is absent).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use cosmic::iced::{Color, Font, font};

const SYSTEM_FALLBACK_PATH: &str = "/var/lib/aenes-greeter/greeter.toml";

/// Visual and wallpaper configuration shared by `locker` and `greeter`.
#[derive(Clone, Debug, PartialEq)]
pub struct StyleConfig {
    /// Layout style: `"win11"`, `"android"`, `"macos"`, `"sidebar"`, `"editorial"`.
    pub style: String,
    /// Fixed wallpaper file path (used when `wallpaper_mode == "fixed"` or as fallback).
    pub wallpaper: String,
    /// Wallpaper mode: `"fixed"`, `"rotate_on_lock"`, `"rotate_interval"`.
    pub wallpaper_mode: String,
    /// Directory scanned for rotating wallpapers.
    pub wallpaper_dir: String,
    /// Whether rotating wallpapers use deterministic shuffle order.
    pub wallpaper_shuffle: bool,
    /// Rotation interval in minutes when `wallpaper_mode == "rotate_interval"`.
    pub wallpaper_interval_mins: u32,
    /// Blur radius (`0..=60`).
    pub blur_radius: u32,
    /// Darkening overlay opacity on top of the wallpaper (`0.0..=0.85`).
    pub dim_opacity: f32,
    /// Theme polarity: `"dark"` or `"light"`.
    pub theme: String,
    /// Font family: `"system"`, `"sans"`, `"mono"`, `"serif"`.
    pub font_family: String,
    /// Font weight: `"light"`, `"normal"`, `"semibold"`, `"bold"`.
    pub font_weight: String,
    /// Clock size preset: `"small"`, `"medium"`, `"large"`, `"huge"`.
    pub clock_size: String,
    /// 24-hour clock format.
    pub clock_24h: bool,
    /// Display seconds in the clock.
    pub show_seconds: bool,
    /// Display the date underneath/beside the clock.
    pub show_date: bool,
    /// Display the user avatar.
    pub show_avatar: bool,
    /// Display the user name.
    pub show_username: bool,
    /// Glass card opacity behind the login block (`0.0..=0.95`).
    pub card_opacity: f32,
    /// Glass card corner radius in px (`4.0..=32.0`).
    pub card_radius: f32,
    /// Optional accent color (`""` for system accent, or `"#RRGGBB"`).
    pub accent_color: String,
}

impl Default for StyleConfig {
    fn default() -> Self {
        Self {
            style: "win11".to_owned(),
            wallpaper: String::new(),
            wallpaper_mode: "fixed".to_owned(),
            wallpaper_dir: String::new(),
            wallpaper_shuffle: true,
            wallpaper_interval_mins: 30,
            blur_radius: 0,
            dim_opacity: 0.0,
            theme: "dark".to_owned(),
            font_family: "system".to_owned(),
            font_weight: "normal".to_owned(),
            clock_size: "medium".to_owned(),
            clock_24h: true,
            show_seconds: false,
            show_date: true,
            show_avatar: true,
            show_username: true,
            card_opacity: 0.0,
            card_radius: 16.0,
            accent_color: String::new(),
        }
    }
}

impl StyleConfig {
    /// Load configuration from `~/.config/cosmic-taskbar/greeter.toml`, falling back
    /// to `/var/lib/aenes-greeter/greeter.toml` if unavailable.
    pub fn load() -> Self {
        for path in candidate_config_paths() {
            if let Ok(text) = fs::read_to_string(&path) {
                tracing::info!("loaded greeter style config from {:?}", path);
                return Self::parse(&text);
            }
        }
        Self::default()
    }

    /// Zero-dependency line-based TOML parser matching `cosmic-taskbar/src/control_panel/data.rs`.
    pub fn parse(text: &str) -> Self {
        let mut cfg = Self::default();
        for raw_line in text.lines() {
            let line = strip_toml_comment(raw_line).trim();
            if line.is_empty() || line.starts_with('[') {
                continue;
            }
            let Some((key, val)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let val = val.trim();
            match key {
                "style" => {
                    let parsed = parse_toml_string(val).to_lowercase();
                    if matches!(
                        parsed.as_str(),
                        "win11" | "android" | "macos" | "sidebar" | "editorial"
                    ) {
                        cfg.style = parsed;
                    }
                }
                "wallpaper" => cfg.wallpaper = parse_toml_string(val),
                "wallpaper_mode" => {
                    let parsed = parse_toml_string(val).to_lowercase();
                    if matches!(
                        parsed.as_str(),
                        "fixed" | "rotate_on_lock" | "rotate_interval"
                    ) {
                        cfg.wallpaper_mode = parsed;
                    }
                }
                "wallpaper_dir" => cfg.wallpaper_dir = parse_toml_string(val),
                "wallpaper_shuffle" => {
                    if let Ok(b) = val.parse::<bool>() {
                        cfg.wallpaper_shuffle = b;
                    }
                }
                "wallpaper_interval_mins" => {
                    if let Ok(n) = val.parse::<u32>() {
                        cfg.wallpaper_interval_mins = n.clamp(1, 1440);
                    }
                }
                "blur_radius" => {
                    if let Ok(n) = val.parse::<u32>() {
                        cfg.blur_radius = n.min(60);
                    }
                }
                "dim_opacity" => {
                    if let Ok(f) = val.parse::<f32>()
                        && f.is_finite()
                    {
                        cfg.dim_opacity = f.clamp(0.0, 0.85);
                    }
                }
                "theme" => {
                    let parsed = parse_toml_string(val).to_lowercase();
                    if matches!(parsed.as_str(), "dark" | "light") {
                        cfg.theme = parsed;
                    }
                }
                "font_family" => {
                    let parsed = parse_toml_string(val).to_lowercase();
                    if matches!(parsed.as_str(), "system" | "sans" | "mono" | "serif") {
                        cfg.font_family = parsed;
                    }
                }
                "font_weight" => {
                    let parsed = parse_toml_string(val).to_lowercase();
                    if matches!(parsed.as_str(), "light" | "normal" | "semibold" | "bold") {
                        cfg.font_weight = parsed;
                    }
                }
                "clock_size" => {
                    let parsed = parse_toml_string(val).to_lowercase();
                    if matches!(parsed.as_str(), "small" | "medium" | "large" | "huge") {
                        cfg.clock_size = parsed;
                    }
                }
                "clock_24h" => {
                    if let Ok(b) = val.parse::<bool>() {
                        cfg.clock_24h = b;
                    }
                }
                "show_seconds" => {
                    if let Ok(b) = val.parse::<bool>() {
                        cfg.show_seconds = b;
                    }
                }
                "show_date" => {
                    if let Ok(b) = val.parse::<bool>() {
                        cfg.show_date = b;
                    }
                }
                "show_avatar" => {
                    if let Ok(b) = val.parse::<bool>() {
                        cfg.show_avatar = b;
                    }
                }
                "show_username" => {
                    if let Ok(b) = val.parse::<bool>() {
                        cfg.show_username = b;
                    }
                }
                "card_opacity" => {
                    if let Ok(f) = val.parse::<f32>()
                        && f.is_finite()
                    {
                        cfg.card_opacity = f.clamp(0.0, 0.95);
                    }
                }
                "card_radius" => {
                    if let Ok(f) = val.parse::<f32>()
                        && f.is_finite()
                    {
                        cfg.card_radius = f.clamp(4.0, 32.0);
                    }
                }
                "accent_color" => cfg.accent_color = parse_toml_string(val),
                _ => {}
            }
        }
        cfg
    }

    /// Pick the wallpaper path according to `wallpaper_mode` and `lock_count`.
    pub fn pick_wallpaper(&self, lock_count: u64) -> Option<PathBuf> {
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.pick_wallpaper_at(lock_count, now_secs)
    }

    /// Deterministic variant of [`Self::pick_wallpaper`] for testing interval rotation.
    pub fn pick_wallpaper_at(&self, lock_count: u64, now_unix_secs: u64) -> Option<PathBuf> {
        match self.wallpaper_mode.as_str() {
            "rotate_on_lock" | "rotate_interval" => {
                let dir = self.resolved_wallpaper_dir();
                let mut files = dir.map(|d| scan_wallpapers(&d)).unwrap_or_default();
                if files.is_empty() {
                    return self.fixed_wallpaper_path();
                }
                if self.wallpaper_shuffle {
                    let seed = 0x9E37_79B9_7F4A_7C15 ^ (files.len() as u64);
                    deterministic_shuffle(&mut files, seed);
                }
                let idx = if self.wallpaper_mode == "rotate_interval" {
                    let period = u64::from(self.wallpaper_interval_mins.max(1)) * 60;
                    ((now_unix_secs / period) as usize) % files.len()
                } else {
                    (lock_count as usize) % files.len()
                };
                files.into_iter().nth(idx)
            }
            _ => self.fixed_wallpaper_path(),
        }
    }

    fn fixed_wallpaper_path(&self) -> Option<PathBuf> {
        let trimmed = self.wallpaper.trim();
        if trimmed.is_empty() {
            return None;
        }
        let path = expand_tilde(trimmed);
        path.is_file().then_some(path)
    }

    fn resolved_wallpaper_dir(&self) -> Option<PathBuf> {
        let trimmed = self.wallpaper_dir.trim();
        if !trimmed.is_empty() {
            let path = expand_tilde(trimmed);
            if path.is_dir() {
                return Some(path);
            }
        }
        let home = dirs::home_dir()?;
        for rel in ["Imagens/Wallpapers", "Imagens", "Pictures/Wallpapers", "Pictures"] {
            let candidate = home.join(rel);
            if candidate.is_dir() {
                return Some(candidate);
            }
        }
        None
    }

    /// Clock scale multiplier based on `clock_size`.
    pub fn clock_scale(&self) -> f32 {
        match self.clock_size.as_str() {
            "small" => 0.72,
            "large" => 1.25,
            "huge" => 1.55,
            _ => 1.0,
        }
    }

    /// Returns `true` when light theme is selected.
    pub fn is_light(&self) -> bool {
        self.theme == "light"
    }

    /// Resolve the configured font family and weight.
    pub fn font(&self) -> Font {
        let weight = match self.font_weight.as_str() {
            "light" => font::Weight::Light,
            "semibold" => font::Weight::Semibold,
            "bold" => font::Weight::Bold,
            _ => font::Weight::Normal,
        };
        self.font_with_weight(weight)
    }

    /// Resolve the configured font family with an explicit weight override.
    pub fn font_with_weight(&self, weight: font::Weight) -> Font {
        let base = match self.font_family.as_str() {
            "mono" => Font::MONOSPACE,
            "serif" => Font {
                family: font::Family::Serif,
                ..Font::DEFAULT
            },
            "sans" => Font {
                family: font::Family::SansSerif,
                ..Font::DEFAULT
            },
            _ => cosmic::font::default(),
        };
        Font { weight, ..base }
    }

    /// Parse `#RRGGBB` accent color override, if valid.
    pub fn accent_color_rgba(&self) -> Option<Color> {
        parse_hex_color(&self.accent_color)
    }
}

/// Load a wallpaper image file into a libcosmic image handle.
pub fn load_wallpaper_handle(path: &Path) -> Option<cosmic::widget::image::Handle> {
    match fs::read(path) {
        Ok(bytes) if !bytes.is_empty() => Some(cosmic::widget::image::Handle::from_bytes(bytes)),
        Ok(_) => None,
        Err(err) => {
            tracing::warn!("failed to read wallpaper {:?}: {}", path, err);
            None
        }
    }
}

fn candidate_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::with_capacity(2);
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        paths.push(PathBuf::from(xdg).join("cosmic-taskbar/greeter.toml"));
    } else if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".config/cosmic-taskbar/greeter.toml"));
    }
    paths.push(PathBuf::from(SYSTEM_FALLBACK_PATH));
    paths
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    if path == "~"
        && let Some(home) = dirs::home_dir()
    {
        return home;
    }
    PathBuf::from(path)
}

fn is_wallpaper_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "png" | "jpg" | "jpeg" | "webp"
    )
}

/// Scan `dir` for wallpaper images (non-recursive + 1 level of subdirectories), sorted deterministically.
fn scan_wallpapers(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if is_wallpaper_file(&path) {
            out.push(path);
        } else if path.is_dir()
            && let Ok(sub_entries) = fs::read_dir(&path)
        {
            for sub in sub_entries.flatten() {
                let sub_path = sub.path();
                if is_wallpaper_file(&sub_path) {
                    out.push(sub_path);
                }
            }
        }
    }
    out.sort();
    out
}

/// Deterministic Fisher-Yates shuffle using splitmix64.
fn deterministic_shuffle<T>(slice: &mut [T], mut state: u64) {
    let len = slice.len();
    if len <= 1 {
        return;
    }
    for i in (1..len).rev() {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        let j = (z as usize) % (i + 1);
        slice.swap(i, j);
    }
}

fn parse_hex_color(raw: &str) -> Option<Color> {
    let hex = raw.trim().strip_prefix('#').unwrap_or(raw.trim());
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::from_rgb8(r, g, b))
}

fn strip_toml_comment(line: &str) -> &str {
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    for (idx, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_double => escaped = true,
            '"' if !in_single => in_double = !in_double,
            '\'' if !in_double => in_single = !in_single,
            '#' if !in_single && !in_double => return &line[..idx],
            _ => {}
        }
    }
    line
}

fn parse_toml_string(raw: &str) -> String {
    let s = raw.trim();
    if let Some(inner) = s.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        let mut out = String::with_capacity(inner.len());
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some(other) => {
                        out.push('\\');
                        out.push(other);
                    }
                    None => out.push('\\'),
                }
            } else {
                out.push(c);
            }
        }
        out
    } else if let Some(inner) = s.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
        inner.to_owned()
    } else {
        s.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cosmic_greeter_style_test_{}_{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn parse_all_fields_and_comments() {
        let toml = r##"
            # Full config
            style = "android" # inline comment
            wallpaper = "/tmp/wall.png"
            wallpaper_mode = "rotate_on_lock"
            wallpaper_dir = "/tmp/walls"
            wallpaper_shuffle = false
            wallpaper_interval_mins = 45
            blur_radius = 24
            dim_opacity = 0.35
            theme = "light"
            font_family = "mono"
            font_weight = "bold"
            clock_size = "huge"
            clock_24h = false
            show_seconds = true
            show_date = false
            show_avatar = false
            show_username = false
            card_opacity = 0.65
            card_radius = 22.0
            accent_color = "#ff5500"
        "##;
        let cfg = StyleConfig::parse(toml);
        assert_eq!(cfg.style, "android");
        assert_eq!(cfg.wallpaper, "/tmp/wall.png");
        assert_eq!(cfg.wallpaper_mode, "rotate_on_lock");
        assert_eq!(cfg.wallpaper_dir, "/tmp/walls");
        assert!(!cfg.wallpaper_shuffle);
        assert_eq!(cfg.wallpaper_interval_mins, 45);
        assert_eq!(cfg.blur_radius, 24);
        assert!((cfg.dim_opacity - 0.35).abs() < 1e-6);
        assert_eq!(cfg.theme, "light");
        assert_eq!(cfg.font_family, "mono");
        assert_eq!(cfg.font_weight, "bold");
        assert_eq!(cfg.clock_size, "huge");
        assert!(!cfg.clock_24h);
        assert!(cfg.show_seconds);
        assert!(!cfg.show_date);
        assert!(!cfg.show_avatar);
        assert!(!cfg.show_username);
        assert!((cfg.card_opacity - 0.65).abs() < 1e-6);
        assert!((cfg.card_radius - 22.0).abs() < 1e-6);
        assert_eq!(cfg.accent_color, "#ff5500");
        assert!(cfg.accent_color_rgba().is_some());
    }

    #[test]
    fn parse_clamps_and_rejects_invalid_enums() {
        let toml = r#"
            style = "invalid"
            wallpaper_mode = "bogus"
            wallpaper_interval_mins = 0
            blur_radius = 999
            dim_opacity = 2.5
            card_opacity = -1.0
            card_radius = 100.0
            theme = "neon"
            font_family = "comic"
            font_weight = "black"
            clock_size = "tiny"
        "#;
        let cfg = StyleConfig::parse(toml);
        assert_eq!(cfg.style, "win11");
        assert_eq!(cfg.wallpaper_mode, "fixed");
        assert_eq!(cfg.wallpaper_interval_mins, 1);
        assert_eq!(cfg.blur_radius, 60);
        assert_eq!(cfg.dim_opacity, 0.85);
        assert_eq!(cfg.card_opacity, 0.0);
        assert_eq!(cfg.card_radius, 32.0);
        assert_eq!(cfg.theme, "dark");
        assert_eq!(cfg.font_family, "system");
        assert_eq!(cfg.font_weight, "normal");
        assert_eq!(cfg.clock_size, "medium");
    }

    #[test]
    fn pick_wallpaper_fixed_and_rotate_modes() {
        let dir = make_temp_dir("pick");
        let sub = dir.join("sub");
        let deep = sub.join("deep");
        fs::create_dir_all(&deep).unwrap();

        let a = dir.join("a.png");
        let b = dir.join("b.JPG");
        let c = sub.join("c.webp");
        let ignored_txt = dir.join("notes.txt");
        let ignored_deep = deep.join("d.png");
        for p in [&a, &b, &c, &ignored_txt, &ignored_deep] {
            fs::write(p, b"img").unwrap();
        }

        // Fixed mode
        let mut cfg = StyleConfig::default();
        cfg.wallpaper_mode = "fixed".into();
        cfg.wallpaper = a.to_string_lossy().into_owned();
        assert_eq!(cfg.pick_wallpaper_at(0, 0), Some(a.clone()));
        assert_eq!(cfg.pick_wallpaper_at(5, 9999), Some(a.clone()));

        // Rotate on lock (sequential)
        cfg.wallpaper_mode = "rotate_on_lock".into();
        cfg.wallpaper_dir = dir.to_string_lossy().into_owned();
        cfg.wallpaper_shuffle = false;
        assert_eq!(cfg.pick_wallpaper_at(0, 0), Some(a.clone()));
        assert_eq!(cfg.pick_wallpaper_at(1, 0), Some(b.clone()));
        assert_eq!(cfg.pick_wallpaper_at(2, 0), Some(c.clone()));
        assert_eq!(cfg.pick_wallpaper_at(3, 0), Some(a.clone()));

        // Rotate on lock (deterministic shuffle covers all 3)
        cfg.wallpaper_shuffle = true;
        let picks: Vec<PathBuf> = (0..3)
            .map(|i| cfg.pick_wallpaper_at(i, 0).unwrap())
            .collect();
        assert_eq!(picks.len(), 3);
        assert!(picks.contains(&a) && picks.contains(&b) && picks.contains(&c));

        // Rotate interval (every 10 minutes = 600s)
        cfg.wallpaper_mode = "rotate_interval".into();
        cfg.wallpaper_interval_mins = 10;
        cfg.wallpaper_shuffle = false;
        assert_eq!(cfg.pick_wallpaper_at(0, 0), Some(a.clone()));
        assert_eq!(cfg.pick_wallpaper_at(0, 599), Some(a.clone()));
        assert_eq!(cfg.pick_wallpaper_at(0, 600), Some(b.clone()));
        assert_eq!(cfg.pick_wallpaper_at(0, 1200), Some(c.clone()));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn hex_color_parsing() {
        assert_eq!(parse_hex_color("#ffffff"), Some(Color::WHITE));
        assert_eq!(parse_hex_color("000000"), Some(Color::BLACK));
        assert_eq!(parse_hex_color(""), None);
        assert_eq!(parse_hex_color("#12345"), None);
        assert_eq!(parse_hex_color("#gggggg"), None);
    }
}
