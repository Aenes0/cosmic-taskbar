// SPDX-License-Identifier: GPL-3.0-only

//! Animation pieces both the taskbar and the Start menu use: the spring and
//! the switch that turns motion off.

use std::sync::OnceLock;

/// Longest slice a spring integrates in one go; longer frames are split.
const SPRING_SUBSTEP_S: f32 = 1.0 / 120.0;
/// A frame longer than this (a stalled bar) is treated as this long.
const SPRING_MAX_FRAME_S: f32 = 0.1;

#[derive(Debug, Clone, Copy, Default)]
pub struct Spring {
    pub pos: f32,
    pub vel: f32,
}

impl Spring {
    pub fn new(pos: f32) -> Self {
        Self { pos, vel: 0.0 }
    }

    /// Semi-implicit Euler, split into fixed substeps so a long frame stays stable.
    pub fn step(&mut self, target: f32, dt_s: f32, stiffness: f32, damping: f32) {
        let mut left = dt_s.clamp(0.0, SPRING_MAX_FRAME_S);
        while left > 0.0 {
            let dt = left.min(SPRING_SUBSTEP_S);
            let accel = stiffness * (target - self.pos) - damping * self.vel;
            self.vel += accel * dt;
            self.pos += self.vel * dt;
            left -= dt;
        }
    }

    pub fn settled(&self, target: f32) -> bool {
        (self.pos - target).abs() < 0.002 && self.vel.abs() < 0.01
    }

    /// Settled as far as the eye can tell: a card's lift or a fade within 2%
    /// of where it is going. Stops the frames long before `settled` would.
    pub fn settled_loose(&self, target: f32) -> bool {
        (self.pos - target).abs() < 0.02 && self.vel.abs() < 0.2
    }
}

/// 0.0 when motion is off (reduced motion or `animation_duration_ms` = 0), else 1.0.
/// Read once: `COSMETICS_REDUCE_MOTION` and the compositor's `animation_duration_ms`.
pub fn motion_scale() -> f32 {
    static SCALE: OnceLock<f32> = OnceLock::new();
    *SCALE.get_or_init(|| {
        let reduce = std::env::var("COSMETICS_REDUCE_MOTION").ok();
        let duration = std::env::var_os("HOME")
            .map(|home| {
                std::path::Path::new(&home)
                    .join(".config/cosmic/com.system76.CosmicComp/v1/animation_duration_ms")
            })
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| text.trim().trim_start_matches("Some(").trim_end_matches(')').parse().ok());
        motion_scale_from(reduce.as_deref(), duration)
    })
}

fn motion_scale_from(reduce: Option<&str>, duration_ms: Option<u32>) -> f32 {
    if reduce == Some("1") || duration_ms == Some(0) {
        0.0
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spring_converges_without_big_overshoot() {
        let mut s = Spring::new(1.0);
        let mut peak = 1.0f32;
        let mut t = 0.0;
        while t < 2.0 {
            s.step(1.7, 1.0 / 120.0, 400.0, 28.0);
            peak = peak.max(s.pos);
            t += 1.0 / 120.0;
        }
        assert!(s.settled(1.7), "pos={} vel={}", s.pos, s.vel);
        assert!(peak <= 1.7 * 1.08, "peak={peak}");
    }

    #[test]
    fn a_spring_is_loosely_settled_well_before_it_is_settled() {
        let mut s = Spring::new(0.0);
        let (mut loose_at, mut tight_at) = (None, None);
        for frame in 0..600 {
            s.step(1.0, 1.0 / 60.0, 380.0, 24.0);
            if loose_at.is_none() && s.settled_loose(1.0) {
                loose_at = Some(frame);
            }
            if tight_at.is_none() && s.settled(1.0) {
                tight_at = Some(frame);
            }
        }
        let (loose, tight) = (loose_at.unwrap(), tight_at.unwrap());
        assert!(loose < tight, "loose {loose} tight {tight}");
        assert!(loose < 45, "loosely settled after {loose} frames at 60 Hz");
    }

    #[test]
    fn spring_survives_a_long_frame() {
        let mut s = Spring::new(1.0);
        s.step(1.7, 5.0, 400.0, 28.0);
        assert!(s.pos.is_finite() && s.pos < 3.0);
    }

    #[test]
    fn motion_is_off_for_reduced_motion_or_zero_duration() {
        assert_eq!(motion_scale_from(Some("1"), Some(200)), 0.0);
        assert_eq!(motion_scale_from(None, Some(0)), 0.0);
        assert_eq!(motion_scale_from(None, Some(200)), 1.0);
        assert_eq!(motion_scale_from(None, None), 1.0);
        assert_eq!(motion_scale_from(Some("0"), None), 1.0);
    }
}
