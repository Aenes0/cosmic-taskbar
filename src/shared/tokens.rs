// SPDX-License-Identifier: GPL-3.0-only

//! Shared visual design tokens for consistent polish across Start, Central,
//! Tools, and Taskbar popups.

use std::time::Duration;

use cosmic::iced::{Color, Shadow, Vector};

/// Standard corner radius for floating menus and popups.
pub const POPUP_RADIUS: f32 = 12.0;

/// Standard border width for crisp hairline edges on floating cards.
pub const HAIRLINE_WIDTH: f32 = 1.0;

/// Short compositor focus-settling window after opening a layer surface.
/// Long enough to ignore the initial focus handoff from the panel, short enough
/// that quick intentional clicks outside always register immediately.
pub const FOCUS_SETTLING: Duration = Duration::from_millis(150);

/// Soft elevation shadow for floating menus and panels.
pub fn popup_shadow() -> Shadow {
    Shadow {
        color: Color::from_rgba(0.0, 0.0, 0.0, 0.36),
        offset: Vector::new(0.0, 6.0),
        blur_radius: 16.0,
    }
}

/// Slightly tighter shadow for compact context menus (Win+X, taskbar menus).
pub fn menu_shadow() -> Shadow {
    Shadow {
        color: Color::from_rgba(0.0, 0.0, 0.0, 0.34),
        offset: Vector::new(0.0, 4.0),
        blur_radius: 14.0,
    }
}

/// Scales a base animation duration by the user's `motion_scale()`.
/// Returns `Duration::ZERO` when reduce-motion is active.
pub fn scaled_duration(base_ms: u64) -> Duration {
    let scale = crate::shared::anim::motion_scale();
    if scale <= 0.0 {
        Duration::ZERO
    } else {
        Duration::from_secs_f32((base_ms as f32 / 1000.0) * scale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_settling_is_snappy() {
        assert!(FOCUS_SETTLING <= Duration::from_millis(200));
        assert!(FOCUS_SETTLING >= Duration::from_millis(80));
    }

    #[test]
    fn shadows_have_positive_blur() {
        assert!(popup_shadow().blur_radius > 0.0);
        assert!(menu_shadow().blur_radius > 0.0);
    }
}
