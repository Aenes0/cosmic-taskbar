// SPDX-License-Identifier: GPL-3.0-only

//! User-configurable settings for the Windows-style screen snipping tool
//! (`extras/bin/print-recorte`, bound to `Win+Shift+S`).
//!
//! Stored in `~/.config/cosmic-taskbar/screenshot.conf` as shell-sourceable
//! `KEY="VALUE"` pairs so `print-recorte` reads it directly with zero overhead.

use std::{
    io,
    path::{Path, PathBuf},
};

/// Action taken immediately after capturing a region.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OpenMode {
    /// Send a notification with thumbnail + "Editar" / "Abrir pasta" actions (default).
    #[default]
    Notify,
    /// Open Satty annotation editor immediately.
    Satty,
    /// Show the floating GTK4 `print-preview` card.
    Preview,
    /// Save & copy silently without notification or editor.
    Silent,
}

impl OpenMode {
    pub const ALL: [OpenMode; 4] = [
        OpenMode::Notify,
        OpenMode::Satty,
        OpenMode::Preview,
        OpenMode::Silent,
    ];

    pub fn id(self) -> &'static str {
        match self {
            OpenMode::Notify => "notify",
            OpenMode::Satty => "satty",
            OpenMode::Preview => "preview",
            OpenMode::Silent => "silent",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id.trim().to_ascii_lowercase().as_str() {
            "notify" => Some(OpenMode::Notify),
            "satty" => Some(OpenMode::Satty),
            "preview" => Some(OpenMode::Preview),
            "silent" => Some(OpenMode::Silent),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            OpenMode::Notify => "Notificação",
            OpenMode::Satty => "Abrir Satty",
            OpenMode::Preview => "Prévia flutuante",
            OpenMode::Silent => "Silencioso",
        }
    }
}

/// Initial drawing tool selected when opening Satty.
pub const SATTY_TOOLS: &[(&str, &str)] = &[
    ("arrow", "Seta"),
    ("brush", "Pincel"),
    ("rectangle", "Retângulo"),
    ("text", "Texto"),
    ("blur", "Desfoque"),
    ("highlight", "Marca-texto"),
    ("crop", "Recortar"),
];

/// Selection border color presets (`#RRGGBB`).
pub const BORDER_COLOR_PRESETS: &[(&str, &str)] = &[
    ("#3daee9", "Azul"),
    ("#2ec27e", "Verde"),
    ("#ff7800", "Laranja"),
    ("#e01b24", "Vermelho"),
    ("#ffffff", "Branco"),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenshotConfig {
    /// Custom destination folder; empty = `$(xdg-user-dir PICTURES)/Screenshots`.
    pub save_dir: String,
    /// Copy captured PNG to Wayland clipboard via `wl-copy`.
    pub copy_clipboard: bool,
    /// What happens right after capture.
    pub open_mode: OpenMode,
    /// Initial Satty tool (`arrow`, `brush`, `rectangle`, `text`, `blur`, `highlight`, `crop`).
    pub satty_tool: String,
    /// Slurp selection border hex color (`#RRGGBB`).
    pub border_color: String,
    /// Slurp selection border thickness in pixels (`1..=6`).
    pub border_width: u32,
    /// Slurp background dim opacity in percent (`0..=85`).
    pub overlay_opacity: u32,
    /// Seconds the notification action helper waits before exiting (`30..=3600`).
    pub notify_timeout_secs: u32,
    /// Attach `sound-name:screen-capture` hint to the notification.
    pub play_sound: bool,
}

impl Default for ScreenshotConfig {
    fn default() -> Self {
        Self {
            save_dir: String::new(),
            copy_clipboard: true,
            open_mode: OpenMode::Notify,
            satty_tool: "arrow".to_owned(),
            border_color: "#3daee9".to_owned(),
            border_width: 2,
            overlay_opacity: 40,
            notify_timeout_secs: 600,
            play_sound: true,
        }
    }
}

impl ScreenshotConfig {
    pub fn config_path() -> PathBuf {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config")
            });
        base.join("cosmic-taskbar/screenshot.conf")
    }

    pub fn load() -> Self {
        Self::load_from(&Self::config_path())
    }

    pub fn load_from(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> io::Result<()> {
        self.save_to(&Self::config_path())
    }

    pub fn save_to(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body = self.format_conf();
        let tmp = path.with_file_name(format!(
            ".screenshot.conf.tmp.{}",
            std::process::id()
        ));
        std::fs::write(&tmp, body.as_bytes())?;
        std::fs::rename(&tmp, path)
    }

    pub fn resolved_dir(&self) -> PathBuf {
        let trimmed = self.save_dir.trim();
        if !trimmed.is_empty() {
            if let Some(rest) = trimmed.strip_prefix("~/") {
                return PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(rest);
            }
            return PathBuf::from(trimmed);
        }
        let home = std::env::var("HOME").unwrap_or_default();
        let imagens = PathBuf::from(&home).join("Imagens/Screenshots");
        if imagens.parent().is_some_and(Path::is_dir) {
            imagens
        } else {
            PathBuf::from(home).join("Pictures/Screenshots")
        }
    }

    pub fn normalized(&self) -> Self {
        let mut out = self.clone();
        out.save_dir = sanitize_shell_value(&out.save_dir);
        if !SATTY_TOOLS.iter().any(|(id, _)| *id == out.satty_tool) {
            out.satty_tool = "arrow".to_owned();
        }
        out.border_color = normalize_hex_rgb(&out.border_color)
            .unwrap_or_else(|| "#3daee9".to_owned());
        out.border_width = out.border_width.clamp(1, 6);
        out.overlay_opacity = out.overlay_opacity.clamp(0, 85);
        out.notify_timeout_secs = out.notify_timeout_secs.clamp(30, 3600);
        out
    }

    pub fn format_conf(&self) -> String {
        let n = self.normalized();
        let hex = n.border_color.trim_start_matches('#');
        let dim_alpha = ((n.overlay_opacity as f32 / 100.0) * 255.0).round() as u8;
        let b = |v: bool| if v { 1u8 } else { 0u8 };
        format!(
            "# Configuração do print-recorte (Win+Shift+S)\n\
             # Editado pelo Painel de Controle do cosmic-taskbar\n\
             \n\
             SAVE_DIR=\"{}\"\n\
             COPY_CLIPBOARD={}\n\
             OPEN_MODE=\"{}\"\n\
             SATTY_TOOL=\"{}\"\n\
             BORDER_COLOR=\"#{hex}\"\n\
             SLURP_BORDER=\"{hex}ff\"\n\
             SLURP_FILL=\"{hex}22\"\n\
             SLURP_BG=\"000000{dim_alpha:02x}\"\n\
             BORDER_WIDTH={}\n\
             OVERLAY_OPACITY={}\n\
             NOTIFY_TIMEOUT_SECS={}\n\
             PLAY_SOUND={}\n",
            n.save_dir,
            b(n.copy_clipboard),
            n.open_mode.id(),
            n.satty_tool,
            n.border_width,
            n.overlay_opacity,
            n.notify_timeout_secs,
            b(n.play_sound),
        )
    }

    pub fn parse(text: &str) -> Self {
        let mut cfg = Self::default();
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, val)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let val = strip_quotes(val.trim());
            match key {
                "SAVE_DIR" => cfg.save_dir = val.to_owned(),
                "COPY_CLIPBOARD" => {
                    if let Some(b) = parse_bool(val) {
                        cfg.copy_clipboard = b;
                    }
                }
                "OPEN_MODE" => {
                    if let Some(m) = OpenMode::from_id(val) {
                        cfg.open_mode = m;
                    }
                }
                "SATTY_TOOL" => {
                    if SATTY_TOOLS.iter().any(|(id, _)| *id == val) {
                        cfg.satty_tool = val.to_owned();
                    }
                }
                "BORDER_COLOR" => {
                    if let Some(hex) = normalize_hex_rgb(val) {
                        cfg.border_color = hex;
                    }
                }
                "BORDER_WIDTH" => {
                    if let Ok(w) = val.parse::<u32>() {
                        cfg.border_width = w.clamp(1, 6);
                    }
                }
                "OVERLAY_OPACITY" => {
                    if let Ok(op) = val.parse::<u32>() {
                        cfg.overlay_opacity = op.clamp(0, 85);
                    }
                }
                "NOTIFY_TIMEOUT_SECS" => {
                    if let Ok(s) = val.parse::<u32>() {
                        cfg.notify_timeout_secs = s.clamp(30, 3600);
                    }
                }
                "PLAY_SOUND" => {
                    if let Some(b) = parse_bool(val) {
                        cfg.play_sound = b;
                    }
                }
                _ => {}
            }
        }
        cfg
    }
}

fn strip_quotes(val: &str) -> &str {
    if let Some(inner) = val.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        inner
    } else if let Some(inner) = val.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        inner
    } else {
        val
    }
}

fn parse_bool(val: &str) -> Option<bool> {
    match val.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Strip characters that could break double-quoted shell assignments.
fn sanitize_shell_value(val: &str) -> String {
    val.trim()
        .chars()
        .filter(|&c| !matches!(c, '"' | '`' | '$' | '\\' | '\n' | '\r' | '\0'))
        .collect()
}

pub fn normalize_hex_rgb(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let digits = trimmed.strip_prefix('#').unwrap_or(trimmed);
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("#{}", digits.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_save_and_load() {
        let dir = std::env::temp_dir().join(format!("screenshot-cfg-test-{}", std::process::id()));
        _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("screenshot.conf");

        let cfg = ScreenshotConfig {
            save_dir: "/home/user/Capturas".into(),
            copy_clipboard: false,
            open_mode: OpenMode::Satty,
            satty_tool: "blur".into(),
            border_color: "#2EC27E".into(),
            border_width: 4,
            overlay_opacity: 60,
            notify_timeout_secs: 180,
            play_sound: false,
        };
        cfg.save_to(&path).unwrap();
        let loaded = ScreenshotConfig::load_from(&path);
        assert_eq!(loaded, cfg.normalized());

        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains("SLURP_BORDER=\"2ec27eff\""));
        assert!(raw.contains("SLURP_BG=\"00000099\""));
        assert!(raw.contains("OPEN_MODE=\"satty\""));

        _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sanitizes_shell_injection_and_clamps_ranges() {
        let cfg = ScreenshotConfig {
            save_dir: "/tmp/$(rm -rf /)`bad`\"q\"".into(),
            border_color: "invalid".into(),
            border_width: 99,
            overlay_opacity: 200,
            notify_timeout_secs: 5,
            satty_tool: "evil;cmd".into(),
            ..ScreenshotConfig::default()
        };
        let n = cfg.normalized();
        assert!(!n.save_dir.contains('$'));
        assert!(!n.save_dir.contains('`'));
        assert!(!n.save_dir.contains('"'));
        assert_eq!(n.border_color, "#3daee9");
        assert_eq!(n.border_width, 6);
        assert_eq!(n.overlay_opacity, 85);
        assert_eq!(n.notify_timeout_secs, 30);
        assert_eq!(n.satty_tool, "arrow");
    }
}
