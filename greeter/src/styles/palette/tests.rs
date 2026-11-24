// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::backdrop::{Analysis, Region};

const STYLES: [&str; 5] = ["win11", "android", "macos", "ios", "ipados"];
const THEMES: [&str; 2] = ["dark", "light"];

fn cfg(style: &str, theme: &str) -> StyleConfig {
    StyleConfig {
        style: style.to_owned(),
        theme: theme.to_owned(),
        ..StyleConfig::default()
    }
}

fn backdrops() -> impl Iterator<Item = f32> {
    (0..=20).map(|i| i as f32 * 0.05)
}

#[test]
fn contrast_reference_values() {
    assert!((contrast_ratio(1.0, 0.0) - 21.0).abs() < 0.01);
    assert!((relative_luminance(Color::WHITE) - 1.0).abs() < 1e-4);
}

#[test]
fn field_text_readable_on_any_backdrop() {
    for style in STYLES {
        for theme in THEMES {
            let p = palette(&cfg(style, theme), None);
            for bg in backdrops() {
                let a = min_alpha_for_contrast(p.field_text, p.field_fill, bg, p.field_fill.a);
                let fill = Color { a, ..p.field_fill };
                let under = composite_luminance(fill, bg);
                for (name, fg) in [("texto", p.field_text), ("placeholder", p.field_placeholder)] {
                    let c = contrast_ratio(relative_luminance(fg), under);
                    assert!(c >= MIN_CONTRAST, "{style}/{theme} {name} bg={bg}: {c}");
                }
            }
        }
    }
}

#[test]
fn date_readable_after_veil() {
    for style in STYLES {
        for theme in THEMES {
            let p = palette(&cfg(style, theme), None);
            for bg in backdrops() {
                let a = min_alpha_for_contrast(p.text, p.veil, bg, 0.0);
                let veil = Color { a, ..p.veil };
                let c = contrast_ratio(relative_luminance(p.text), composite_luminance(veil, bg));
                assert!(c >= MIN_CONTRAST, "{style}/{theme} bg={bg}: {c}");
            }
        }
    }
}

#[test]
fn android_tonal_field_has_contrast_for_any_wallpaper() {
    let steps = [0.0_f32, 0.25, 0.5, 0.75, 1.0];
    for theme in THEMES {
        for r in steps {
            for g in steps {
                for b in steps {
                    let p = palette(&cfg("android", theme), Some([r, g, b]));
                    assert!((p.field_fill.a - 1.0).abs() < 1e-6, "campo tonal deve ser opaco");
                    let under = relative_luminance(p.field_fill);
                    for fg in [p.field_text, p.field_placeholder] {
                        let c = contrast_ratio(relative_luminance(fg), under);
                        assert!(c >= MIN_CONTRAST, "{theme} tonal=[{r},{g},{b}]: {c}");
                    }
                }
            }
        }
    }
}

#[test]
fn accent_color_wins_over_tonal() {
    let mut c = cfg("android", "dark");
    c.accent_color = "#60A5FA".to_owned();
    let p = palette(&c, Some([0.9, 0.1, 0.1]));
    assert_eq!(p.accent, Some(Color::from_rgb8(0x60, 0xA5, 0xFA)));
    let q = palette(&c, None);
    assert_eq!(q.button_fill, p.button_fill, "accent_color também ancora o tom do Android");
    assert_eq!(palette(&cfg("win11", "dark"), Some([0.9, 0.1, 0.1])).accent, None);
}

#[test]
fn tonal_source_changes_android_only() {
    let a = palette(&cfg("android", "dark"), Some([0.9, 0.1, 0.1]));
    let b = palette(&cfg("android", "dark"), Some([0.1, 0.2, 0.9]));
    assert_ne!(a.field_fill, b.field_fill);
    assert_ne!(a.button_fill, b.button_fill);
    let w1 = palette(&cfg("macos", "dark"), Some([0.9, 0.1, 0.1]));
    let w2 = palette(&cfg("macos", "dark"), Some([0.1, 0.2, 0.9]));
    assert_eq!(w1, w2);
}

#[test]
fn light_theme_changes_whole_palette() {
    for style in STYLES {
        let d = palette(&cfg(style, "dark"), None);
        let l = palette(&cfg(style, "light"), None);
        assert_ne!(d.text, l.text, "{style} text");
        assert_ne!(d.field_fill, l.field_fill, "{style} field_fill");
        assert_ne!(d.glass_fill, l.glass_fill, "{style} glass_fill");
        assert_ne!(d.button_icon, l.button_icon, "{style} button_icon");
        assert_ne!(d.veil, l.veil, "{style} veil");
    }
}

#[test]
fn min_alpha_respects_floor_and_cap() {
    // Texto branco sobre véu preto num fundo branco: precisa de véu, mas cabe abaixo do teto.
    let a = min_alpha_for_contrast(Color::WHITE, Color::BLACK, 1.0, 0.0);
    assert!(a > 0.4 && a < MAX_VEIL, "{a}");
    // O piso nunca é reduzido.
    assert_eq!(min_alpha_for_contrast(Color::WHITE, Color::BLACK, 0.0, 0.3), 0.3);
    // Sem solução (texto = véu): devolve o teto.
    assert_eq!(min_alpha_for_contrast(Color::WHITE, Color::WHITE, 1.0, 0.0), MAX_VEIL);
}

#[test]
fn fonts_per_style() {
    let f = |style: &str, family: &str, weight: &str, role| {
        let mut c = cfg(style, "dark");
        c.font_family = family.to_owned();
        c.font_weight = weight.to_owned();
        font_for(&c, role, |_| true)
    };
    let clock = f("win11", "system", "normal", FontRole::Clock);
    assert_eq!(clock.family, font::Family::Name("Inter"));
    assert_eq!(clock.weight, font::Weight::Semibold);
    assert_eq!(f("android", "system", "normal", FontRole::Date).family, font::Family::Name("Nunito"));
    assert_eq!(f("macos", "system", "normal", FontRole::Clock).family, font::Family::Name("Nunito"));
    assert_eq!(f("ios", "system", "normal", FontRole::Clock).weight, font::Weight::ExtraBold);
    assert_eq!(f("ipados", "system", "normal", FontRole::Clock).weight, font::Weight::ExtraBold);
    assert_eq!(f("ios", "mono", "normal", FontRole::Clock).family, font::Family::Monospace);
    assert_eq!(f("ios", "serif", "normal", FontRole::Name).family, font::Family::Serif);
    assert_eq!(f("ios", "sans", "normal", FontRole::Name).family, font::Family::SansSerif);
    assert_eq!(f("win11", "system", "light", FontRole::Clock).weight, font::Weight::Light);
    let date_native = f("win11", "system", "normal", FontRole::Date).weight;
    assert_eq!(f("win11", "system", "light", FontRole::Date).weight, date_native);
}

#[test]
fn missing_family_falls_back_to_system_font_with_same_weight() {
    assert_eq!(resolve_family("Inter", |n| n == "Inter"), Some("Inter"));
    assert_eq!(resolve_family("Inter", |_| false), None);
    for style in STYLES {
        for role in [FontRole::Clock, FontRole::Date, FontRole::Body, FontRole::Name] {
            let c = cfg(style, "dark");
            let (_, weight) = native_font(style, role);
            assert_eq!(font_for(&c, role, |_| false), c.font_with_weight(weight), "{style} {role:?}");
        }
    }
    let mut c = cfg("ios", "dark");
    c.font_weight = "bold".to_owned();
    assert_eq!(font_for(&c, FontRole::Clock, |_| false), c.font_with_weight(font::Weight::Bold));
    // Só a família pedida importa: Inter presente mas Nunito ausente.
    let mac = cfg("macos", "dark");
    assert_eq!(font_for(&mac, FontRole::Date, |n| n == "Inter"), mac.font_with_weight(font::Weight::Semibold));
}

#[test]
fn style_font_never_names_a_missing_family() {
    // No banco real: ou a família livre existe, ou o resultado é o do sistema.
    let c = cfg("win11", "dark");
    let f = style_font(&c, FontRole::Clock);
    if !system_has_family("Inter") {
        assert_eq!(f, c.font_with_weight(font::Weight::Semibold));
    } else {
        assert_eq!(f.family, font::Family::Name("Inter"));
    }
}

#[test]
fn backdrop_luminance_applies_the_dim_overlay() {
    let region = Region { x0: 0.0, y0: 0.0, x1: 1.0, y1: 1.0 };
    let analysis = Analysis::uniform(0.8, [1.0; 3]);
    // Sem dim (ou abaixo do limiar em que `wallpaper_view` nem pinta o overlay): luminância crua.
    let raw = analysis.region_luminance(region);
    assert!((raw - 0.8).abs() < 1e-3, "{raw}");
    assert_eq!(backdrop_luminance(Some(&analysis), region, 0.0), raw);
    assert_eq!(backdrop_luminance(Some(&analysis), region, 0.004), raw);
    // Preto de alpha `dim` por cima: igual a `composite_luminance`.
    let dimmed = backdrop_luminance(Some(&analysis), region, 0.6);
    assert!((dimmed - composite_luminance(black(0.6), raw)).abs() < 1e-6, "{dimmed}");
    assert!(dimmed < raw * 0.5, "o dim tem de escurecer bastante: {dimmed}");
    // O dim é preso a 0.85, como em `wallpaper_view` (e em `StyleConfig`).
    assert_eq!(
        backdrop_luminance(Some(&analysis), region, 3.0),
        backdrop_luminance(Some(&analysis), region, 0.85)
    );
    // Sem análise: 0.5 também recebe o overlay.
    assert_eq!(backdrop_luminance(None, region, 0.0), 0.5);
    assert!(backdrop_luminance(None, region, 0.6) < 0.5);
}

/// Replica o que cada estilo faz com a luminância medida: véu da data e preenchimento do campo.
#[test]
fn veil_and_field_readable_with_dim_in_every_style() {
    let region = Region { x0: 0.0, y0: 0.0, x1: 1.0, y1: 1.0 };
    for style in STYLES {
        for theme in THEMES {
            let p = palette(&cfg(style, theme), None);
            for dim in [0.0_f32, 0.6, 0.85] {
                for raw in [0.05_f32, 0.95] {
                    let analysis = Analysis::uniform(raw, [raw; 3]);
                    let lum = backdrop_luminance(Some(&analysis), region, dim);
                    // Contraste real: o que está sob o texto é o fundo DEPOIS do overlay preto.
                    let seen = composite_luminance(black(dim), raw);
                    assert!((lum - seen).abs() < 1e-4 || dim <= 0.005, "{style}/{theme} dim={dim}");
                    let veil_a = min_alpha_for_contrast(p.text, p.veil, lum, 0.0);
                    let date = contrast_ratio(
                        relative_luminance(p.text),
                        composite_luminance(Color { a: veil_a, ..p.veil }, seen),
                    );
                    assert!(date >= MIN_CONTRAST - 0.01, "{style}/{theme} dim={dim} raw={raw}: data {date}");
                    let fill_a = min_alpha_for_contrast(p.field_text, p.field_fill, lum, p.field_fill.a);
                    let under = composite_luminance(Color { a: fill_a, ..p.field_fill }, seen);
                    for (name, fg) in [("texto", p.field_text), ("placeholder", p.field_placeholder)] {
                        let c = contrast_ratio(relative_luminance(fg), under);
                        assert!(c >= MIN_CONTRAST - 0.01, "{style}/{theme} dim={dim} raw={raw}: {name} {c}");
                    }
                }
            }
        }
    }
}
