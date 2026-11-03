// SPDX-License-Identifier: GPL-3.0-only

//! The taskbar's zoom and hop for a panel button with a single icon (the
//! Start button, the workspaces' button): the same springs and the same
//! drawing-only growth, one item. At rest the icon keeps its size; under the
//! pointer it grows as much of the taskbar's half again as fits the button.

use crate::bar::fx::{Fx, PEAK_MAGNIFICATION};
use crate::bar::zoomed::Zoomed;
use cosmic::iced::{Element, Subscription};
use std::time::Duration;

const ID: u32 = 0;
/// A frame while something moves, as the taskbar's.
const FRAME: Duration = Duration::from_millis(16);
/// How far above its middle the icon grows from (see `zoomed::icon_transformation`).
const ANCHOR_SHIFT: f32 = 0.14;

#[derive(Default)]
pub struct ButtonZoom {
    fx: Fx,
}

impl ButtonZoom {
    pub fn hover(&mut self, entering: bool) {
        self.fx.set_hover(ID, entering);
    }

    /// A click: one hop.
    pub fn press(&mut self) {
        self.fx.launch(ID);
    }

    pub fn tick(&mut self) {
        // "Has windows": a button hops once, it does not wait for an app.
        self.fx.tick(&[(ID, 0.0)], 0.0, &|_| true);
    }

    pub fn animating(&self) -> bool {
        self.fx.animating()
    }

    /// Frames, only while the icon moves; the caller maps them to its tick
    /// (iced takes no capturing closure in a subscription's map).
    pub fn frames(&self) -> Subscription<std::time::Instant> {
        if self.animating() {
            cosmic::iced::time::every(FRAME)
        } else {
            Subscription::none()
        }
    }

    /// The icon, `icon_px` tall, drawn grown and lifted in a button `cell_px` tall.
    pub fn view<'a, M: 'a>(
        &self,
        icon: impl Into<Element<'a, M, cosmic::Theme, cosmic::Renderer>>,
        icon_px: f32,
        cell_px: f32,
    ) -> Element<'a, M, cosmic::Theme, cosmic::Renderer> {
        let scale = drawn_scale(self.fx.scale_of(ID), icon_px, cell_px);
        let rise = self.fx.rise_of(ID, icon_px, cell_px, true);
        Zoomed::new(icon, scale, rise).into()
    }
}

/// The growth to draw for the springs' `fx_scale` (1 at rest, the taskbar's
/// peak under the pointer): the same share of the largest growth whose top
/// still fits in the button, never more than the taskbar's.
pub fn drawn_scale(fx_scale: f32, icon_px: f32, cell_px: f32) -> f32 {
    if icon_px <= 0.0 {
        return 1.0;
    }
    let room = 1.0 + (cell_px - 1.0 - icon_px) / ((1.0 + ANCHOR_SHIFT) * icon_px);
    let peak = room.clamp(1.0, PEAK_MAGNIFICATION);
    1.0 + (fx_scale - 1.0) / (PEAK_MAGNIFICATION - 1.0) * (peak - 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bar::zoomed::icon_transformation;
    use cosmic::iced::{Point, Rectangle, Size};

    fn run(zoom: &mut ButtonZoom, ms: u32) {
        for _ in 0..(ms / 8) {
            zoom.tick();
            std::thread::sleep(Duration::from_millis(8));
        }
    }

    #[test]
    fn at_rest_the_icon_keeps_its_size() {
        assert_eq!(drawn_scale(1.0, 20.0, 40.0), 1.0);
    }

    #[test]
    fn with_room_it_grows_as_the_taskbar_s_icon_under_the_pointer() {
        assert!((drawn_scale(PEAK_MAGNIFICATION, 20.0, 48.0) - PEAK_MAGNIFICATION).abs() < 0.001);
    }

    #[test]
    fn the_grown_icon_never_leaves_the_button() {
        for (icon, cell) in [(16.0, 40.0), (20.0, 40.0), (24.0, 40.0), (32.0, 40.0), (36.0, 64.0)] {
            let scale = drawn_scale(PEAK_MAGNIFICATION, icon, cell);
            let top_margin = (cell - icon) / 2.0;
            let t = icon_transformation(0.0, cell / 2.0, icon / 2.0, scale, 0.0);
            let top = (Rectangle::new(Point::new(0.0, top_margin), Size::new(0.0, 0.0)) * t).y;
            assert!(top >= 0.0, "icon {icon} cell {cell}: top {top}");
            assert!(scale >= 1.0);
        }
    }

    #[test]
    fn an_icon_as_tall_as_its_button_does_not_grow() {
        assert_eq!(drawn_scale(PEAK_MAGNIFICATION, 40.0, 40.0), 1.0);
        assert_eq!(drawn_scale(PEAK_MAGNIFICATION, 0.0, 40.0), 1.0);
    }

    #[test]
    fn hovering_grows_it_leaving_brings_it_back_and_then_no_frames() {
        let mut zoom = ButtonZoom::default();
        assert!(!zoom.animating());
        zoom.hover(true);
        assert!(zoom.animating());
        run(&mut zoom, 700);
        assert!((zoom.fx.scale_of(ID) - PEAK_MAGNIFICATION).abs() < 0.03);
        assert!(!zoom.animating(), "resting under the pointer needs no frames");
        zoom.hover(false);
        run(&mut zoom, 1500);
        assert_eq!(zoom.fx.scale_of(ID), 1.0);
        assert!(!zoom.animating());
    }

    #[test]
    fn a_click_hops_once_and_stops() {
        let mut zoom = ButtonZoom::default();
        zoom.press();
        assert!(zoom.animating());
        run(&mut zoom, 1000);
        assert!(!zoom.animating(), "one hop, not the launching app's repeated ones");
        assert_eq!(zoom.fx.rise_of(ID, 20.0, 40.0, true), 0.0);
    }
}
