// SPDX-License-Identifier: GPL-3.0-only

//! Pure animation math for the bar: proximity zoom, springs, the click bounce.
//! No I/O besides `motion_scale`, so it is testable without a compositor.

pub use crate::shared::anim::{Spring, motion_scale};

pub const MAX_ZOOM: f32 = 1.7;
pub const ZOOM_RADIUS_ICONS: f32 = 2.5;

/// Scale of an icon whose centre is `distance_px` from the pointer: the
/// maximum at the pointer, a cosine falloff, 1.0 from the radius on.
pub fn zoom_scale(distance_px: f32, icon_px: f32) -> f32 {
    let radius = ZOOM_RADIUS_ICONS * icon_px;
    let d = distance_px.abs();
    if d >= radius {
        return 1.0;
    }
    let k = 0.5 * (1.0 + (std::f32::consts::PI * d / radius).cos());
    1.0 + (MAX_ZOOM - 1.0) * k
}

const BOUNCE_FIRST_S: f32 = 0.5;
const BOUNCE_END_S: f32 = 0.8;

/// Upward offset in px of the click bounce, `t_s` seconds after the click:
/// a hop of half an icon, then a smaller one, then rest.
pub fn bounce_offset(t_s: f32, icon_px: f32) -> f32 {
    let hop = |u: f32, height: f32| 4.0 * height * u * (1.0 - u);
    if t_s <= 0.0 || t_s >= BOUNCE_END_S {
        0.0
    } else if t_s < BOUNCE_FIRST_S {
        hop(t_s / BOUNCE_FIRST_S, 0.5 * icon_px)
    } else {
        hop(
            (t_s - BOUNCE_FIRST_S) / (BOUNCE_END_S - BOUNCE_FIRST_S),
            0.25 * icon_px,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_peaks_at_the_pointer_and_ends_at_the_radius() {
        assert_eq!(zoom_scale(0.0, 32.0), MAX_ZOOM);
        assert_eq!(zoom_scale(80.0, 32.0), 1.0);
        assert_eq!(zoom_scale(500.0, 32.0), 1.0);
    }

    #[test]
    fn zoom_falls_monotonically() {
        let mut prev = zoom_scale(0.0, 32.0);
        for d in 1..=80 {
            let s = zoom_scale(d as f32, 32.0);
            assert!(s <= prev, "d={d}: {s} > {prev}");
            prev = s;
        }
    }

    #[test]
    fn zoom_ignores_the_sign_of_the_distance() {
        assert_eq!(zoom_scale(-20.0, 32.0), zoom_scale(20.0, 32.0));
    }




    #[test]
    fn bounce_starts_and_ends_at_zero_and_peaks_in_range() {
        assert_eq!(bounce_offset(0.0, 32.0), 0.0);
        assert_eq!(bounce_offset(1.0, 32.0), 0.0);
        assert_eq!(bounce_offset(-1.0, 32.0), 0.0);
        let peak = (0..=900)
            .map(|ms| bounce_offset(ms as f32 / 1000.0, 32.0))
            .fold(0.0f32, f32::max);
        assert!((12.0..=20.0).contains(&peak), "peak={peak}");
        assert!((0..=900).all(|ms| bounce_offset(ms as f32 / 1000.0, 32.0) >= 0.0));
    }

}
