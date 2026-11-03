// SPDX-License-Identifier: GPL-3.0-only

//! Pure animation math for the bar: proximity zoom, springs, the click bounce.
//! No I/O besides `motion_scale`, so it is testable without a compositor.

pub use crate::shared::anim::{Spring, motion_scale};

/// How much an icon whose centre is `distance_px` from the pointer is
/// magnified: `peak` under the pointer, half of the way to it (so +25% for a
/// +50% peak) at one icon pitch, back to 1.0 two pitches away. A cosine in
/// between, as on macOS.
pub fn magnification(distance_px: f32, pitch_px: f32, peak: f32) -> f32 {
    let radius = 2.0 * pitch_px;
    let d = distance_px.abs();
    if radius <= 0.0 || d >= radius {
        return 1.0;
    }
    let k = 0.5 * (1.0 + (std::f32::consts::PI * d / radius).cos());
    1.0 + (peak - 1.0) * k
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
    fn the_icon_under_the_pointer_gets_the_peak_and_a_neighbour_half_of_it() {
        assert_eq!(magnification(0.0, 52.0, 1.5), 1.5);
        assert!((magnification(52.0, 52.0, 1.5) - 1.25).abs() < 0.001);
        assert!((magnification(-52.0, 52.0, 1.5) - 1.25).abs() < 0.001);
    }

    #[test]
    fn two_pitches_away_and_beyond_nothing_grows() {
        assert_eq!(magnification(104.0, 52.0, 1.5), 1.0);
        assert_eq!(magnification(500.0, 52.0, 1.5), 1.0);
    }

    #[test]
    fn the_magnification_falls_monotonically_with_distance() {
        let mut prev = magnification(0.0, 52.0, 1.5);
        for d in 1..=104 {
            let m = magnification(d as f32, 52.0, 1.5);
            assert!(m <= prev, "d={d}: {m} > {prev}");
            prev = m;
        }
    }

    #[test]
    fn a_zero_pitch_does_not_divide_by_zero() {
        assert!(magnification(10.0, 0.0, 1.5).is_finite());
        assert_eq!(magnification(10.0, 0.0, 1.5), 1.0);
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
