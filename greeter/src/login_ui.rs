// SPDX-License-Identifier: GPL-3.0-only

//! Tela de login estilo Windows 11 (variante C): lógica pura e construtores visuais
//! compartilhados por locker e greeter.

/// O que a linha de status única, abaixo do campo de senha, deve mostrar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusLine<'a> {
    Empty,
    Authenticating,
    CapsLock,
    Error(&'a str),
    Info(&'a str),
}

/// Prioridade: Authenticating > CapsLock > Error > Info > Empty.
/// O chamador passa `caps_lock` só quando o campo de senha está visível.
pub fn status_line<'a>(
    authenticating: bool,
    caps_lock: bool,
    error: Option<&'a str>,
    info: Option<&'a str>,
) -> StatusLine<'a> {
    if authenticating {
        StatusLine::Authenticating
    } else if caps_lock {
        StatusLine::CapsLock
    } else if let Some(error) = error {
        StatusLine::Error(error)
    } else if let Some(info) = info {
        StatusLine::Info(info)
    } else {
        StatusLine::Empty
    }
}

/// Tira `:` e espaços do fim do prompt do PAM ("Password: " -> "Password").
pub fn placeholder(prompt: &str) -> &str {
    prompt.trim_end_matches(|c: char| c == ':' || c.is_whitespace())
}

/// Placeholder do campo: no prompt secreto (a senha), o texto traduzido ("Senha");
/// nos outros prompts, o texto do PAM sem o `:` final.
pub fn field_placeholder(prompt: &str, secret: bool) -> std::borrow::Cow<'_, str> {
    if secret {
        std::borrow::Cow::Owned(crate::fl!("password"))
    } else {
        std::borrow::Cow::Borrowed(placeholder(prompt))
    }
}

/// O botão de mostrar a senha só aparece quando o campo tem texto (como no Windows 11).
pub fn show_visibility_toggle(value: &str) -> bool {
    !value.is_empty()
}

// ---------------------------------------------------------------------------------------------
// Construtores visuais da variante C (compartilhados por locker e greeter).
// ---------------------------------------------------------------------------------------------

use std::borrow::Cow;

use cosmic::iced::{self, Alignment, Background, Border, Color, Length, Vector};
use cosmic::{Element, theme, widget};

use crate::motion::Look;
use crate::offset::offset;

/// Opaque dark fallback color painted underneath every lock/login surface so zero
/// desktop pixels can ever leak through while a wallpaper decodes or loads.
pub const OPAQUE_DARK_BG: Color = Color {
    r: 0.05,
    g: 0.05,
    b: 0.07,
    a: 1.0,
};

/// Tamanho máximo do avatar (com o anel); em telas baixas ele encolhe, ver `screen_metrics`.
pub const AVATAR_SIZE: f32 = 140.0;
pub const FIELD_WIDTH: f32 = 300.0;
pub const SUBMIT_SIZE: f32 = 32.0;
pub const EDGE_PADDING: f32 = 24.0;

/// Anel branco fino em volta do avatar, como no Windows 11.
const AVATAR_RING: f32 = 3.0;
/// Fração da altura da tela: avatar (16%), topo do relógio (10%), tamanho da hora (11%).
const AVATAR_SCREEN_FRACTION: f32 = 0.16;
const CLOCK_TOP_FRACTION: f32 = 0.10;
const TIME_SCREEN_FRACTION: f32 = 0.11;
const TIME_MAX_SIZE: f32 = 88.0;
pub(crate) const DATE_SIZE: f32 = 19.0;
const NAME_SIZE: f32 = 27.0;
const NAME_PADDING: u16 = 14;
const STATUS_SIZE: f32 = 13.0;
const STATUS_TOP_PADDING: u16 = 10;
/// Altura de linha estimada (a medida nos prints fica abaixo de 1,4 × o tamanho da fonte).
const LINE_HEIGHT_ESTIMATE: f32 = 1.4;
/// Folga mínima entre a data e o bloco central: 5% da altura (40 px em 800 px).
const MIN_GAP_FRACTION: f32 = 0.05;
/// Limites inferiores quando a tela é tão baixa que é preciso encolher.
const AVATAR_MIN_SIZE: f32 = 48.0;
pub(crate) const TIME_MIN_SIZE: f32 = 24.0;
/// Altura usada quando o tamanho do monitor é desconhecido (mesmo fallback de `menu_rect`).
const FALLBACK_SCREEN_HEIGHT: f32 = 1080.0;
const TEXT_FIELD_PADDING: u16 = 10;
pub(crate) const fn white(a: f32) -> Color {
    Color { a, ..Color::WHITE }
}

pub(crate) const fn black(a: f32) -> Color {
    Color { a, ..Color::BLACK }
}

/// `color` com o alfa multiplicado por `alpha` (fade de um elemento inteiro).
pub fn fade(color: Color, alpha: f32) -> Color {
    Color { a: color.a * alpha, ..color }
}

/// Interpolação linear componente a componente; `t` preso em `[0, 1]`.
pub fn mix(from: Color, to: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let lerp = |a: f32, b: f32| a + (b - a) * t;
    Color {
        r: lerp(from.r, to.r),
        g: lerp(from.g, to.g),
        b: lerp(from.b, to.b),
        a: lerp(from.a, to.a),
    }
}

/// Identidade da linha de status (variante e texto): muda quando o que aparece muda.
pub fn status_key(status: &StatusLine<'_>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::mem::discriminant(status).hash(&mut hasher);
    if let StatusLine::Error(text) | StatusLine::Info(text) = status {
        text.hash(&mut hasher);
    }
    hasher.finish()
}

/// Mensagens de entrada e saída do mouse de um elemento (hover animado).
#[derive(Clone)]
pub struct Hovers<M> {
    pub enter: M,
    pub exit: M,
}

/// Estado visual do campo de senha: aparência, hover do campo, foco e hover da seta (0..=1).
pub struct FieldFx<M> {
    pub look: Look,
    pub hover: f32,
    pub focus: f32,
    pub submit_hover: f32,
    pub field_hovers: Option<Hovers<M>>,
    pub submit_hovers: Option<Hovers<M>>,
}

/// Estado visual do botão ⏻ e do menu dele.
pub struct PowerFx<M> {
    pub look: Look,
    pub hover: f32,
    pub menu: Look,
    pub hovers: Option<Hovers<M>>,
}

/// Estilo de botão com todas as cores multiplicadas por `alpha`.
fn fade_button(mut s: widget::button::Style, alpha: f32) -> widget::button::Style {
    let fade_bg = |b: Background| match b {
        Background::Color(c) => Background::Color(fade(c, alpha)),
        other => other,
    };
    s.background = s.background.map(fade_bg);
    s.overlay = s.overlay.map(fade_bg);
    s.border_color = fade(s.border_color, alpha);
    s.outline_color = fade(s.outline_color, alpha);
    s.icon_color = s.icon_color.map(|c| fade(c, alpha));
    s.text_color = s.text_color.map(|c| fade(c, alpha));
    s
}

/// O estilo `Icon` do tema com fade (o olho do campo some junto com a tela).
fn faded_icon_button(alpha: f32) -> theme::Button {
    use cosmic::widget::button::Catalog;
    theme::Button::Custom {
        active: Box::new(move |focused, t| fade_button(t.active(focused, false, &theme::Button::Icon), alpha)),
        disabled: Box::new(move |t| fade_button(t.disabled(&theme::Button::Icon), alpha)),
        hovered: Box::new(move |focused, t| fade_button(t.hovered(focused, false, &theme::Button::Icon), alpha)),
        pressed: Box::new(move |focused, t| fade_button(t.pressed(focused, false, &theme::Button::Icon), alpha)),
    }
}

pub(crate) fn shift(look: Look) -> Vector {
    Vector::new(look.dx, look.dy)
}

/// Medidas que dependem da altura do monitor (relógio grande e avatar cabem juntos em telas
/// baixas, como a janela aninhada de 800 px).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenMetrics {
    /// Lado do avatar, já com o anel.
    pub avatar: f32,
    /// Espaço acima do relógio, dentro da margem `EDGE_PADDING` (relógio a ~10% do topo).
    pub clock_top: f32,
    /// Tamanho da fonte da hora.
    pub time_size: f32,
}

/// Altura estimada de um texto com sombra (a cópia da sombra desce 1 px).
fn text_height(size: f32) -> f32 {
    size * LINE_HEIGHT_ESTIMATE + 1.0
}

/// Altura estimada do relógio (hora + data).
pub fn clock_height(time_size: f32) -> f32 {
    text_height(time_size) + text_height(DATE_SIZE)
}

/// Altura estimada do bloco central (avatar, nome, campo + linha, status com 2 linhas).
pub fn center_height(avatar: f32) -> f32 {
    avatar
        + text_height(NAME_SIZE)
        + 2.0 * f32::from(NAME_PADDING)
        + SUBMIT_SIZE
        + 2.0
        + f32::from(STATUS_TOP_PADDING)
        + 2.0 * text_height(STATUS_SIZE)
}

/// Folga mínima entre o fim da data e o topo do bloco central.
pub fn min_gap(screen_height: f32) -> f32 {
    (screen_height * MIN_GAP_FRACTION).round()
}

/// Medidas para a altura `screen_height`, com a garantia de que o relógio termina pelo
/// menos `min_gap` acima do bloco central (centralizado na tela). Se não couber, encolhe
/// nesta ordem: espaço acima do relógio, avatar (até 48), hora (até 24).
pub fn screen_metrics(screen_height: Option<f32>) -> ScreenMetrics {
    let h = match screen_height {
        Some(h) if h > 0.0 => h,
        _ => FALLBACK_SCREEN_HEIGHT,
    };
    let mut m = ScreenMetrics {
        avatar: AVATAR_SIZE.min((h * AVATAR_SCREEN_FRACTION).round()),
        clock_top: (h * CLOCK_TOP_FRACTION - EDGE_PADDING).max(0.0).round(),
        time_size: TIME_MAX_SIZE.min((h * TIME_SCREEN_FRACTION).round()),
    };
    let gap = |m: &ScreenMetrics| {
        (h - center_height(m.avatar)) / 2.0 - (EDGE_PADDING + m.clock_top + clock_height(m.time_size))
    };
    while gap(&m) < min_gap(h) {
        if m.clock_top >= 1.0 {
            m.clock_top -= 1.0;
        } else if m.avatar > AVATAR_MIN_SIZE {
            m.avatar -= 1.0;
        } else if m.time_size > TIME_MIN_SIZE {
            m.time_size -= 1.0;
        } else {
            break;
        }
    }
    m
}

/// Avatar redondo de `size` px (com o anel branco fino); sem foto, o boneco padrão.
pub fn avatar<'a, M: 'a>(
    icon: Option<&'a widget::image::Handle>,
    size: f32,
    look: Look,
) -> Element<'a, M> {
    let inner = (size - 2.0 * AVATAR_RING).max(0.0);
    let picture: Element<'a, M> = match icon {
        Some(handle) => widget::image(handle)
            .width(Length::Fixed(inner))
            .height(Length::Fixed(inner))
            .content_fit(iced::ContentFit::Cover)
            .border_radius([inner / 2.0; 4])
            .opacity(look.alpha)
            .into(),
        None => widget::svg(widget::svg::Handle::from_memory(
            include_bytes!("../res/avatar-default.svg").as_slice(),
        ))
        .width(Length::Fixed(inner))
        .height(Length::Fixed(inner))
        .opacity(look.alpha)
        .into(),
    };
    let alpha = look.alpha;
    let ringed = widget::container(picture)
        .padding(AVATAR_RING)
        .class(theme::Container::custom(move |_| widget::container::Style {
            border: Border {
                radius: (size / 2.0).into(),
                width: AVATAR_RING,
                color: fade(white(0.9), alpha),
            },
            ..Default::default()
        }));
    offset(ringed, shift(look)).into()
}

/// Campo de senha no estilo Windows 11: sem ícone à esquerda; o botão de mostrar a senha
/// (`on_visible_toggle`) só aparece quando já há texto. `.password()` quando `secret`.
/// O chamador acrescenta id, `on_input` e `on_submit`.
pub fn password_input<'a, M: Clone + 'static>(
    placeholder: impl Into<Cow<'a, str>>,
    value: &'a str,
    secret: bool,
    on_visible_toggle: M,
    alpha: f32,
) -> widget::TextInput<'a, M> {
    // Altura fixa de SUBMIT_SIZE (igual à seta), com ou sem o botão de mostrar a senha.
    let mut input = widget::text_input(placeholder, value)
        .padding([0, TEXT_FIELD_PADDING])
        .line_height(iced::widget::text::LineHeight::Absolute(SUBMIT_SIZE.into()));
    if secret {
        input = input.password();
    }
    if show_visibility_toggle(value) {
        input = input.trailing_icon(
            widget::button::custom(
                widget::icon::from_name(if secret {
                    "view-reveal-symbolic"
                } else {
                    "view-conceal-symbolic"
                })
                .fallback(Some(widget::icon::IconFallback::Names(vec![
                    "image-red-eye-symbolic".into(),
                ])))
                .size(16)
                .icon(),
            )
            .class(faded_icon_button(alpha))
            .on_press(on_visible_toggle)
            .padding(8)
            .into(),
        );
    }
    input
}

/// Tela inteira, sem cartão (o fundo vem de baixo): relógio no topo e ⏻ no canto inferior
/// direito numa camada; o centro (avatar, nome, campo, status) centralizado na tela toda em
/// outra camada por cima (só os próprios widgets dela capturam o mouse).
pub fn screen<'a, M: 'a>(
    top: Element<'a, M>,
    center: Element<'a, M>,
    bottom_right: Option<Element<'a, M>>,
) -> Element<'a, M> {
    let mut bottom = widget::row![widget::space::horizontal().width(Length::Fill)];
    if let Some(element) = bottom_right {
        bottom = bottom.push(element);
    }
    let chrome = widget::container(widget::column![
        top,
        widget::space::vertical().height(Length::Fill),
        bottom,
    ])
    .padding(EDGE_PADDING)
    .width(Length::Fill)
    .height(Length::Fill);
    let middle = widget::container(center)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);
    widget::container(
        iced::widget::stack![chrome, middle]
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .class(theme::Container::Transparent)
    .into()
}

/// Opaque dark base + wallpaper image + optional dim overlay.
/// Guarantees zero desktop pixels can ever show through while loading or rendering.
pub fn wallpaper_view<'a, M: 'a>(
    image: &'a widget::image::Handle,
    dim_opacity: f32,
) -> Element<'a, M> {
    let bg_base = widget::container(widget::space::horizontal())
        .width(Length::Fill)
        .height(Length::Fill)
        .class(theme::Container::custom(|_| widget::container::Style {
            background: Some(Background::Color(OPAQUE_DARK_BG)),
            ..Default::default()
        }));

    let img = widget::image(image)
        .content_fit(iced::ContentFit::Cover)
        .width(Length::Fill)
        .height(Length::Fill);

    let dim = dim_opacity.clamp(0.0, 0.85);
    if dim > 0.005 {
        let overlay = widget::container(widget::space::horizontal())
            .width(Length::Fill)
            .height(Length::Fill)
            .class(theme::Container::custom(move |_| widget::container::Style {
                background: Some(Background::Color(black(dim))),
                ..Default::default()
            }));
        iced::widget::stack![bg_base, img, overlay]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        iced::widget::stack![bg_base, img]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

/// Clock strings and animation look passed into `styles::view`.
pub struct ClockData {
    pub time: String,
    pub hour: String,
    pub minute: String,
    pub date: String,
    /// Data curta (como "ter., 24 de nov.") dos widgets compactos.
    pub short_date: String,
    pub look: Look,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authenticating_hides_caps_and_error() {
        assert_eq!(status_line(true, true, Some("x"), Some("i")), StatusLine::Authenticating);
    }
    #[test]
    fn caps_lock_wins_over_error() {
        assert_eq!(status_line(false, true, Some("Senha incorreta"), None), StatusLine::CapsLock);
    }
    #[test]
    fn error_shown_when_idle() {
        assert_eq!(status_line(false, false, Some("Senha incorreta"), None), StatusLine::Error("Senha incorreta"));
    }
    #[test]
    fn error_wins_over_info() {
        assert_eq!(status_line(false, false, Some("e"), Some("i")), StatusLine::Error("e"));
    }
    #[test]
    fn info_shown_when_no_error() {
        assert_eq!(status_line(false, false, None, Some("Coloque o dedo no leitor")), StatusLine::Info("Coloque o dedo no leitor"));
    }
    #[test]
    fn empty_when_nothing_to_say() {
        assert_eq!(status_line(false, false, None, None), StatusLine::Empty);
    }
    #[test]
    fn placeholder_strips_pam_colon() {
        assert_eq!(placeholder("Password: "), "Password");
        assert_eq!(placeholder("Senha:"), "Senha");
        assert_eq!(placeholder("Senha"), "Senha");
        assert_eq!(placeholder(""), "");
    }
    #[test]
    fn secret_prompt_uses_translated_placeholder() {
        crate::localize::localizer();
        assert_eq!(field_placeholder("Password: ", true), "Password");
        assert_eq!(field_placeholder("Senha:", true), "Password");
    }
    #[test]
    fn non_secret_prompt_keeps_pam_text() {
        crate::localize::localizer();
        assert_eq!(field_placeholder("Token: ", false), "Token");
        assert_eq!(field_placeholder("login:", false), "login");
    }
    #[test]
    fn visibility_toggle_only_with_text() {
        assert!(!show_visibility_toggle(""));
        assert!(show_visibility_toggle("a"));
    }
    #[test]
    fn metrics_on_1440p_monitor() {
        let m = screen_metrics(Some(1440.0));
        assert_eq!(m.avatar, 140.0);
        assert_eq!(m.clock_top, 120.0);
        assert_eq!(m.time_size, 88.0);
    }
    #[test]
    fn metrics_on_800px_nested_window() {
        let m = screen_metrics(Some(800.0));
        assert_eq!(m.avatar, 128.0);
        // 10% − 24 daria 56; desce para 45 para garantir a folga mínima de 40 px.
        assert_eq!(m.clock_top, 45.0);
        assert_eq!(m.time_size, 88.0);
    }
    /// Propriedade: o relógio nunca encosta no bloco central; sobra pelo menos `min_gap(h)`
    /// (40 px em 800 px de altura, crescendo com a tela).
    #[test]
    fn clock_and_center_never_overlap() {
        for h in [480.0, 600.0, 768.0, 800.0, 1080.0, 1440.0, 2160.0] {
            let m = screen_metrics(Some(h));
            let clock_bottom = EDGE_PADDING + m.clock_top + clock_height(m.time_size);
            let center_top = (h - center_height(m.avatar)) / 2.0;
            assert!(
                center_top - clock_bottom >= min_gap(h),
                "h={h}: clock_bottom={clock_bottom} center_top={center_top} metrics={m:?}"
            );
            assert!(m.avatar > 2.0 * AVATAR_RING && m.time_size > 0.0 && m.clock_top >= 0.0);
        }
        assert_eq!(min_gap(800.0), 40.0);
    }
    #[test]
    fn fade_scales_alpha_only() {
        let c = fade(Color { r: 1.0, g: 0.5, b: 0.0, a: 0.8 }, 0.5);
        assert_eq!(c, Color { r: 1.0, g: 0.5, b: 0.0, a: 0.4 });
        assert_eq!(fade(Color::WHITE, 1.0), Color::WHITE);
    }
    #[test]
    fn mix_interpolates_and_clamps() {
        assert_eq!(mix(Color::BLACK, Color::WHITE, 0.0), Color::BLACK);
        assert_eq!(mix(Color::BLACK, Color::WHITE, 1.0), Color::WHITE);
        assert_eq!(mix(Color::BLACK, Color::WHITE, 2.0), Color::WHITE);
        assert_eq!(mix(Color::BLACK, Color::WHITE, 0.5).r, 0.5);
    }
    #[test]
    fn status_key_changes_with_kind_and_text() {
        let e1 = status_key(&StatusLine::Error("a"));
        assert_eq!(e1, status_key(&StatusLine::Error("a")));
        assert_ne!(e1, status_key(&StatusLine::Error("b")));
        assert_ne!(e1, status_key(&StatusLine::Info("a")));
        assert_ne!(status_key(&StatusLine::CapsLock), status_key(&StatusLine::Empty));
    }
    #[test]
    fn metrics_without_valid_size_use_1080p() {
        let fallback = screen_metrics(Some(1080.0));
        assert_eq!(screen_metrics(None), fallback);
        assert_eq!(screen_metrics(Some(0.0)), fallback);
        assert_eq!(screen_metrics(Some(-50.0)), fallback);
        assert_eq!(screen_metrics(Some(f32::NAN)), fallback);
    }
}
