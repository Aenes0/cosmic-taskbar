// SPDX-License-Identifier: GPL-3.0-only

//! The menu's open/close animation: its spring constants and the slide. The
//! spring itself is shared with the taskbar (`shared::anim`).

pub use crate::shared::anim::{Spring, motion_scale};

/// Spring constants: the opening gives a little overshoot, the closing none.
pub const OPEN: (f32, f32) = (380.0, 26.0);
pub const CLOSE: (f32, f32) = (520.0, 46.0);

/// The menu's offset from its place at spring position `pos` (0 = away,
/// 1 = in place), when `travel` px is the way out of the screen: it slides as
/// one piece, like a drawer, out past the edge of its surface (which ends at
/// the panel), since a menu cannot fade as a whole and fading only its
/// background left the icons and text behind.
pub fn slide_offset(pos: f32, travel: f32) -> f32 {
    (1.0 - pos) * travel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_converges_without_big_overshoot() {
        let mut s = Spring::new(0.0);
        let mut peak = 0.0f32;
        let mut t = 0.0;
        while t < 1.5 {
            s.step(1.0, 1.0 / 120.0, OPEN.0, OPEN.1);
            peak = peak.max(s.pos);
            t += 1.0 / 120.0;
        }
        assert!(s.settled(1.0), "pos={} vel={}", s.pos, s.vel);
        assert!(peak <= 1.1, "peak={peak}");
    }

    #[test]
    fn closing_goes_down_fast_without_going_below_zero_much() {
        let mut s = Spring::new(1.0);
        let mut t = 0.0;
        let mut low = 1.0f32;
        while t < 0.3 {
            s.step(0.0, 1.0 / 120.0, CLOSE.0, CLOSE.1);
            low = low.min(s.pos);
            t += 1.0 / 120.0;
        }
        assert!(s.pos < 0.05, "pos={}", s.pos);
        assert!(low > -0.05, "low={low}");
    }


    #[test]
    fn slide_and_glass_follow_the_position() {
        assert_eq!(slide_offset(1.0, 900.0), 0.0);
        assert_eq!(slide_offset(0.0, 900.0), 900.0);
        assert_eq!(slide_offset(0.5, 900.0), 450.0);
        // A little past its place on the way in: a little the other way.
        assert!(slide_offset(1.05, 900.0) < 0.0);
    }

}
