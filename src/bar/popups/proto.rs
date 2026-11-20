// SPDX-License-Identifier: GPL-3.0-only

//! Wire protocol (JSON lines over a Unix socket) and pure layout/state helpers
//! shared between the panel taskbar applet and the layer-shell popup host.

use std::path::PathBuf;
use std::time::Duration;

use cosmic::applet::cosmic_panel_config::PanelAnchor;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;

/// Panel edge in serializable form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnchorEdge {
    Bottom,
    Top,
    Left,
    Right,
}

impl From<PanelAnchor> for AnchorEdge {
    fn from(anchor: PanelAnchor) -> Self {
        match anchor {
            PanelAnchor::Bottom => Self::Bottom,
            PanelAnchor::Top => Self::Top,
            PanelAnchor::Left => Self::Left,
            PanelAnchor::Right => Self::Right,
        }
    }
}

impl AnchorEdge {
    pub fn is_horizontal(self) -> bool {
        matches!(self, Self::Bottom | Self::Top)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuAction {
    pub name: String,
    pub exec: String,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PopupCommand {
    ShowPreview {
        output: String,
        anchor: AnchorEdge,
        icon_center: f32,
        app_id: String,
        icon_name: String,
        window_ids: Vec<String>,
    },
    HidePreview {
        output: String,
    },
    ShowMenu {
        output: String,
        anchor: AnchorEdge,
        icon_center: f32,
        app_id: String,
        original_app_id: String,
        app_name: String,
        icon_name: String,
        exec: Option<String>,
        terminal: bool,
        is_pinned: bool,
        actions: Vec<MenuAction>,
        window_ids: Vec<String>,
    },
    HideAll,
}

/// Socket path in `$XDG_RUNTIME_DIR` (or `/tmp` fallback).
pub fn socket_path() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    dir.join("cosmic-taskbar-popups.sock")
}

/// Default width of the right-click menu in logical px.
pub const MENU_WIDTH: f32 = 264.0;
/// Maximum box size of one preview thumbnail card.
pub const PREVIEW_MAX_W: f32 = 300.0;
pub const PREVIEW_MAX_H: f32 = 150.0;
pub const PREVIEW_SPACING: f32 = 8.0;
pub const PREVIEW_PADDING: f32 = 8.0;

/// Estimated major-axis size of a preview popup for centering over `icon_center`.
pub fn estimate_preview_major(window_count: usize, horizontal: bool) -> f32 {
    let n = window_count.clamp(1, 6) as f32;
    let card = if horizontal { 240.0 } else { PREVIEW_MAX_H };
    n * card + (n - 1.0).max(0.0) * PREVIEW_SPACING + 2.0 * PREVIEW_PADDING
}

/// Clamps the leading margin (left for horizontal, top for vertical) so a
/// popup of `popup_size` centered on `icon_center` stays within `[pad, screen - popup - pad]`.
pub fn clamp_margin(icon_center: f32, popup_size: f32, screen_size: f32, pad: f32) -> i32 {
    let ideal = icon_center - popup_size / 2.0;
    if screen_size <= 0.0 {
        return ideal.max(pad).round() as i32;
    }
    let max_pos = (screen_size - popup_size - pad).max(pad);
    ideal.clamp(pad, max_pos).round() as i32
}

/// Computes the icon's center coordinate along the panel's major axis in screen coordinates.
pub fn icon_center_on_screen(
    icon_pos_in_bar: f32,
    icon_len: f32,
    bar_total_len: f32,
    prefix_len: f32,
    screen_len: f32,
) -> f32 {
    let group_len = prefix_len + bar_total_len;
    let group_start = ((screen_len - group_len) / 2.0).max(0.0);
    group_start + prefix_len + icon_pos_in_bar + icon_len / 2.0
}

/// Sends a command to the resident `--taskbar-popups` host, lazily spawning it
/// on Vulkan GPU if it is not running yet.
pub async fn send_command(cmd: PopupCommand) {
    let Ok(mut payload) = serde_json::to_string(&cmd) else {
        return;
    };
    payload.push('\n');
    let path = socket_path();

    let stream = match UnixStream::connect(&path).await {
        Ok(s) => Some(s),
        Err(_) => {
            // Never spawn the host just to tell it to hide.
            if matches!(cmd, PopupCommand::HidePreview { .. } | PopupCommand::HideAll) {
                return;
            }
            spawn_host();
            let mut connected = None;
            for _ in 0..30 {
                tokio::time::sleep(Duration::from_millis(50)).await;
                if let Ok(s) = UnixStream::connect(&path).await {
                    connected = Some(s);
                    break;
                }
            }
            connected
        }
    };

    if let Some(mut stream) = stream {
        let _ = stream.write_all(payload.as_bytes()).await;
        let _ = stream.shutdown().await;
    }
}

fn resolve_exe() -> Option<PathBuf> {
    if let Some(arg0) = std::env::args_os().next() {
        let p = PathBuf::from(&arg0);
        if p.is_absolute() && p.exists() {
            return Some(p);
        }
    }
    let exe = std::env::current_exe().ok()?;
    let s = exe.to_string_lossy();
    if let Some(stripped) = s.strip_suffix(" (deleted)") {
        let p = PathBuf::from(stripped);
        if p.exists() {
            return Some(p);
        }
    }
    Some(exe)
}

fn spawn_host() {
    let Some(exe) = resolve_exe() else {
        tracing::warn!("taskbar-popups: could not resolve executable path");
        return;
    };
    let mut command = std::process::Command::new(&exe);
    command.arg("--taskbar-popups");
    // Restore original ICED_BACKEND so the layer-shell popup host runs on Vulkan GPU
    // instead of inheriting the panel applet's tiny-skia override.
    match crate::bar::ORIGINAL_BACKEND.get() {
        Some(Some(val)) => {
            command.env("ICED_BACKEND", val);
        }
        Some(None) => {
            command.env_remove("ICED_BACKEND");
        }
        None => {}
    }
    // Remove panel-specific nested Wayland socket so it connects directly to cosmic-comp.
    command.env_remove("WAYLAND_SOCKET");
    command.env_remove("X_PRIVILEGED_WAYLAND_SOCKET");
    command.stdin(std::process::Stdio::null());
    if let Err(err) = command.spawn() {
        tracing::warn!("taskbar-popups: failed to spawn {}: {err}", exe.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_margin_centers_and_stays_inside_screen() {
        assert_eq!(clamp_margin(960.0, 264.0, 1920.0, 8.0), 828);
        assert_eq!(clamp_margin(40.0, 264.0, 1920.0, 8.0), 8);
        assert_eq!(clamp_margin(1900.0, 264.0, 1920.0, 8.0), 1648);
    }

    #[test]
    fn icon_center_on_screen_accounts_for_center_group_and_prefix() {
        // Screen 1920, prefix 80 (Start + Workspaces), taskbar 400 -> group 480, start 720.
        // Taskbar starts at 800; icon at x=100, w=48 -> center at 924.
        let center = icon_center_on_screen(100.0, 48.0, 400.0, 80.0, 1920.0);
        assert!((center - 924.0).abs() < 0.01, "center={center}");
    }

    #[test]
    fn popup_command_roundtrips_json() {
        let cmd = PopupCommand::ShowPreview {
            output: "DP-3".into(),
            anchor: AnchorEdge::Bottom,
            icon_center: 924.0,
            app_id: "brave-browser".into(),
            icon_name: "brave-browser".into(),
            window_ids: vec!["win-1".into(), "win-2".into()],
        };
        let s = serde_json::to_string(&cmd).unwrap();
        let back: PopupCommand = serde_json::from_str(&s).unwrap();
        assert_eq!(cmd, back);
    }
}
