// SPDX-License-Identifier: GPL-3.0-only

//! Paleta de cores de um estilo: tema claro/escuro, contraste WCAG, véu, cores tonais do Android
//! e fontes por estilo.


use std::sync::Mutex;

use cosmic::iced::advanced::graphics::text::font_system;
use cosmic::iced::{Color, Font, font};

use crate::backdrop::{Analysis, Region};
use crate::style_config::StyleConfig;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub text: Color,
    pub text_muted: Color,
    pub text_shadow: Color,
    pub field_fill: Color,
    pub field_text: Color,
    pub field_placeholder: Color,
    pub field_border: Color,
    pub glass_fill: Color,
    pub glass_border: Color,
    pub glass_shadow: Color,
    pub button_fill: Color,
    pub button_icon: Color,
    pub card_fill: Color,
    pub veil: Color,
    pub accent: Option<Color>,
}

/// Contraste mínimo WCAG AA para texto normal.
pub const MIN_CONTRAST: f32 = 4.5;
/// Teto de alpha de qualquer véu/preenchimento elevado por contraste.
pub const MAX_VEIL: f32 = 0.92;

/// Origem do tom do Android quando o papel de parede não foi analisado (lilás do protótipo).
const DEFAULT_TONAL: [f32; 3] = [0.62, 0.45, 0.85];

fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

fn linear_to_srgb(v: f32) -> f32 {
    if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
}

/// Luminância relativa WCAG (0 = preto, 1 = branco); ignora o alpha.
pub fn relative_luminance(c: Color) -> f32 {
    0.2126 * srgb_to_linear(c.r) + 0.7152 * srgb_to_linear(c.g) + 0.0722 * srgb_to_linear(c.b)
}

/// Razão de contraste WCAG entre duas luminâncias: `(max + 0.05) / (min + 0.05)`.
pub fn contrast_ratio(l1: f32, l2: f32) -> f32 {
    (l1.max(l2) + 0.05) / (l1.min(l2) + 0.05)
}

/// Luminância de `over` (com seu alpha) misturado em sRGB sobre um cinza de luminância `bg_lum`.
pub fn composite_luminance(over: Color, bg_lum: f32) -> f32 {
    let gray = linear_to_srgb(bg_lum.clamp(0.0, 1.0));
    let a = over.a.clamp(0.0, 1.0);
    let mix = |c: f32| c * a + gray * (1.0 - a);
    relative_luminance(Color::from_rgb(mix(over.r), mix(over.g), mix(over.b)))
}

/// Luminância assumida quando o fundo ainda não foi analisado.
const UNKNOWN_LUMINANCE: f32 = 0.5;
/// Teto do overlay de escurecimento (o mesmo clamp de `login_ui::wallpaper_view`).
const MAX_DIM: f32 = 0.85;
/// Abaixo disto `wallpaper_view` nem pinta o overlay.
const MIN_DIM: f32 = 0.005;

/// Luminância do fundo na `region` COMO O USUÁRIO A VÊ: a medida da análise (ou 0.5 sem análise)
/// com o overlay preto de alpha `dim` (`dim_opacity`) por cima, igual a `wallpaper_view`. Todo
/// cálculo de véu, campo e tinta legível parte daqui, nunca da luminância crua.
pub fn backdrop_luminance(analysis: Option<&Analysis>, region: Region, dim: f32) -> f32 {
    let lum = analysis.map_or(UNKNOWN_LUMINANCE, |a| a.region_luminance(region));
    let dim = dim.clamp(0.0, MAX_DIM);
    if dim <= MIN_DIM {
        return lum;
    }
    composite_luminance(black(dim), lum)
}

/// Menor alpha em `[floor, MAX_VEIL]` com que `over` deixa `text` com contraste ≥ [`MIN_CONTRAST`]
/// sobre um fundo de luminância `bg_lum`. Nunca devolve menos que `floor`; devolve [`MAX_VEIL`]
/// quando nem o teto basta.
pub fn min_alpha_for_contrast(text: Color, over: Color, bg_lum: f32, floor: f32) -> f32 {
    let text_lum = relative_luminance(text);
    let ok = |alpha: f32| {
        let c = contrast_ratio(text_lum, composite_luminance(Color { a: alpha, ..over }, bg_lum));
        c >= MIN_CONTRAST
    };
    let floor = floor.clamp(0.0, 1.0);
    if ok(floor) {
        return floor;
    }
    if floor >= MAX_VEIL || !ok(MAX_VEIL) {
        return MAX_VEIL.max(floor);
    }
    // O contraste cresce com o alpha (o véu se afasta do texto): bisseção.
    let (mut lo, mut hi) = (floor, MAX_VEIL);
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        if ok(mid) { hi = mid } else { lo = mid }
    }
    hi
}

fn rgb_to_hsl(rgb: [f32; 3]) -> (f32, f32, f32) {
    let [r, g, b] = rgb.map(|v| v.clamp(0.0, 1.0));
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.0;
    let d = max - min;
    if d < 1e-6 {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s, l)
}

fn hsl(h: f32, s: f32, l: f32, a: f32) -> Color {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h.rem_euclid(360.0) / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    Color::from_rgba(r + m, g + m, b + m, a)
}

const fn white(a: f32) -> Color {
    Color::from_rgba(1.0, 1.0, 1.0, a)
}

const fn black(a: f32) -> Color {
    Color::from_rgba(0.0, 0.0, 0.0, a)
}

/// Paleta do estilo `cfg.style` no tema `cfg.theme`. `tonal_source` (RGB médio do papel de parede)
/// só afeta o Android; `accent_color` da config vence qualquer cor derivada.
///
/// `field_fill` é a base: quem desenha eleva o alpha com [`min_alpha_for_contrast`] sobre a
/// luminância medida. `field_placeholder` tem a mesma cor do texto (sem alpha) para que o piso de
/// contraste calculado para o texto valha para ele também.
pub fn palette(cfg: &StyleConfig, tonal_source: Option<[f32; 3]>) -> Palette {
    let accent = cfg.accent_color_rgba();
    let base = if cfg.is_light() { light(&cfg.style) } else { dark(&cfg.style) };
    let palette = Palette { accent, ..base };
    if cfg.style != "android" {
        return palette;
    }
    let source = accent.map(|c| [c.r, c.g, c.b]).or(tonal_source).unwrap_or(DEFAULT_TONAL);
    android_tonal(palette, source, cfg.is_light())
}

fn dark(style: &str) -> Palette {
    // Alpha base do campo por estilo (win11 vem do protótipo: preto 35 %, borda branca 30 %).
    let field_a = match style {
        "macos" => 0.30,
        "ios" | "ipados" => 0.32,
        _ => 0.35,
    };
    Palette {
        text: Color::WHITE,
        text_muted: white(0.72),
        text_shadow: black(0.35),
        field_fill: black(field_a),
        field_text: Color::WHITE,
        field_placeholder: Color::WHITE,
        field_border: white(0.30),
        glass_fill: white(0.14),
        glass_border: white(0.28),
        glass_shadow: black(0.35),
        button_fill: white(0.16),
        button_icon: Color::WHITE,
        card_fill: black(0.30),
        veil: black(0.35),
        accent: None,
    }
}

fn light(style: &str) -> Palette {
    let field_a = match style {
        "win11" => 0.60,
        _ => 0.55,
    };
    let ink = Color::from_rgb(0.07, 0.07, 0.09);
    Palette {
        text: ink,
        text_muted: Color { a: 0.72, ..ink },
        text_shadow: white(0.45),
        field_fill: white(field_a),
        field_text: ink,
        field_placeholder: ink,
        field_border: black(0.25),
        glass_fill: white(0.50),
        glass_border: white(0.70),
        glass_shadow: black(0.18),
        button_fill: white(0.45),
        button_icon: ink,
        card_fill: white(0.55),
        veil: white(0.35),
        accent: None,
    }
}

/// Android 16: campo em pílula tonal clara e botões tonais escuros (claros no tema claro),
/// derivados do tom da `source`; luminosidade presa para garantir o contraste.
fn android_tonal(base: Palette, source: [f32; 3], light: bool) -> Palette {
    let (hue, sat, _) = rgb_to_hsl(source);
    let sat = sat.clamp(0.0, 0.85);
    let tone = |s: f32, l: f32, a: f32| hsl(hue, (sat * s).min(0.85), l, a);
    let (field_l, text_l) = if light { (0.96, 0.12) } else { (0.90, 0.12) };
    let mut p = Palette {
        field_fill: tone(0.7, field_l, 1.0),
        field_text: tone(0.8, text_l, 1.0),
        field_placeholder: tone(0.5, 0.22, 1.0),
        field_border: tone(0.7, 0.80, 0.80),
        ..base
    };
    if light {
        p.button_fill = tone(0.6, 0.86, 0.85);
        p.button_icon = tone(0.8, 0.16, 1.0);
        p.glass_fill = tone(0.6, 0.90, 0.50);
        p.text = tone(0.8, 0.12, 1.0);
    } else {
        p.button_fill = tone(0.5, 0.16, 0.65);
        p.button_icon = tone(0.8, 0.92, 1.0);
        p.glass_fill = tone(0.5, 0.20, 0.50);
        p.text = tone(0.8, 0.92, 1.0);
    }
    p
}

/// Papel do texto na tela, para escolher peso e tamanho de fonte por estilo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontRole {
    Clock,
    Date,
    Body,
    Name,
}

/// Família livre e peso nativo do estilo para um papel (tabela pura, sem consultar o sistema).
fn native_font(style: &str, role: FontRole) -> (&'static str, font::Weight) {
    use font::Weight;
    match (style, role) {
        ("android", FontRole::Clock) => ("Nunito", Weight::Normal),
        ("android", FontRole::Date | FontRole::Name) => ("Nunito", Weight::Medium),
        ("macos", FontRole::Clock) => ("Nunito", Weight::Bold),
        ("macos", FontRole::Date | FontRole::Name) => ("Nunito", Weight::Semibold),
        ("ios" | "ipados", FontRole::Clock) => ("Inter", Weight::ExtraBold),
        ("ios" | "ipados", FontRole::Date | FontRole::Name) => ("Inter", Weight::Semibold),
        (_, FontRole::Clock | FontRole::Name) => ("Inter", Weight::Semibold),
        (_, FontRole::Date) => ("Inter", Weight::Medium),
        ("android" | "macos", FontRole::Body) => ("Nunito", Weight::Normal),
        (_, FontRole::Body) => ("Inter", Weight::Normal),
    }
}

/// `Some(wanted)` só se a família existe; senão `None` (cai na fonte do sistema). Sem isso o
/// cosmic-text troca a fonte ausente por uma de emoji e os dígitos saem quebrados.
fn resolve_family(wanted: &'static str, available: impl Fn(&str) -> bool) -> Option<&'static str> {
    available(wanted).then_some(wanted)
}

/// A família existe no banco de fontes do iced/cosmic-text? Resultado guardado por família: o
/// banco é consultado uma vez, não a cada `view`.
fn system_has_family(name: &str) -> bool {
    static CACHE: Mutex<Vec<(String, bool)>> = Mutex::new(Vec::new());
    let Ok(mut cache) = CACHE.lock() else { return false };
    if let Some((_, found)) = cache.iter().find(|(n, _)| n == name) {
        return *found;
    }
    let found = font_system().read().is_ok_and(|fs| {
        fs.db().faces().any(|f| f.families.iter().any(|(family, _)| family.eq_ignore_ascii_case(name)))
    });
    cache.push((name.to_owned(), found));
    found
}

/// Decisão pura de [`style_font`], com a consulta de famílias injetada.
fn font_for(cfg: &StyleConfig, role: FontRole, available: impl Fn(&str) -> bool) -> Font {
    use font::Weight;
    let (name, native) = native_font(&cfg.style, role);
    let weight = match (role, cfg.font_weight.as_str()) {
        (FontRole::Clock, "light") => Weight::Light,
        (FontRole::Clock, "semibold") => Weight::Semibold,
        (FontRole::Clock, "bold") => Weight::Bold,
        _ => native,
    };
    if cfg.font_family == "system" {
        return match resolve_family(name, available) {
            Some(family) => Font { weight, ..Font::with_name(family) },
            None => cfg.font_with_weight(weight),
        };
    }
    cfg.font_with_weight(weight)
}

/// Fonte do estilo para um papel. Com `font_family = "system"` usa a fonte livre do estilo
/// (win11/ios/ipados: Inter; android/macos: Nunito) se ela estiver instalada, senão a fonte do
/// sistema com o mesmo peso; `sans`/`mono`/`serif` a sobrepõem. `font_weight != "normal"`
/// sobrepõe só o relógio.
pub fn style_font(cfg: &StyleConfig, role: FontRole) -> Font {
    font_for(cfg, role, system_has_family)
}

#[cfg(test)]
mod tests;
