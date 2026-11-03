// SPDX-License-Identifier: GPL-3.0-only

//! A use, in percent, as five dots under an item's icon on the bar, each a
//! fifth: lit from green to red as far as the use goes, the rest faint. The
//! numbers themselves show on hover. Five small rounded boxes: nothing drawn
//! that the bar did not already draw.

use cosmic::iced::{Background, Border, Color, Length};
use cosmic::{Element, theme, widget};

pub const DOTS: usize = 5;

/// How many dots a use lights: a fifth each, the last from 80 to 100 %.
pub fn lit(percent: f32) -> usize {
    if !percent.is_finite() || percent <= 0.0 {
        return 0;
    }
    ((percent / 20.0).ceil() as usize).min(DOTS)
}

/// The color of the dot at `index` (0 to 4), the same wherever the use is:
/// green, yellow-green, yellow, orange, red. Soft on a dark theme, deeper on a
/// light one, as the other colors of the bar.
pub fn color(index: usize, dark: bool) -> Color {
    const DARK: [u32; DOTS] = [0x3DD68C, 0xA6DB4A, 0xFFD23F, 0xFF9F43, 0xFF5D5D];
    const LIGHT: [u32; DOTS] = [0x15924F, 0x6E9B12, 0xB88F00, 0xD2690F, 0xD12C2C];
    let rgb = if dark { DARK } else { LIGHT }[index.min(DOTS - 1)];
    Color::from_rgb8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

/// A dot not lit.
fn faint(dark: bool) -> Color {
    if dark { Color::from_rgba(1.0, 1.0, 1.0, 0.22) } else { Color::from_rgba(0.0, 0.0, 0.0, 0.18) }
}

/// The dots' size and the gap between them for an icon `icon_px` tall.
pub fn metrics(icon_px: u16) -> (f32, f32) {
    let dot = (f32::from(icon_px) / 5.0).round().max(3.0);
    (dot, (dot / 2.0).max(1.0))
}

/// The five dots of `percent`.
pub fn view<'a, M: 'a>(percent: f32, icon_px: u16, dark: bool) -> Element<'a, M> {
    let (dot, gap) = metrics(icon_px);
    let on = lit(percent);
    let dots = (0..DOTS).map(|index| {
        let fill = if index < on { color(index, dark) } else { faint(dark) };
        widget::container(widget::space::horizontal())
            .width(Length::Fixed(dot))
            .height(Length::Fixed(dot))
            .class(theme::Container::custom(move |_| widget::container::Style {
                background: Some(Background::Color(fill)),
                border: Border { radius: (dot / 2.0).into(), ..Default::default() },
                ..Default::default()
            }))
            .into()
    });
    widget::row::with_children(dots.collect::<Vec<_>>()).spacing(gap).into()
}

/// The thermometer beside an icon, by its setting: `None`, not shown;
/// `Some(true)`, red (past the limit, or lately); `Some(false)`, shown in the
/// bar's own color. Nothing without a temperature to tell.
pub fn thermometer(mode: super::config::BarMode, hot: bool, has_temp: bool) -> Option<bool> {
    use super::config::BarMode;
    if !has_temp {
        return None;
    }
    match mode {
        BarMode::Never => None,
        BarMode::Always => Some(hot),
        BarMode::Over => hot.then_some(true),
    }
}

/// What the hover says: "CPU 95% · 72°".
pub fn usage_tip(name: &str, percent: f32, temp: Option<f32>) -> String {
    let mut tip = format!("{name} {:.0}%", percent.max(0.0));
    if let Some(temp) = temp {
        tip.push_str(&format!(" · {temp:.0}°"));
    }
    tip
}

/// The memory's: "Memória 62% · 19,8 GB" (`used` already written out).
pub fn memory_tip(name: &str, percent: f32, used: &str) -> String {
    format!("{name} {:.0}% · {used}", percent.max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hue in degrees.
    fn hue(c: Color) -> f32 {
        let (max, min) = (c.r.max(c.g).max(c.b), c.r.min(c.g).min(c.b));
        let d = max - min;
        if d == 0.0 {
            return 0.0;
        }
        let h = if max == c.r {
            ((c.g - c.b) / d).rem_euclid(6.0)
        } else if max == c.g {
            (c.b - c.r) / d + 2.0
        } else {
            (c.r - c.g) / d + 4.0
        };
        h * 60.0
    }

    #[test]
    fn each_dot_is_a_fifth_the_last_from_80_to_100() {
        assert_eq!(lit(0.0), 0);
        assert_eq!(lit(0.4), 1);
        assert_eq!(lit(20.0), 1);
        assert_eq!(lit(20.1), 2);
        assert_eq!(lit(40.0), 2);
        assert_eq!(lit(60.5), 4);
        assert_eq!(lit(80.0), 4);
        assert_eq!(lit(80.1), 5);
        assert_eq!(lit(95.0), 5);
        assert_eq!(lit(100.0), 5);
    }

    #[test]
    fn nonsense_lights_nothing_and_too_much_no_more_than_five() {
        assert_eq!(lit(-5.0), 0);
        assert_eq!(lit(f32::NAN), 0);
        assert_eq!(lit(f32::INFINITY), 0);
        assert_eq!(lit(250.0), DOTS);
    }

    #[test]
    fn the_colors_go_from_green_to_red_in_both_themes() {
        for dark in [true, false] {
            let first = color(0, dark);
            let last = color(DOTS - 1, dark);
            assert!(first.g > first.r, "the first is green");
            assert!(last.r > last.g, "the last is red");
            // Dot by dot, the hue goes from green (~150°) down to red (0°).
            for i in 1..DOTS {
                assert!(hue(color(i, dark)) < hue(color(i - 1, dark)), "dot {i}, dark {dark}");
            }
        }
        assert_eq!(color(99, true), color(DOTS - 1, true));
    }

    #[test]
    fn the_dots_fit_under_the_icon_in_every_panel_size() {
        // Icon and padding by panel size (XS..XL): icon, dots and a gap of 3 fit the bar.
        for (icon, padding) in [(16u16, 8u16), (20, 10), (28, 14), (32, 16), (48, 16)] {
            let (dot, gap) = metrics(icon);
            assert!(f32::from(icon) + 3.0 + dot <= f32::from(icon + 2 * padding), "icon {icon}");
            assert!(dot >= 3.0 && gap >= 1.0);
        }
    }

    #[test]
    fn the_thermometer_follows_its_setting() {
        use crate::central::monitor::config::BarMode;
        assert_eq!(thermometer(BarMode::Never, true, true), None);
        assert_eq!(thermometer(BarMode::Always, false, true), Some(false));
        assert_eq!(thermometer(BarMode::Always, true, true), Some(true));
        assert_eq!(thermometer(BarMode::Over, false, true), None);
        assert_eq!(thermometer(BarMode::Over, true, true), Some(true));
        // No temperature (a GPU that says none): nothing to show.
        assert_eq!(thermometer(BarMode::Always, true, false), None);
    }

    #[test]
    fn the_hover_says_the_name_the_use_and_the_temperature() {
        assert_eq!(usage_tip("CPU", 95.2, Some(71.6)), "CPU 95% · 72°");
        assert_eq!(usage_tip("GPU", 3.0, None), "GPU 3%");
        assert_eq!(memory_tip("Memória", 61.7, "19,8 GB"), "Memória 62% · 19,8 GB");
    }
}
