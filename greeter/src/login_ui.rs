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
use cosmic::widget::text_input;
use cosmic::{Element, theme, widget};

use crate::fl;
use crate::motion::Look;
use crate::offset::offset;
use crate::power::PowerAction;
use crate::style_config::StyleConfig;

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

/// Altura mínima da linha de status (uma linha de 13 px): os estados normais não fazem o
/// layout pular; texto longo do PAM quebra em várias linhas dentro de `STATUS_MAX_WIDTH`.
const STATUS_MIN_HEIGHT: f32 = 20.0;
/// Anel branco fino em volta do avatar, como no Windows 11.
const AVATAR_RING: f32 = 3.0;
/// Fração da altura da tela: avatar (16%), topo do relógio (10%), tamanho da hora (11%).
const AVATAR_SCREEN_FRACTION: f32 = 0.16;
const CLOCK_TOP_FRACTION: f32 = 0.10;
const TIME_SCREEN_FRACTION: f32 = 0.11;
const TIME_MAX_SIZE: f32 = 88.0;
const DATE_SIZE: f32 = 19.0;
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
const TIME_MIN_SIZE: f32 = 24.0;
/// Altura usada quando o tamanho do monitor é desconhecido (mesmo fallback de `menu_rect`).
const FALLBACK_SCREEN_HEIGHT: f32 = 1080.0;
const STATUS_MAX_WIDTH: f32 = 320.0;
const POWER_ICON_SIZE: u16 = 24;
const TEXT_FIELD_PADDING: u16 = 10;
const POWER_MENU_WIDTH: f32 = 200.0;
const SHADOW: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.4 };
/// Sombra do texto e dos ícones em duas camadas: preto 22% a 1 px e preto 10% a 2 px.
const SHADOW_NEAR: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.22 };
const SHADOW_FAR: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.10 };
/// Menu ⏻ no padrão do Windows 11, escuro fixo (não segue o tema).
const MENU_BACKGROUND: Color = Color { r: 0.17, g: 0.17, b: 0.17, a: 0.96 };
const MENU_RADIUS: f32 = 8.0;
const MENU_PADDING: u16 = 4;
const MENU_ITEM_HEIGHT: f32 = 36.0;
const MENU_ITEM_PADDING: u16 = 12;
const MENU_ITEM_RADIUS: f32 = 4.0;
const MENU_ICON_SIZE: u16 = 16;
const MENU_ICON_GAP: f32 = 12.0;
const MENU_TEXT_SIZE: f32 = 14.0;
/// Espaço entre o menu ⏻ e o botão; o menu fica alinhado à direita do botão (margem
/// `EDGE_PADDING` da borda da tela, como o flyout do Windows 11).
const MENU_GAP: f32 = 8.0;
const POWER_BUTTON_PADDING: u16 = 10;
/// Lado do botão ⏻: ícone + padding dos dois lados.
const POWER_BUTTON_SIZE: f32 = POWER_ICON_SIZE as f32 + 2.0 * POWER_BUTTON_PADDING as f32;
/// Ícone ⏻ fino próprio (haste de 2 px e anel de 1 px a 22 px), sem depender do tema de ícones.
const POWER_ICON_SVG: &[u8] = include_bytes!("../res/power-thin.svg");

const fn white(a: f32) -> Color {
    Color { a, ..Color::WHITE }
}

const fn black(a: f32) -> Color {
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

fn shift(look: Look) -> Vector {
    Vector::new(look.dx, look.dy)
}

/// Embrulha `element` sempre num `mouse_area` (a forma da árvore não depende dos dados:
/// trocar `Some` por `None` não pode resetar foco e texto do campo); as mensagens de hover só
/// entram quando houver.
fn with_hovers<'a, M: Clone + 'static>(
    element: impl Into<Element<'a, M>>,
    hovers: Option<Hovers<M>>,
) -> Element<'a, M> {
    let area = widget::mouse_area(element);
    match hovers {
        Some(Hovers { enter, exit }) => area.on_enter(enter).on_exit(exit).into(),
        None => area.into(),
    }
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

/// Texto com sombra suave em duas camadas: cópia preta 22% 1 px abaixo (ocupa layout, como
/// a sombra única de antes) e preta 10% 2 px abaixo (só desenho, via `offset`, para o layout
/// não crescer). A camada de 1 px vem primeiro porque a `stack` do iced toma o tamanho da
/// primeira camada. Todas as cores passam por `fade(_, alpha)`.
fn shadow_text<'a, M: 'a>(
    content: Cow<'a, str>,
    size: f32,
    font: Option<cosmic::font::Font>,
    color: Color,
    alpha: f32,
) -> Element<'a, M> {
    let make = |c: Color| {
        let mut t = widget::text(content.clone())
            .size(size)
            .align_x(iced::alignment::Horizontal::Center)
            .class(theme::Text::Color(c));
        if let Some(font) = font {
            t = t.font(font);
        }
        t
    };
    iced::widget::stack![
        widget::container(make(fade(SHADOW_NEAR, alpha))).padding([1, 0, 0, 0]),
        offset(make(fade(SHADOW_FAR, alpha)), Vector::new(0.0, 2.0)),
        make(fade(color, alpha)),
    ]
    .into()
}

/// Relógio grande do topo: hora (semibold) e, embaixo, a data longa (medium).
#[allow(dead_code)]
pub fn clock<'a, M: 'a>(
    time: String,
    date: String,
    metrics: ScreenMetrics,
    look: Look,
) -> Element<'a, M> {
    let clock = widget::container(
        widget::column![
            shadow_text(
                Cow::Owned(time),
                metrics.time_size,
                Some(cosmic::font::semibold()),
                Color::WHITE,
                look.alpha,
            ),
            shadow_text(
                Cow::Owned(date),
                DATE_SIZE,
                Some(cosmic::iced::Font {
                    weight: iced::font::Weight::Medium,
                    ..cosmic::font::default()
                }),
                Color::WHITE,
                look.alpha,
            ),
        ]
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .align_x(Alignment::Center)
    .padding([metrics.clock_top, 0.0, 0.0, 0.0]);
    offset(clock, shift(look)).into()
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

/// Nome completo, grande e branco.
#[allow(dead_code)]
pub fn user_name<'a, M: 'a>(full_name: &'a str, look: Look) -> Element<'a, M> {
    let name = widget::container(shadow_text(
        Cow::Borrowed(full_name),
        NAME_SIZE,
        Some(cosmic::font::bold()),
        Color::WHITE,
        look.alpha,
    ))
    .padding([NAME_PADDING, 0]);
    offset(name, shift(look)).into()
}

/// Fundo escuro e opaco como o campo do Windows 11; `alpha` é o fade do elemento inteiro.
fn field_appearance(background: Color, text_alpha: f32, alpha: f32) -> text_input::Appearance {
    text_input::Appearance {
        background: Background::Color(fade(background, alpha)),
        border_radius: 4.0.into(),
        border_offset: None,
        border_width: 1.0,
        border_color: fade(white(0.2), alpha),
        label_color: fade(Color::WHITE, alpha),
        placeholder_color: fade(white(0.6), alpha),
        selected_text_color: fade(Color::WHITE, alpha),
        icon_color: Some(fade(white(text_alpha), alpha)),
        text_color: Some(fade(white(text_alpha), alpha)),
        selected_fill: fade(white(0.3), alpha),
    }
}

/// Fundo `mix(preto 50%, preto 55%, hover)` em repouso e no hover (a transição vem de fora);
/// borda do foco `mix(branco 20%, destaque, focus)`.
fn field_style(hover: f32, focus: f32, alpha: f32, accent_override: Option<Color>) -> theme::TextInput {
    let background = mix(black(0.5), black(0.55), hover);
    theme::TextInput::Custom {
        active: Box::new(move |_| field_appearance(background, 1.0, alpha)),
        error: Box::new(move |_| field_appearance(black(0.5), 1.0, alpha)),
        hovered: Box::new(move |_| field_appearance(background, 1.0, alpha)),
        focused: Box::new(move |theme| {
            let accent = accent_override.unwrap_or_else(|| theme.cosmic().accent_color().into());
            text_input::Appearance {
                border_color: fade(mix(white(0.2), accent, focus), alpha),
                ..field_appearance(black(0.5), 1.0, alpha)
            }
        }),
        disabled: Box::new(move |_| field_appearance(black(0.5), 0.5, alpha)),
    }
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

/// Seta: fundo `branco(0,12 · hover)` em repouso e no hover (a transição vem de fora).
fn submit_style(hover: f32, alpha: f32) -> theme::Button {
    let style = move |background: f32| {
        let mut s = widget::button::Style::new();
        s.background = (background > 0.0).then_some(Background::Color(fade(white(background), alpha)));
        s.border_radius = [0.0, 3.0, 3.0, 0.0].into();
        s.icon_color = Some(fade(Color::WHITE, alpha));
        s.text_color = Some(fade(Color::WHITE, alpha));
        s
    };
    let rest = 0.12 * hover.clamp(0.0, 1.0);
    theme::Button::Custom {
        active: Box::new(move |_, _| style(rest)),
        disabled: Box::new(move |_| {
            let mut s = style(0.0);
            s.icon_color = Some(fade(white(0.4), alpha));
            s
        }),
        hovered: Box::new(move |_, _| style(rest)),
        pressed: Box::new(move |_, _| style(0.2)),
    }
}

/// Campo com a seta (→) dentro, na borda direita, e a linha de destaque embaixo.
/// O botão de mostrar a senha (`password_input`) fica logo à esquerda da seta: o padding
/// direito do campo reserva o lugar da seta, desenhada por cima numa `stack`.
/// A linha fica fixa na cor de destaque (o `TextInput` não expõe o foco); no foco, a borda
/// do próprio campo também vira cor de destaque.
#[allow(dead_code)]
pub fn password_row<'a, M: Clone + 'static>(
    input: widget::TextInput<'a, M>,
    submit: Option<M>,
    fx: FieldFx<M>,
) -> Element<'a, M> {
    password_row_styled(input, submit, fx, &StyleConfig::default())
}

pub fn password_row_styled<'a, M: Clone + 'static>(
    input: widget::TextInput<'a, M>,
    submit: Option<M>,
    fx: FieldFx<M>,
    cfg: &StyleConfig,
) -> Element<'a, M> {
    let alpha = fx.look.alpha;
    let accent_override = cfg.accent_color_rgba();
    let input = with_hovers(
        input
            .width(Length::Fill)
            .padding([0, SUBMIT_SIZE as u16, 0, TEXT_FIELD_PADDING])
            .style(field_style(fx.hover, fx.focus, alpha, accent_override)),
        fx.field_hovers,
    );
    // A seta fica 1 px para dentro, sem cobrir a borda do campo.
    let arrow_size = SUBMIT_SIZE - 2.0;
    let arrow = widget::tooltip(
        with_hovers(
            widget::button::custom(widget::icon::from_name("go-next-symbolic").size(16).icon())
                .width(Length::Fixed(arrow_size))
                .height(Length::Fixed(arrow_size))
                .padding(7)
                .class(submit_style(fx.submit_hover, alpha))
                .on_press_maybe(submit),
            fx.submit_hovers,
        ),
        widget::text(fl!("sign-in")),
        widget::tooltip::Position::Top,
    );
    let bar = widget::container(widget::space::horizontal())
        .width(Length::Fill)
        .height(Length::Fixed(2.0))
        .class(theme::Container::custom(move |theme| {
            let accent = accent_override.unwrap_or_else(|| theme.cosmic().accent_color().into());
            widget::container::Style {
                background: Some(Background::Color(fade(accent, alpha))),
                ..Default::default()
            }
        }));
    let row = widget::column![
        iced::widget::stack![
            input,
            widget::container(arrow)
                .width(Length::Fill)
                .height(Length::Fixed(SUBMIT_SIZE))
                .padding(1)
                .align_x(Alignment::End)
                .align_y(Alignment::Center),
        ]
        .height(Length::Fixed(SUBMIT_SIZE)),
        bar,
    ]
    .width(Length::Fixed(FIELD_WIDTH));
    offset(row, shift(fx.look)).into()
}

/// Indicador de "autenticando" com as cores do próprio tema, multiplicadas por `alpha`.
fn spinner<'a, M: Clone + 'a>(alpha: f32) -> Element<'a, M> {
    use cosmic::widget::progress_bar::style::Catalog;
    let base = theme::active().style(&Default::default(), false, true);
    widget::indeterminate_circular()
        .size(16.0)
        .bar_height(2.0)
        .track_color(fade(base.track_color, alpha))
        .bar_color(fade(base.bar_color, alpha))
        .into()
}

/// A linha de status: altura mínima de uma linha (o layout não pula nos estados normais);
/// texto longo do PAM quebra dentro de `STATUS_MAX_WIDTH`. Nunca vermelha.
#[allow(dead_code)]
pub fn status_view<'a, M: Clone + 'a>(status: StatusLine<'a>, look: Look) -> Element<'a, M> {
    let color = white(0.9);
    let text = |s: Cow<'a, str>| shadow_text(s, STATUS_SIZE, None, color, look.alpha);
    let content: Element<'a, M> = match status {
        StatusLine::Empty => widget::space::horizontal().into(),
        StatusLine::Authenticating => widget::row![
            spinner(look.alpha),
            text(Cow::Owned(fl!("authenticating"))),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
        StatusLine::CapsLock => text(Cow::Owned(fl!("caps-lock"))),
        StatusLine::Error(s) | StatusLine::Info(s) => text(Cow::Borrowed(s)),
    };
    let line = widget::row![
        widget::space::vertical().height(Length::Fixed(STATUS_MIN_HEIGHT)),
        widget::container(widget::container(content).max_width(STATUS_MAX_WIDTH))
            .width(Length::Fill)
            .align_x(Alignment::Center),
    ]
    .width(Length::Fill)
    .align_y(Alignment::Center);
    offset(line, shift(look)).into()
}

/// Botão ⏻: fundo preto sutil em repouso (contraste em papel de parede claro), claro no
/// hover: `mix(preto 15%, branco 15%, hover)` (a transição vem de fora).
fn power_button_style(hover: f32, alpha: f32) -> theme::Button {
    let style = move |background: Color| {
        let mut s = widget::button::Style::new();
        s.background = Some(Background::Color(fade(background, alpha)));
        s.border_radius = f32::from(POWER_ICON_SIZE).into();
        s.icon_color = Some(fade(Color::WHITE, alpha));
        s.text_color = Some(fade(Color::WHITE, alpha));
        s
    };
    let rest = Color { a: 0.15, ..SHADOW };
    let current = mix(rest, white(0.15), hover);
    theme::Button::Custom {
        active: Box::new(move |_, _| style(current)),
        disabled: Box::new(move |_| style(rest)),
        hovered: Box::new(move |_, _| style(current)),
        pressed: Box::new(move |_, _| style(white(0.25))),
    }
}

/// Ícone ⏻ fino próprio, recolorido para `color`.
fn power_glyph<'a, M: 'a>(size: u16, color: Color) -> Element<'a, M> {
    widget::svg(widget::svg::Handle::from_memory(POWER_ICON_SVG))
        .width(Length::Fixed(f32::from(size)))
        .height(Length::Fixed(f32::from(size)))
        .class(theme::Svg::custom(move |_| iced::widget::svg::Style { color: Some(color) }))
        .into()
}

/// Ícone ⏻ branco com a mesma sombra do texto (duas camadas, ver `shadow_text`).
fn shadow_icon<'a, M: 'a>(size: u16, alpha: f32) -> Element<'a, M> {
    let make = |c: Color| power_glyph::<M>(size, c);
    iced::widget::stack![
        widget::container(make(fade(SHADOW_NEAR, alpha))).padding([1, 0, 0, 0]),
        offset(make(fade(SHADOW_FAR, alpha)), Vector::new(0.0, 2.0)),
        make(fade(Color::WHITE, alpha)),
    ]
    .into()
}

/// Item do menu ⏻ (Windows 11): 36 px de altura, ícone 16 em branco 90%, 12 px até o texto
/// 14 branco; hover branco 6%, pressionado branco 4%.
fn power_menu_item<'a, M: Clone + 'static>(action: PowerAction, on_press: M, alpha: f32) -> Element<'a, M> {
    let icon_color = fade(white(0.9), alpha);
    let style = move |background: Option<Color>| {
        let mut s = widget::button::Style::new();
        s.background = background.map(|c| Background::Color(fade(c, alpha)));
        s.border_radius = MENU_ITEM_RADIUS.into();
        s.icon_color = Some(icon_color);
        s.text_color = Some(fade(Color::WHITE, alpha));
        s
    };
    let icon: Element<'a, M> = match action {
        PowerAction::Shutdown => power_glyph(MENU_ICON_SIZE, icon_color),
        _ => widget::icon::from_name(action.icon_name())
            .size(MENU_ICON_SIZE)
            .icon()
            .class(theme::Svg::custom(move |_| iced::widget::svg::Style { color: Some(icon_color) }))
            .into(),
    };
    let content = widget::row![
        icon,
        widget::space::horizontal().width(Length::Fixed(MENU_ICON_GAP)),
        widget::text(action.label())
            .size(MENU_TEXT_SIZE)
            .class(theme::Text::Color(fade(Color::WHITE, alpha))),
    ]
    .align_y(Alignment::Center)
    .height(Length::Fill);
    widget::button::custom(content)
        .width(Length::Fill)
        .height(Length::Fixed(MENU_ITEM_HEIGHT))
        .padding([0, MENU_ITEM_PADDING])
        .class(theme::Button::Custom {
            active: Box::new(move |_, _| style(None)),
            disabled: Box::new(move |_| style(None)),
            hovered: Box::new(move |_, _| style(Some(white(0.06)))),
            pressed: Box::new(move |_, _| style(Some(white(0.04)))),
        })
        .on_press(on_press)
        .into()
}

/// Menu ⏻ (Windows 11, escuro fixo): fundo cinza 17% a 96%, borda 1 px branco 8%, raio 8,
/// padding 4, largura `POWER_MENU_WIDTH`. `look` dá o fade e a subida ao abrir.
fn power_menu<'a, M: Clone + 'static>(on_action: fn(PowerAction) -> M, look: Look) -> Element<'a, M> {
    let alpha = look.alpha;
    let items: Vec<Element<'a, M>> = PowerAction::ALL
        .into_iter()
        .map(|action| power_menu_item(action, on_action(action), alpha))
        .collect();
    let menu = widget::container(widget::Column::with_children(items))
        .padding(MENU_PADDING)
        .class(theme::Container::custom(move |_| widget::container::Style {
            icon_color: Some(fade(white(0.9), alpha)),
            text_color: Some(fade(Color::WHITE, alpha)),
            background: Some(Background::Color(fade(MENU_BACKGROUND, alpha))),
            border: Border {
                radius: MENU_RADIUS.into(),
                width: 1.0,
                color: fade(white(0.08), alpha),
            },
            ..Default::default()
        }))
        .width(Length::Fixed(POWER_MENU_WIDTH));
    offset(menu, shift(look)).into()
}

/// Botão ⏻ do canto inferior direito com o menu Suspender, Reiniciar, Desligar acima dele.
pub fn power_button<'a, M: Clone + 'static>(
    open: bool,
    on_toggle: M,
    on_action: fn(PowerAction) -> M,
    fx: PowerFx<M>,
) -> Element<'a, M> {
    let alpha = fx.look.alpha;
    let button = widget::tooltip(
        with_hovers(
            widget::button::custom(shadow_icon(POWER_ICON_SIZE, alpha))
                .padding(POWER_BUTTON_PADDING)
                .class(power_button_style(fx.hover, alpha))
                .on_press(on_toggle.clone()),
            fx.hovers,
        ),
        widget::text(fl!("power")),
        widget::tooltip::Position::Top,
    );
    let mut popover = widget::popover(button)
        // Menu alinhado à direita do botão, `MENU_GAP` acima dele (o popover vira o menu para
        // cima quando ele não cabe abaixo do ponto, que é sempre o caso no canto inferior).
        .position(widget::popover::Position::Point(iced::Point::new(
            POWER_BUTTON_SIZE - POWER_MENU_WIDTH,
            -MENU_GAP,
        )))
        .on_close(on_toggle);
    if open {
        popover = popover.popup(power_menu(on_action, fx.menu));
    }
    offset(popover, shift(fx.look)).into()
}

/// Centro da tela: avatar, nome, campo (se houver) e status, centralizados.
#[allow(dead_code)]
pub fn center<'a, M: 'a>(
    avatar: Element<'a, M>,
    name: Element<'a, M>,
    field: Option<Element<'a, M>>,
    status: Element<'a, M>,
) -> Element<'a, M> {
    let mut column = widget::column![avatar, name].align_x(Alignment::Center);
    if let Some(field) = field {
        column = column.push(field);
    }
    column
        .push(widget::container(status).padding([STATUS_TOP_PADDING, 0, 0, 0]))
        .into()
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

/// Styled user name respecting `StyleConfig` font family and weight.
pub fn user_name_styled<'a, M: 'a>(
    full_name: &'a str,
    look: Look,
    cfg: &StyleConfig,
) -> Element<'a, M> {
    let font = if cfg.font_weight == "normal" {
        cfg.font_with_weight(iced::font::Weight::Bold)
    } else {
        cfg.font()
    };
    let name = widget::container(shadow_text(
        Cow::Borrowed(full_name),
        NAME_SIZE,
        Some(font),
        Color::WHITE,
        look.alpha,
    ))
    .padding([NAME_PADDING, 0]);
    offset(name, shift(look)).into()
}

/// Styled status line respecting `StyleConfig` font family.
pub fn status_view_styled<'a, M: Clone + 'a>(
    status: StatusLine<'a>,
    look: Look,
    cfg: &StyleConfig,
) -> Element<'a, M> {
    let color = white(0.9);
    let status_font = cfg.font_with_weight(iced::font::Weight::Normal);
    let text = |s: Cow<'a, str>| shadow_text(s, STATUS_SIZE, Some(status_font), color, look.alpha);
    let content: Element<'a, M> = match status {
        StatusLine::Empty => widget::space::horizontal().into(),
        StatusLine::Authenticating => widget::row![
            spinner(look.alpha),
            text(Cow::Owned(fl!("authenticating"))),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
        StatusLine::CapsLock => text(Cow::Owned(fl!("caps-lock"))),
        StatusLine::Error(s) | StatusLine::Info(s) => text(Cow::Borrowed(s)),
    };
    let line = widget::row![
        widget::space::vertical().height(Length::Fixed(STATUS_MIN_HEIGHT)),
        widget::container(widget::container(content).max_width(STATUS_MAX_WIDTH))
            .width(Length::Fill)
            .align_x(Alignment::Center),
    ]
    .width(Length::Fill)
    .align_y(Alignment::Center);
    offset(line, shift(look)).into()
}

/// Clock strings and animation look passed into [`screen_styled`].
pub struct ClockData {
    pub time: String,
    pub hour: String,
    pub minute: String,
    pub date: String,
    pub look: Look,
}

/// Login widgets passed into [`screen_styled`].
pub struct LoginBlock<'a, M> {
    pub avatar: Option<Element<'a, M>>,
    pub name: Option<Element<'a, M>>,
    pub field: Option<Element<'a, M>>,
    pub status: Element<'a, M>,
    pub look: Look,
}

fn shadow_text_aligned<'a, M: 'a>(
    content: Cow<'a, str>,
    size: f32,
    font: Option<cosmic::font::Font>,
    color: Color,
    alpha: f32,
    align: iced::alignment::Horizontal,
) -> Element<'a, M> {
    let make = |c: Color| {
        let mut t = widget::text(content.clone())
            .size(size)
            .align_x(align)
            .class(theme::Text::Color(c));
        if let Some(font) = font {
            t = t.font(font);
        }
        t
    };
    iced::widget::stack![
        widget::container(make(fade(SHADOW_NEAR, alpha))).padding([1, 0, 0, 0]),
        offset(make(fade(SHADOW_FAR, alpha)), Vector::new(0.0, 2.0)),
        make(fade(color, alpha)),
    ]
    .into()
}

fn clock_font(cfg: &StyleConfig) -> cosmic::font::Font {
    if cfg.font_weight == "normal" {
        cfg.font_with_weight(iced::font::Weight::Semibold)
    } else {
        cfg.font()
    }
}

fn date_font(cfg: &StyleConfig) -> cosmic::font::Font {
    cfg.font_with_weight(iced::font::Weight::Medium)
}

fn assemble_login_column<'a, M: 'a>(login: LoginBlock<'a, M>) -> Element<'a, M> {
    let mut column = widget::column![].align_x(Alignment::Center);
    if let Some(avatar) = login.avatar {
        column = column.push(avatar);
    }
    if let Some(name) = login.name {
        column = column.push(name);
    }
    if let Some(field) = login.field {
        column = column.push(field);
    }
    column
        .push(widget::container(login.status).padding([STATUS_TOP_PADDING, 0, 0, 0]))
        .into()
}

fn maybe_glass_card<'a, M: 'a>(
    content: Element<'a, M>,
    cfg: &StyleConfig,
    alpha: f32,
    min_opacity: f32,
    min_radius: f32,
) -> Element<'a, M> {
    let opacity = cfg.card_opacity.max(min_opacity);
    if opacity <= 0.01 {
        return content;
    }
    let radius = cfg.card_radius.max(min_radius);
    let is_light = cfg.is_light();
    let accent = cfg.accent_color_rgba();
    widget::container(content)
        .padding([24, 28])
        .class(theme::Container::custom(move |_| {
            let bg = if is_light {
                Color {
                    r: 0.96,
                    g: 0.96,
                    b: 0.98,
                    a: opacity * alpha,
                }
            } else {
                Color {
                    r: 0.07,
                    g: 0.08,
                    b: 0.11,
                    a: opacity * alpha,
                }
            };
            let border_color = match accent {
                Some(c) => Color { a: 0.42 * alpha, ..c },
                None => fade(white(0.14), alpha),
            };
            widget::container::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    radius: radius.into(),
                    width: 1.0,
                    color: border_color,
                },
                ..Default::default()
            }
        }))
        .into()
}

/// Render the full login/lock screen using one of the 5 configured layout styles:
/// `"win11"`, `"android"`, `"macos"`, `"sidebar"`, or `"editorial"`.
pub fn screen_styled<'a, M: 'a>(
    cfg: &StyleConfig,
    metrics: ScreenMetrics,
    clock_data: ClockData,
    login: LoginBlock<'a, M>,
    power: Option<Element<'a, M>>,
) -> Element<'a, M> {
    let scale = cfg.clock_scale();
    let time_size = (metrics.time_size * scale).clamp(TIME_MIN_SIZE, 156.0);
    let date_size = (DATE_SIZE * (0.75 + 0.25 * scale)).clamp(14.0, 30.0);
    let c_font = clock_font(cfg);
    let d_font = date_font(cfg);
    let login_alpha = login.look.alpha;

    match cfg.style.as_str() {
        "android" => {
            // Giant 2-line stacked clock (HH over MM) + date underneath + Material You pill/card below.
            let stacked_size = (metrics.time_size * 1.25 * scale).clamp(52.0, 156.0);
            let mut clock_col = widget::column![
                shadow_text(
                    Cow::Owned(clock_data.hour),
                    stacked_size,
                    Some(c_font),
                    Color::WHITE,
                    clock_data.look.alpha,
                ),
                shadow_text(
                    Cow::Owned(clock_data.minute),
                    stacked_size,
                    Some(c_font),
                    Color::WHITE,
                    clock_data.look.alpha,
                ),
            ]
            .spacing(0)
            .align_x(Alignment::Center);
            if cfg.show_date {
                clock_col = clock_col.push(
                    widget::container(shadow_text(
                        Cow::Owned(clock_data.date),
                        date_size,
                        Some(d_font),
                        white(0.92),
                        clock_data.look.alpha,
                    ))
                    .padding([8, 0, 0, 0]),
                );
            }
            let top = offset(
                widget::container(clock_col)
                    .width(Length::Fill)
                    .align_x(Alignment::Center)
                    .padding([(metrics.clock_top * 0.5).max(8.0), 0.0, 0.0, 0.0]),
                shift(clock_data.look),
            );
            let login_card = maybe_glass_card(
                assemble_login_column(login),
                cfg,
                login_alpha,
                0.42,
                24.0,
            );
            let mut bottom = widget::row![widget::space::horizontal().width(Length::Fill)];
            if let Some(p) = power {
                bottom = bottom.push(p);
            }
            let body = widget::column![
                top,
                widget::space::vertical().height(Length::Fill),
                widget::container(login_card)
                    .width(Length::Fill)
                    .align_x(Alignment::Center),
                widget::space::vertical().height(Length::Fixed(24.0)),
                bottom,
            ]
            .width(Length::Fill)
            .height(Length::Fill);
            widget::container(body)
                .padding(EDGE_PADDING)
                .width(Length::Fill)
                .height(Length::Fill)
                .class(theme::Container::Transparent)
                .into()
        }
        "macos" => {
            // Clean top clock + compact avatar/name/field anchored near the bottom.
            let mut clock_col = widget::column![shadow_text(
                Cow::Owned(clock_data.time),
                time_size,
                Some(c_font),
                Color::WHITE,
                clock_data.look.alpha,
            )]
            .align_x(Alignment::Center);
            if cfg.show_date {
                clock_col = clock_col.push(shadow_text(
                    Cow::Owned(clock_data.date),
                    date_size,
                    Some(d_font),
                    Color::WHITE,
                    clock_data.look.alpha,
                ));
            }
            let top = offset(
                widget::container(clock_col)
                    .width(Length::Fill)
                    .align_x(Alignment::Center)
                    .padding([(metrics.clock_top * 0.7).max(16.0), 0.0, 0.0, 0.0]),
                shift(clock_data.look),
            );
            let login_box = maybe_glass_card(
                assemble_login_column(login),
                cfg,
                login_alpha,
                0.0,
                cfg.card_radius,
            );
            let bottom_anchor = widget::container(login_box)
                .width(Length::Fill)
                .align_x(Alignment::Center)
                .padding([0.0, 0.0, 36.0, 0.0]);
            let mut power_row = widget::row![widget::space::horizontal().width(Length::Fill)];
            if let Some(p) = power {
                power_row = power_row.push(p);
            }
            let chrome = widget::container(widget::column![
                top,
                widget::space::vertical().height(Length::Fill),
                power_row,
            ])
            .padding(EDGE_PADDING)
            .width(Length::Fill)
            .height(Length::Fill);
            let bottom_layer = widget::container(bottom_anchor)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::End)
                .padding(EDGE_PADDING);
            widget::container(
                iced::widget::stack![chrome, bottom_layer]
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .class(theme::Container::Transparent)
            .into()
        }
        "sidebar" => {
            // Frosted glass vertical panel on the left (360px wide); clean wallpaper on the right.
            let sidebar_time_size = (time_size * 0.72).clamp(36.0, 72.0);
            let mut clock_col = widget::column![shadow_text(
                Cow::Owned(clock_data.time),
                sidebar_time_size,
                Some(c_font),
                Color::WHITE,
                clock_data.look.alpha,
            )]
            .align_x(Alignment::Center);
            if cfg.show_date {
                clock_col = clock_col.push(shadow_text(
                    Cow::Owned(clock_data.date),
                    date_size,
                    Some(d_font),
                    white(0.88),
                    clock_data.look.alpha,
                ));
            }
            let clock_el = offset(clock_col, shift(clock_data.look));
            let login_el = assemble_login_column(login);
            let mut panel_col = widget::column![
                widget::container(clock_el)
                    .width(Length::Fill)
                    .align_x(Alignment::Center)
                    .padding([32, 0, 0, 0]),
                widget::space::vertical().height(Length::Fill),
                widget::container(login_el)
                    .width(Length::Fill)
                    .align_x(Alignment::Center),
                widget::space::vertical().height(Length::Fill),
            ]
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center);
            if let Some(p) = power {
                panel_col = panel_col.push(
                    widget::container(p)
                        .width(Length::Fill)
                        .align_x(Alignment::End),
                );
            }
            let panel_opacity = cfg.card_opacity.max(0.72);
            let is_light = cfg.is_light();
            let accent = cfg.accent_color_rgba();
            let panel = widget::container(panel_col)
                .width(Length::Fixed(360.0))
                .height(Length::Fill)
                .padding(EDGE_PADDING)
                .class(theme::Container::custom(move |_| {
                    let bg = if is_light {
                        Color {
                            r: 0.94,
                            g: 0.95,
                            b: 0.97,
                            a: panel_opacity * login_alpha,
                        }
                    } else {
                        Color {
                            r: 0.06,
                            g: 0.07,
                            b: 0.10,
                            a: panel_opacity * login_alpha,
                        }
                    };
                    let border_color = match accent {
                        Some(c) => Color {
                            a: 0.35 * login_alpha,
                            ..c
                        },
                        None => fade(white(0.12), login_alpha),
                    };
                    widget::container::Style {
                        background: Some(Background::Color(bg)),
                        border: Border {
                            radius: 0.0.into(),
                            width: 1.0,
                            color: border_color,
                        },
                        ..Default::default()
                    }
                }));
            widget::container(
                widget::row![panel, widget::space::horizontal().width(Length::Fill)]
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .class(theme::Container::Transparent)
            .into()
        }
        "editorial" => {
            // Large clock & date anchored bottom-left + floating compact login card bottom-right.
            let ed_time_size = (time_size * 1.15).clamp(48.0, 140.0);
            let mut clock_col = widget::column![shadow_text_aligned(
                Cow::Owned(clock_data.time),
                ed_time_size,
                Some(c_font),
                Color::WHITE,
                clock_data.look.alpha,
                iced::alignment::Horizontal::Left,
            )]
            .align_x(Alignment::Start);
            if cfg.show_date {
                clock_col = clock_col.push(shadow_text_aligned(
                    Cow::Owned(clock_data.date),
                    date_size * 1.1,
                    Some(d_font),
                    white(0.92),
                    clock_data.look.alpha,
                    iced::alignment::Horizontal::Left,
                ));
            }
            let clock_block = offset(clock_col, shift(clock_data.look));
            let login_card = maybe_glass_card(
                assemble_login_column(login),
                cfg,
                login_alpha,
                0.45,
                cfg.card_radius,
            );
            let mut top_row = widget::row![widget::space::horizontal().width(Length::Fill)];
            if let Some(p) = power {
                top_row = top_row.push(p);
            }
            let bottom_row = widget::row![
                widget::container(clock_block).align_y(Alignment::End),
                widget::space::horizontal().width(Length::Fill),
                widget::container(login_card).align_y(Alignment::End),
            ]
            .width(Length::Fill)
            .align_y(Alignment::End);
            widget::container(
                widget::column![
                    top_row,
                    widget::space::vertical().height(Length::Fill),
                    bottom_row,
                ]
                .width(Length::Fill)
                .height(Length::Fill),
            )
            .padding(EDGE_PADDING + 12.0)
            .width(Length::Fill)
            .height(Length::Fill)
            .class(theme::Container::Transparent)
            .into()
        }
        _ => {
            // "win11" (default): classic centered top clock + centered login block + bottom-right power.
            let mut clock_col = widget::column![shadow_text(
                Cow::Owned(clock_data.time),
                time_size,
                Some(c_font),
                Color::WHITE,
                clock_data.look.alpha,
            )]
            .align_x(Alignment::Center);
            if cfg.show_date {
                clock_col = clock_col.push(shadow_text(
                    Cow::Owned(clock_data.date),
                    date_size,
                    Some(d_font),
                    Color::WHITE,
                    clock_data.look.alpha,
                ));
            }
            let top = offset(
                widget::container(clock_col)
                    .width(Length::Fill)
                    .align_x(Alignment::Center)
                    .padding([metrics.clock_top, 0.0, 0.0, 0.0]),
                shift(clock_data.look),
            );
            let center = maybe_glass_card(
                assemble_login_column(login),
                cfg,
                login_alpha,
                0.0,
                cfg.card_radius,
            );
            screen(top.into(), center, power)
        }
    }
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
