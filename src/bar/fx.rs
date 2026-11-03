// SPDX-License-Identifier: GPL-3.0-only

//! Per-item zoom and click-bounce state of the bar. Pure state and timing:
//! the view reads `scale_of`/`rise_of`, a tick subscription runs only while
//! `animating` says something still moves.

use crate::bar::anim::{Spring, bounce_offset, magnification, motion_scale};
use rustc_hash::FxHashMap;
use std::time::Instant;

const STIFFNESS: f32 = 320.0;
const DAMPING: f32 = 22.0;
/// The icon under the pointer grows by half; the next ones, by a quarter.
const PEAK_MAGNIFICATION: f32 = 1.5;

/// How much of its box an icon is drawn at when at rest, so that the one under
/// the pointer, half again as big, still fits the button's height `cell_px`:
/// 26 of 32 px in the 40 px panel, more as the panel is taller (never more
/// than the box itself, never under half).
pub fn rest_scale(cell_px: f32, icon_px: f32) -> f32 {
    if icon_px <= 0.0 {
        return 1.0;
    }
    ((cell_px - 1.0) / PEAK_MAGNIFICATION / icon_px).clamp(0.5, 1.0)
}

/// How high a hop goes: the room above an icon at rest, about 15% of the button.
pub fn max_rise(cell_px: f32) -> f32 {
    cell_px.max(0.0) * 0.15
}
/// The distance between two icons when there is only one to go by.
const DEFAULT_PITCH_PX: f32 = 52.0;
/// A launching app keeps hopping this long, or until its window shows.
const LAUNCH_HOP_MAX_S: f32 = 8.0;
const HOP_PERIOD_S: f32 = 0.9;
const HOP_LAST_S: f32 = 0.8;

#[derive(Default, Clone)]
pub struct Fx {
    hover: Option<u32>,
    springs: FxHashMap<u32, Spring>,
    /// Where each spring is going, as of the last tick.
    targets: FxHashMap<u32, f32>,
    bounces: FxHashMap<u32, Instant>,
    last_tick: Option<Instant>,
    /// Something changed since the last tick: it needs a frame to be seen.
    dirty: bool,
}

impl Fx {
    pub fn set_hover(&mut self, id: u32, entering: bool) {
        let before = self.hover;
        if entering {
            self.hover = Some(id);
        } else if self.hover == Some(id) {
            self.hover = None;
        }
        self.dirty |= self.hover != before;
    }

    /// The pointer left the whole bar: nothing is hovered, whatever the icons'
    /// own exit events said (the toolkit drops them when the pointer leaves fast).
    pub fn clear_hover(&mut self) {
        self.dirty |= self.hover.take().is_some();
    }

    pub fn launch(&mut self, id: u32) {
        if motion_scale() > 0.0 {
            self.bounces.insert(id, Instant::now());
            self.dirty = true;
        }
    }

    pub fn scale_of(&self, id: u32) -> f32 {
        self.springs.get(&id).map_or(1.0, |s| s.pos)
    }

    /// Upward offset in px of the item's hop right now.
    pub fn rise_of(&self, id: u32, icon_px: f32, cell_px: f32, has_windows: bool) -> f32 {
        let Some(start) = self.bounces.get(&id) else {
            return 0.0;
        };
        hop_rise(start.elapsed().as_secs_f32(), icon_px, cell_px, has_windows)
    }

    /// Whether frames are needed: a change not yet drawn, a hop going, or a
    /// spring still on its way. Not while the pointer merely rests on an icon.
    pub fn animating(&self) -> bool {
        self.dirty
            || !self.bounces.is_empty()
            || self.springs.iter().any(|(id, s)| {
                !s.settled_loose(self.targets.get(id).copied().unwrap_or(1.0))
            })
    }

    /// Advance the springs and drop finished hops. `items`: each app's
    /// centre along the bar; `has_windows`: whether the app is open.
    pub fn tick(&mut self, items: &[(u32, f32)], _icon_px: f32, has_windows: &dyn Fn(u32) -> bool) {
        let now = Instant::now();
        let dt = self.last_tick.map_or(0.0, |t| (now - t).as_secs_f32());
        self.last_tick = Some(now);
        self.dirty = false;
        let motion = motion_scale() > 0.0;
        let hovered = self
            .hover
            .and_then(|h| items.iter().find(|(i, _)| *i == h).map(|(_, x)| *x));
        let pitch = pitch_of(items);
        for (id, x) in items {
            let target = match (motion, hovered) {
                (true, Some(c)) => magnification(x - c, pitch, PEAK_MAGNIFICATION),
                _ => 1.0,
            };
            self.targets.insert(*id, target);
            let spring = self.springs.entry(*id).or_insert_with(|| Spring::new(1.0));
            spring.step(target, dt, STIFFNESS, DAMPING);
            if spring.settled_loose(target) {
                *spring = Spring { pos: target, vel: 0.0 };
            }
        }
        self.bounces.retain(|id, start| {
            let t = start.elapsed().as_secs_f32();
            if has_windows(*id) {
                t < HOP_LAST_S
            } else {
                t < LAUNCH_HOP_MAX_S
            }
        });
        if !self.animating() {
            self.last_tick = None;
        }
    }
}

/// The distance between neighbouring icons: the smallest gap between centres.
fn pitch_of(items: &[(u32, f32)]) -> f32 {
    let mut xs: Vec<f32> = items.iter().map(|(_, x)| *x).collect();
    xs.sort_by(f32::total_cmp);
    xs.windows(2)
        .map(|w| w[1] - w[0])
        .filter(|gap| *gap > 1.0)
        .min_by(f32::total_cmp)
        .unwrap_or(DEFAULT_PITCH_PX)
}

/// Rise at `t_s` since the click: one hop if the window is already up,
/// repeated hops while the app is still opening.
fn hop_rise(t_s: f32, icon_px: f32, cell_px: f32, has_windows: bool) -> f32 {
    let t = if has_windows { t_s } else { t_s % HOP_PERIOD_S };
    // The hop is shaped for half an icon of height; the panel has `max_rise`.
    bounce_offset(t, icon_px) * max_rise(cell_px) / (0.5 * icon_px).max(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ITEMS: [(u32, f32); 4] = [(1, 20.0), (2, 72.0), (3, 124.0), (4, 176.0)];

    fn run(fx: &mut Fx, ms: u32) {
        for _ in 0..(ms / 8) {
            fx.tick(&ITEMS, 32.0, &|_| false);
            std::thread::sleep(std::time::Duration::from_millis(8));
        }
    }

    #[test]
    fn the_hovered_icon_grows_half_again_its_neighbours_a_quarter_and_the_rest_not_at_all() {
        let mut fx = Fx::default();
        fx.set_hover(2, true);
        run(&mut fx, 700);
        assert!((fx.scale_of(2) - 1.5).abs() < 0.03, "hovered {}", fx.scale_of(2));
        assert!((fx.scale_of(1) - 1.25).abs() < 0.03, "left {}", fx.scale_of(1));
        assert!((fx.scale_of(3) - 1.25).abs() < 0.03, "right {}", fx.scale_of(3));
        assert!((fx.scale_of(4) - 1.0).abs() < 0.03, "two away {}", fx.scale_of(4));
    }

    #[test]
    fn it_stops_asking_for_frames_once_settled_even_with_the_pointer_still_there() {
        let mut fx = Fx::default();
        assert!(!fx.animating());
        fx.set_hover(2, true);
        assert!(fx.animating(), "a new hover needs frames");
        run(&mut fx, 900);
        assert!(!fx.animating(), "settled on the icon: no more frames");
        fx.set_hover(2, false);
        assert!(fx.animating(), "leaving needs frames");
        run(&mut fx, 1500);
        assert!(!fx.animating());
        assert_eq!(fx.scale_of(2), 1.0);
    }

    #[test]
    fn the_hop_rises_no_more_than_the_room_above_the_icon() {
        let peak = (0..=900)
            .map(|ms| hop_rise(ms as f32 / 1000.0, 32.0, 40.0, true))
            .fold(0.0f32, f32::max);
        assert!(peak <= max_rise(40.0) + 0.01, "peak {peak}");
        assert!(peak >= max_rise(40.0) * 0.9, "peak {peak}");
        assert_eq!(hop_rise(0.9, 32.0, 40.0, true), 0.0);
    }

    #[test]
    fn an_app_still_opening_keeps_hopping() {
        assert!(hop_rise(0.9 + 0.25, 32.0, 40.0, false) > max_rise(40.0) * 0.9);
    }

    #[test]
    fn a_launch_starts_a_hop_that_needs_frames() {
        let mut fx = Fx::default();
        fx.launch(7);
        assert!(fx.animating());
        assert!(fx.rise_of(7, 32.0, 40.0, false) >= 0.0);
        assert_eq!(fx.rise_of(8, 32.0, 40.0, false), 0.0);
    }

    #[test]
    fn the_pitch_is_the_smallest_gap_and_defaults_with_one_icon() {
        assert_eq!(pitch_of(&[(1, 20.0), (2, 72.0), (3, 124.0)]), 52.0);
        assert_eq!(pitch_of(&[(3, 124.0), (1, 20.0), (2, 100.0)]), 24.0);
        assert_eq!(pitch_of(&[(1, 20.0)]), DEFAULT_PITCH_PX);
        assert_eq!(pitch_of(&[]), DEFAULT_PITCH_PX);
        assert_eq!(pitch_of(&[(1, 20.0), (2, 20.0)]), DEFAULT_PITCH_PX);
    }

    #[test]
    fn the_pointer_leaving_the_bar_lets_every_icon_back_down_even_without_an_exit_event() {
        let mut fx = Fx::default();
        fx.set_hover(2, true);
        run(&mut fx, 700);
        assert!(fx.scale_of(2) > 1.4);
        // No `set_hover(2, false)`: the exit event never came.
        fx.clear_hover();
        assert!(fx.animating(), "it needs frames to come back down");
        run(&mut fx, 1500);
        assert_eq!(fx.scale_of(2), 1.0);
        assert_eq!(fx.scale_of(1), 1.0);
        assert!(!fx.animating());
    }

    #[test]
    fn clearing_with_nothing_hovered_asks_for_no_frames() {
        let mut fx = Fx::default();
        fx.clear_hover();
        assert!(!fx.animating());
    }

    #[test]
    fn the_resting_size_leaves_room_for_the_half_again_zoom_in_any_panel() {
        // The 40 px panel with 32 px icons: they rest at 26 px.
        assert!((rest_scale(40.0, 32.0) - 26.0 / 32.0).abs() < 0.001);
        for (cell, icon) in [(40.0, 32.0), (48.0, 36.0), (56.0, 40.0), (64.0, 48.0), (80.0, 48.0)] {
            let rest = rest_scale(cell, icon) * icon;
            assert!(rest * PEAK_MAGNIFICATION <= cell, "cell {cell} icon {icon}: {}", rest * PEAK_MAGNIFICATION);
            assert!((0.5..=1.0).contains(&rest_scale(cell, icon)));
        }
    }

    #[test]
    fn a_taller_panel_makes_the_icons_rest_bigger_and_the_hop_higher() {
        assert!(rest_scale(64.0, 48.0) * 48.0 > rest_scale(40.0, 32.0) * 32.0);
        assert!(max_rise(64.0) > max_rise(40.0));
        assert_eq!(max_rise(40.0), 6.0);
    }

    #[test]
    fn a_degenerate_panel_does_not_divide_by_zero() {
        assert!((0.5..=1.0).contains(&rest_scale(0.0, 0.0)));
        assert!((0.5..=1.0).contains(&rest_scale(40.0, 0.0)));
    }

    #[test]
    fn a_stale_id_in_the_hover_does_not_panic() {
        let mut fx = Fx::default();
        fx.set_hover(9, true);
        fx.tick(&[(1, 20.0)], 32.0, &|_| false);
        assert_eq!(fx.scale_of(9), 1.0);
    }
}
