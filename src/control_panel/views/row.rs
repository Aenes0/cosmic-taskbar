// SPDX-License-Identifier: GPL-3.0-only

//! Vocabulário de controles do Painel de Controle: a linha de ajuste única
//! (rótulo + descrição à esquerda, controle à direita) e seus controles.
//! Medidas e cores do mockup `Linha.dc.html` (direção B), sempre derivadas do tema.

use std::ops::RangeInclusive;
use std::sync::atomic::{AtomicBool, Ordering};

use cosmic::{
    Element,
    iced::{self, Alignment, Background, Color, Length},
    theme,
    widget::{self, button},
};

use crate::central::tiles::icon;
use crate::central::widgets::card;
use crate::control_panel::Message;

/// Preenchimento da linha (vertical, horizontal), medido no mockup `Linha`.
pub(super) const ROW_PADDING: [u16; 2] = [14, 18];
const LABEL_SIZE: f32 = 14.0;
const DESC_SIZE: f32 = 12.5;
const DESC_OPACITY: f32 = 0.68;
/// Distância entre o texto da linha e o controle.
const ROW_GAP: f32 = 20.0;
/// O controle ocupa no máximo isto da linha (≈ 60% da página de 900 px).
const CONTROL_MAX_WIDTH: f32 = 520.0;
/// Alfa do fundo de um tom (pílula, selo).
const TONE_BG_ALPHA: f32 = 0.14;
const SWATCH_SIZE: f32 = 26.0;
const BUTTON_HEIGHT: f32 = 32.0;
const BUTTON_RADIUS: f32 = 8.0;
const CONTROL_RADIUS: f32 = 8.0;
const SELECT_MIN_WIDTH: f32 = 150.0;
const SEG_HEIGHT: f32 = 28.0;
const CARD_RADIUS: f32 = 12.0;
const CHOICE_ICON_BOX: f32 = 34.0;
const CHOICE_DESC_HEIGHT: f32 = 34.0;

/// Tom semântico de uma pílula.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Ok,
    Warn,
    Bad,
    Info,
    Neutral,
}

/// Estilo de um botão de ação.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BtnStyle {
    Primary,
    Normal,
    Danger,
}

/// (fundo a 14%, texto) de um tom, sempre derivados do tema.
pub fn tone_colors(theme: &cosmic::Theme, t: Tone) -> (Color, Color) {
    let cosmic = theme.cosmic();
    let text: Color = match t {
        Tone::Ok => cosmic.success_color().into(),
        Tone::Warn => cosmic.warning_color().into(),
        Tone::Bad => cosmic.destructive_color().into(),
        Tone::Info => cosmic.accent_color().into(),
        Tone::Neutral => cosmic.background(theme.transparent).on.into(),
    };
    let mut bg = text;
    bg.a = TONE_BG_ALPHA;
    (bg, text)
}

// ── Cores e fontes derivadas do tema ─────────────────────────────────────

/// Cor do texto do fundo com alfa `a` (sobreposições neutras: botões, trilhos, contornos).
pub(super) fn on_alpha(theme: &cosmic::Theme, a: f32) -> Color {
    let mut c: Color = theme.cosmic().background(theme.transparent).on.into();
    c.a *= a;
    c
}

pub(super) fn accent(theme: &cosmic::Theme) -> Color {
    theme.cosmic().accent_color().into()
}

pub(super) fn accent_alpha(theme: &cosmic::Theme, a: f32) -> Color {
    Color { a, ..accent(theme) }
}

/// Texto legível sobre o fundo de destaque.
pub(super) fn on_accent(theme: &cosmic::Theme) -> Color {
    theme.cosmic().on_accent_color().into()
}

/// Peso 500 do mockup (entre o normal e o seminegrito).
pub(super) fn medium() -> iced::Font {
    iced::Font { weight: iced::font::Weight::Medium, ..cosmic::font::default() }
}

pub(super) fn dimmed(theme: &cosmic::Theme) -> iced::widget::text::Style {
    iced::widget::text::Style { color: Some(on_alpha(theme, DESC_OPACITY)), ..Default::default() }
}

pub(super) fn soft(theme: &cosmic::Theme) -> iced::widget::text::Style {
    iced::widget::text::Style { color: Some(on_alpha(theme, 0.85)), ..Default::default() }
}

pub(super) fn accent_text(theme: &cosmic::Theme) -> iced::widget::text::Style {
    iced::widget::text::Style { color: Some(accent(theme)), ..Default::default() }
}

/// Linha divisória fina entre as linhas de um cartão.
pub(super) fn hairline<'a>() -> Element<'a, Message> {
    widget::container(widget::Space::new().height(1.0))
        .width(Length::Fill)
        .class(theme::Container::custom(|theme| widget::container::Style {
            background: Some(Background::Color(on_alpha(theme, 0.07))),
            ..Default::default()
        }))
        .into()
}

/// Fundo de cartão do painel (grupo, cartão da Visão geral) com contorno fino.
pub(super) fn card_style(theme: &cosmic::Theme, radius: f32) -> widget::container::Style {
    let c = card(theme.cosmic());
    widget::container::Style {
        background: Some(Background::Color(c.base.into())),
        text_color: Some(c.on.into()),
        icon_color: Some(c.on.into()),
        border: iced::Border { radius: radius.into(), width: 1.0, color: on_alpha(theme, 0.07) },
        ..Default::default()
    }
}

/// Quadrinho de ícone tingido (cabeçalho de página, cartões da Visão geral).
pub(super) fn icon_box<'a>(name: &'a str, size: f32, icon_size: u16, tone: Tone) -> Element<'a, Message> {
    widget::container(widget::icon::icon(icon(name)).size(icon_size))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .class(theme::Container::custom(move |theme| {
            let (_, fg) = tone_colors(theme, tone);
            widget::container::Style {
                background: Some(Background::Color(Color { a: 0.12, ..fg })),
                icon_color: Some(fg),
                text_color: Some(fg),
                border: iced::Border { radius: (size * 0.28).round().into(), ..Default::default() },
                ..Default::default()
            }
        }))
        .into()
}

// ── Estilos de botão ─────────────────────────────────────────────────────

/// Tipos de botão do mockup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Btn {
    /// Ação: destaque cheio, neutro elevado ou perigo tingido.
    Action(BtnStyle),
    /// Opção de um segmentado (`true` = escolhida).
    Seg(bool),
    /// Atalho "Ir para".
    Chip(bool),
    /// Texto em destaque sem fundo ("Abrir ›", "Desfazer").
    Link,
    /// Contorno fino sem fundo (botão de avançado).
    Outline,
    /// Item de lista neutro (ações rápidas).
    Tile,
}

pub(super) fn btn_class(kind: Btn, radius: f32) -> theme::Button {
    let style = move |hovered: bool, disabled: bool, theme: &cosmic::Theme| {
        let (bg, fg, border): (Option<Color>, Color, Option<Color>) = match kind {
            Btn::Action(BtnStyle::Primary) => {
                let mut a = accent(theme);
                if hovered {
                    a = Color { r: (a.r * 1.08).min(1.0), g: (a.g * 1.08).min(1.0), b: (a.b * 1.08).min(1.0), ..a };
                }
                (Some(a), on_accent(theme), None)
            }
            Btn::Action(BtnStyle::Normal) => (
                Some(on_alpha(theme, if hovered { 0.12 } else { 0.08 })),
                on_alpha(theme, 1.0),
                Some(on_alpha(theme, 0.07)),
            ),
            Btn::Action(BtnStyle::Danger) => {
                let (bg, fg) = tone_colors(theme, Tone::Bad);
                (Some(Color { a: if hovered { 0.22 } else { bg.a }, ..bg }), fg, None)
            }
            Btn::Seg(true) => (Some(accent(theme)), on_accent(theme), None),
            Btn::Seg(false) => (
                hovered.then(|| on_alpha(theme, 0.06)),
                on_alpha(theme, 0.85),
                None,
            ),
            Btn::Chip(on) => (
                Some(if on { accent_alpha(theme, 0.14) } else { on_alpha(theme, if hovered { 0.08 } else { 0.04 }) }),
                if on { accent(theme) } else { on_alpha(theme, 0.85) },
                Some(on_alpha(theme, 0.07)),
            ),
            Btn::Link => (hovered.then(|| accent_alpha(theme, 0.10)), accent(theme), None),
            Btn::Outline => (hovered.then(|| on_alpha(theme, 0.04)), on_alpha(theme, 1.0), Some(on_alpha(theme, 0.10))),
            Btn::Tile => (Some(on_alpha(theme, if hovered { 0.09 } else { 0.05 })), on_alpha(theme, 1.0), None),
        };
        let fade = |c: Color| if disabled { Color { a: c.a * 0.45, ..c } } else { c };
        widget::button::Style {
            background: bg.map(|c| Background::Color(fade(c))),
            border_radius: radius.into(),
            border_width: if border.is_some() { 1.0 } else { 0.0 },
            border_color: border.map(fade).unwrap_or(Color::TRANSPARENT),
            text_color: Some(fade(fg)),
            icon_color: Some(fade(fg)),
            ..widget::button::Style::new()
        }
    };
    theme::Button::Custom {
        active: Box::new(move |_, t| style(false, false, t)),
        disabled: Box::new(move |t| style(false, true, t)),
        hovered: Box::new(move |_, t| style(true, false, t)),
        pressed: Box::new(move |_, t| style(true, false, t)),
    }
}

/// Botão de ação com a medida do mockup (altura `height`, raio 8, texto 13 médio).
pub(super) fn action_btn<'a>(
    label: impl Into<std::borrow::Cow<'a, str>>,
    msg: Option<Message>,
    style: BtnStyle,
    height: f32,
) -> Element<'a, Message> {
    let font = if style == BtnStyle::Primary { cosmic::font::semibold() } else { medium() };
    button::custom(
        widget::container(widget::text(label.into()).size(13.0).font(font).wrapping(iced::widget::text::Wrapping::None))
            .height(Length::Fill)
            .align_y(Alignment::Center),
    )
    .height(Length::Fixed(height))
    .padding([0, 14])
    .class(btn_class(Btn::Action(style), BUTTON_RADIUS + (height - BUTTON_HEIGHT) / 2.0))
    .on_press_maybe(msg)
    .into()
}

// ── Peças de texto ───────────────────────────────────────────────────────

fn label_text<'a>(label: &'a str) -> Element<'a, Message> {
    widget::text(label).size(LABEL_SIZE).font(medium()).into()
}

fn desc_text<'a>(desc: &'a str) -> Element<'a, Message> {
    widget::text(desc)
        .size(DESC_SIZE)
        .line_height(iced::widget::text::LineHeight::Absolute(18.0.into()))
        .class(theme::Text::Custom(dimmed))
        .into()
}

fn title_block<'a>(label: &'a str, desc: Option<&'a str>) -> Element<'a, Message> {
    let mut col = widget::column::with_capacity(2).spacing(3).push(label_text(label));
    if let Some(d) = desc {
        col = col.push(desc_text(d));
    }
    col.into()
}

/// Linha base: texto à esquerda (ocupa o resto), controle à direita no tamanho natural (até 60%).
fn shell<'a>(
    label: &'a str,
    desc: Option<&'a str>,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    widget::row::with_children(vec![
        widget::container(title_block(label, desc)).width(Length::Fill).into(),
        widget::container(control)
            .max_width(CONTROL_MAX_WIDTH)
            .align_x(Alignment::End)
            .into(),
    ])
    .spacing(ROW_GAP)
    .align_y(Alignment::Center)
    .padding(ROW_PADDING)
    .into()
}

/// Linha empilhada (controle largo embaixo do texto): controle deslizante, campo de texto.
fn stacked<'a>(head: Element<'a, Message>, desc: Option<&'a str>, body: Element<'a, Message>) -> Element<'a, Message> {
    let mut col = widget::column::with_capacity(3).spacing(6).push(head);
    if let Some(d) = desc {
        col = col.push(desc_text(d));
    }
    col.push(body).padding(ROW_PADDING).width(Length::Fill).into()
}

/// Etiqueta pequena ao lado de um título ("Avançado").
pub(super) fn tag<'a>(text: &'a str, tone: Tone) -> Element<'a, Message> {
    widget::container(widget::text(text).size(11.0).font(cosmic::font::semibold()))
        .padding([1, 7])
        .class(theme::Container::custom(move |theme| {
            let (bg, fg) = tone_colors(theme, tone);
            widget::container::Style {
                background: Some(Background::Color(Color { a: 0.12, ..bg })),
                text_color: Some(fg),
                border: iced::Border { radius: 6.0.into(), ..Default::default() },
                ..Default::default()
            }
        }))
        .into()
}

/// Cartão com título, descrição opcional e linhas separadas por divisória.
pub fn group<'a>(
    title: &'a str,
    desc: Option<&'a str>,
    advanced: bool,
    rows: Vec<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut head = widget::row::with_capacity(2)
        .spacing(8)
        .align_y(Alignment::Center)
        .push(
            widget::text(title)
                .size(13.5)
                .font(cosmic::font::semibold())
                .class(theme::Text::Custom(soft)),
        );
    if advanced {
        head = head.push(tag("Avançado", Tone::Warn));
    }
    let mut header = widget::column::with_capacity(2).spacing(2).push(head);
    if let Some(d) = desc {
        header = header.push(desc_text(d));
    }

    let mut body = widget::column::with_capacity(rows.len() * 2);
    for (i, r) in rows.into_iter().enumerate() {
        if i > 0 {
            body = body.push(hairline());
        }
        body = body.push(r);
    }

    let card_box = widget::container(body)
        .width(Length::Fill)
        .class(theme::Container::custom(|theme| card_style(theme, CARD_RADIUS)));

    widget::column::with_children(vec![
        widget::container(header).padding([0, 4]).into(),
        card_box.into(),
    ])
    .spacing(8)
    .into()
}

pub fn toggle<'a>(
    label: &'a str,
    desc: Option<&'a str>,
    on: bool,
    msg: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message> {
    shell(label, desc, widget::toggler(on).on_toggle(msg).into())
}

/// Largura do seletor: no mínimo 150 px, ou o bastante para a opção mais longa.
fn select_width(options: &[String]) -> f32 {
    let longest = options.iter().map(|o| o.chars().count()).max().unwrap_or(0) as f32;
    (longest * 7.4 + 48.0).max(SELECT_MIN_WIDTH)
}

pub fn select<'a>(
    label: &'a str,
    desc: Option<&'a str>,
    options: &'a [String],
    selected: Option<usize>,
    on: impl Fn(usize) -> Message + Send + Sync + 'static,
) -> Element<'a, Message> {
    let width = select_width(options);
    let dd = widget::dropdown(options, selected, on)
        .width(Length::Fixed(width))
        .padding([8, 10, 8, 12])
        .text_size(13.0);
    let boxed = widget::container(dd).class(theme::Container::custom(|theme| widget::container::Style {
        background: Some(Background::Color(on_alpha(theme, 0.07))),
        border: iced::Border { radius: CONTROL_RADIUS.into(), width: 1.0, color: on_alpha(theme, 0.08) },
        ..Default::default()
    }));
    shell(label, desc, boxed.into())
}

/// Só o controle segmentado (trilho com as opções), para usar fora de uma linha.
pub(super) fn seg_control<'a>(
    options: &[&'a str],
    selected: Option<usize>,
    fill: bool,
    on: impl Fn(usize) -> Option<Message> + 'a,
) -> Element<'a, Message> {
    let mut r = widget::row::with_capacity(options.len()).spacing(2).align_y(Alignment::Center);
    for (i, opt) in options.iter().enumerate() {
        let chosen = selected == Some(i);
        let font = if chosen { cosmic::font::semibold() } else { medium() };
        let mut b = button::custom(
            widget::container(widget::text(*opt).size(12.5).font(font).wrapping(iced::widget::text::Wrapping::None))
                .height(Length::Fill)
                .align_y(Alignment::Center)
                .align_x(Alignment::Center)
                .width(if fill { Length::Fill } else { Length::Shrink }),
        )
        .height(Length::Fixed(SEG_HEIGHT))
        .padding([0, 12])
        .class(btn_class(Btn::Seg(chosen), 7.0))
        .on_press_maybe(on(i));
        if fill {
            b = b.width(Length::Fill);
        }
        r = r.push(b);
    }
    let track = widget::container(r).padding(3).class(theme::Container::custom(|theme| widget::container::Style {
        background: Some(Background::Color(on_alpha(theme, 0.05))),
        border: iced::Border { radius: 9.0.into(), width: 1.0, color: on_alpha(theme, 0.06) },
        ..Default::default()
    }));
    if fill { track.width(Length::Fill).into() } else { track.into() }
}

pub fn segmented<'a>(
    label: &'a str,
    desc: Option<&'a str>,
    options: &[&'a str],
    selected: usize,
    on: impl Fn(usize) -> Message + 'a,
) -> Element<'a, Message> {
    shell(label, desc, seg_control(options, Some(selected), false, move |i| Some(on(i))))
}

/// Selo de valor do controle deslizante (mono, destaque a 10%).
fn value_badge<'a>(shown: String) -> Element<'a, Message> {
    widget::container(widget::text(shown).size(12.5).font(cosmic::font::mono()))
        .padding([2, 8])
        .class(theme::Container::custom(|theme| widget::container::Style {
            background: Some(Background::Color(accent_alpha(theme, 0.10))),
            text_color: Some(accent(theme)),
            border: iced::Border { radius: 6.0.into(), ..Default::default() },
            ..Default::default()
        }))
        .into()
}

pub fn slider<'a>(
    label: &'a str,
    desc: Option<&'a str>,
    range: RangeInclusive<f32>,
    step: f32,
    value: f32,
    shown: String,
    min_max: (String, String),
    on: impl Fn(f32) -> Message + 'a,
    on_release: Option<Message>,
) -> Element<'a, Message> {
    let mut s = widget::slider(range, value, on).step(step).width(Length::Fill);
    if let Some(m) = on_release {
        s = s.on_release(m);
    }
    let head = widget::row::with_children(vec![
        widget::container(label_text(label)).width(Length::Fill).into(),
        value_badge(shown),
    ])
    .spacing(16)
    .align_y(Alignment::Center);
    let ends = widget::row::with_children(vec![
        widget::text(min_max.0).size(11.0).class(theme::Text::Custom(dimmed)).width(Length::Fill).into(),
        widget::text(min_max.1).size(11.0).class(theme::Text::Custom(dimmed)).into(),
    ]);
    let body = widget::column::with_children(vec![s.into(), ends.into()]).spacing(2);
    stacked(head.into(), desc, body.into())
}

pub fn status<'a>(
    label: &'a str,
    desc: Option<&'a str>,
    pill_text: &'a str,
    tone: Tone,
    buttons: Vec<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut r = widget::row::with_capacity(buttons.len() + 1)
        .spacing(8)
        .align_y(Alignment::Center)
        .push(status_pill(pill_text.to_owned(), tone));
    for b in buttons {
        r = r.push(b);
    }
    shell(label, desc, r.into())
}

pub fn kv<'a>(label: &'a str, value: String) -> Element<'a, Message> {
    shell(
        label,
        None,
        widget::text(value)
            .size(12.5)
            .font(cosmic::font::mono())
            .class(theme::Text::Custom(soft))
            .align_x(Alignment::End)
            .into(),
    )
}

/// Campo de texto (36 px, fundo elevado e contorno fino).
fn text_field<'a>(
    placeholder: &'a str,
    value: &'a str,
    on: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    widget::text_input(placeholder, value)
        .on_input(on)
        .size(13.0)
        .padding([8, 12])
        .width(Length::Fill)
        .into()
}

pub fn input<'a>(
    label: &'a str,
    desc: Option<&'a str>,
    placeholder: &'a str,
    value: &'a str,
    on: impl Fn(String) -> Message + 'a,
    action: Option<(&'a str, Message)>,
) -> Element<'a, Message> {
    let mut r = widget::row::with_capacity(2)
        .spacing(8)
        .align_y(Alignment::Center)
        .push(text_field(placeholder, value, on));
    if let Some((text, msg)) = action {
        r = r.push(action_btn(text, Some(msg), BtnStyle::Normal, 36.0));
    }
    stacked(label_text(label), desc, r.into())
}

/// Linha empilhada com um controle largo próprio (ex.: campo de busca + botão).
pub fn wide<'a>(label: &'a str, desc: Option<&'a str>, control: Element<'a, Message>) -> Element<'a, Message> {
    stacked(label_text(label), desc, control)
}

pub fn buttons<'a>(
    label: &'a str,
    desc: Option<&'a str>,
    items: Vec<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut r = widget::row::with_capacity(items.len())
        .spacing(8)
        .align_y(Alignment::Center);
    for b in items {
        r = r.push(b);
    }
    shell(label, desc, r.wrap().vertical_spacing(8).into())
}

/// `busy` do painel no quadro atual; `shell::view` o grava antes de montar a página, para que
/// `button` desabilite as ações de sistema sem cada chamador carregar o `app`.
static VIEW_BUSY: AtomicBool = AtomicBool::new(false);

pub(super) fn set_view_busy(busy: bool) {
    VIEW_BUSY.store(busy, Ordering::Relaxed);
}

/// Mensagem do botão enquanto há ação de fundo: ocupado → `None` (botão desabilitado).
pub fn when_idle(busy: bool, msg: Message) -> Option<Message> {
    if busy { None } else { Some(msg) }
}

/// Mensagens que disparam pkexec/systemctl/terminal ou outra ação de fundo (`begin_busy`).
pub fn is_system_action(msg: &Message) -> bool {
    matches!(
        msg,
        Message::RunAction(..)
            | Message::ApplyPower
            | Message::ApplyOsd
            | Message::OpenInTerminal(..)
            | Message::RunWallpapers
            | Message::RunBravePwaFix
            | Message::GreeterApply
            | Message::EnableUserUnit(..)
            | Message::RestartUserUnit(..)
            | Message::CheckUpdates
            | Message::RunUpdateNow
            | Message::ExportConfig
            | Message::RestartPanel
            | Message::SetPowerProfile(..)
    )
}

pub fn button<'a>(label: &'a str, msg: Message, style: BtnStyle) -> Element<'a, Message> {
    let msg = if is_system_action(&msg) {
        when_idle(VIEW_BUSY.load(Ordering::Relaxed), msg)
    } else {
        Some(msg)
    };
    action_btn(label, msg, style, BUTTON_HEIGHT)
}

/// Pílula de estado com ponto na cor do tom (texto próprio).
pub(super) fn status_pill<'a>(text: String, tone: Tone) -> Element<'a, Message> {
    let dot = widget::container(widget::Space::new().width(6.0).height(6.0)).class(theme::Container::custom(
        move |theme| widget::container::Style {
            background: Some(Background::Color(tone_colors(theme, tone).1)),
            border: iced::Border { radius: 3.0.into(), ..Default::default() },
            ..Default::default()
        },
    ));
    widget::container(
        widget::row::with_children(vec![
            dot.into(),
            widget::text(text).size(12.0).font(cosmic::font::semibold()).into(),
        ])
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .class(theme::Container::custom(move |theme| {
        let (bg, fg) = tone_colors(theme, tone);
        widget::container::Style {
            background: Some(Background::Color(bg)),
            text_color: Some(fg),
            border: iced::Border { radius: 999.0.into(), ..Default::default() },
            ..Default::default()
        }
    }))
    .into()
}

pub fn pill<'a>(text: &'a str, tone: Tone) -> Element<'a, Message> {
    status_pill(text.to_owned(), tone)
}

/// Nota: caixa de informação tingida de destaque, com ícone.
pub fn note<'a>(text: &'a str) -> Element<'a, Message> {
    let inner = widget::row::with_children(vec![
        widget::icon::icon(icon("help-about-symbolic"))
            .size(16)
            .class(theme::Svg::Custom(std::rc::Rc::new(|theme| iced::widget::svg::Style {
                color: Some(accent(theme)),
            })))
            .into(),
        widget::text(text)
            .size(DESC_SIZE)
            .line_height(iced::widget::text::LineHeight::Absolute(18.0.into()))
            .class(theme::Text::Custom(soft))
            .width(Length::Fill)
            .into(),
    ])
    .spacing(10)
    .align_y(Alignment::Start);
    widget::container(
        widget::container(inner)
            .padding([10, 12])
            .width(Length::Fill)
            .class(theme::Container::custom(|theme| widget::container::Style {
                background: Some(Background::Color(accent_alpha(theme, 0.07))),
                border: iced::Border { radius: 10.0.into(), ..Default::default() },
                ..Default::default()
            })),
    )
    .padding(ROW_PADDING)
    .width(Length::Fill)
    .into()
}

pub fn list_item<'a>(
    name: &'a str,
    meta: String,
    pill_info: Option<(&'a str, Tone)>,
    action: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut r = widget::row::with_capacity(3)
        .spacing(12)
        .align_y(Alignment::Center)
        .padding(ROW_PADDING)
        .push(
            widget::column::with_children(vec![
                widget::text(name).size(13.5).font(medium()).into(),
                widget::text(meta).size(12.0).class(theme::Text::Custom(dimmed)).into(),
            ])
            .spacing(2)
            .width(Length::Fill),
        );
    if let Some((text, tone)) = pill_info {
        r = r.push(pill(text, tone));
    }
    if let Some(b) = action {
        r = r.push(b);
    }
    r.into()
}

pub fn swatches<'a>(
    label: &'a str,
    desc: Option<&'a str>,
    items: &[(&'a str, Color)],
    selected: Option<usize>,
    on: impl Fn(usize) -> Message + 'a,
) -> Element<'a, Message> {
    let mut r = widget::row::with_capacity(items.len()).spacing(8).align_y(Alignment::Center);
    for (i, (name, color)) in items.iter().enumerate() {
        let color = *color;
        let chosen = selected == Some(i);
        let dot = widget::container(widget::Space::new().width(SWATCH_SIZE).height(SWATCH_SIZE)).class(
            theme::Container::custom(move |theme| widget::container::Style {
                background: Some(Background::Color(color)),
                border: iced::Border {
                    radius: (SWATCH_SIZE / 2.0).into(),
                    width: if chosen { 0.0 } else { 1.0 },
                    color: on_alpha(theme, 0.18),
                },
                ..Default::default()
            }),
        );
        // Anel de seleção: 2 px de folga e 2 px de destaque em volta da amostra.
        let ring = widget::container(dot).padding(2).class(theme::Container::custom(move |theme| {
            widget::container::Style {
                border: iced::Border {
                    radius: (SWATCH_SIZE / 2.0 + 4.0).into(),
                    width: if chosen { 2.0 } else { 0.0 },
                    color: accent(theme),
                },
                ..Default::default()
            }
        }));
        r = r.push(widget::tooltip(
            button::custom(ring)
                .padding(0)
                .class(theme::Button::Transparent)
                .on_press(on(i)),
            widget::text::caption(*name),
            widget::tooltip::Position::Bottom,
        ));
    }
    shell(label, desc, r.into())
}

/// Uma opção de [`choice_cards`].
pub struct Choice<'a> {
    pub icon: &'a str,
    pub name: &'a str,
    pub desc: &'a str,
    pub selected: bool,
    pub msg: Option<Message>,
}

/// Cartões de escolha lado a lado (planos de energia): quadrinho de ícone, nome e descrição;
/// o escolhido ganha fundo de destaque a 10%, borda de destaque e o quadrinho cheio.
pub fn choice_cards<'a>(items: Vec<Choice<'a>>) -> Element<'a, Message> {
    let mut r = widget::row::with_capacity(items.len()).spacing(10);
    for c in items {
        let sel = c.selected;
        let ico = widget::container(widget::icon::icon(icon(c.icon)).size(18))
            .width(Length::Fixed(CHOICE_ICON_BOX))
            .height(Length::Fixed(CHOICE_ICON_BOX))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .class(theme::Container::custom(move |theme| {
                let (bg, fg) = if sel { (accent(theme), on_accent(theme)) } else { (on_alpha(theme, 0.08), on_alpha(theme, 0.85)) };
                widget::container::Style {
                    background: Some(Background::Color(bg)),
                    icon_color: Some(fg),
                    border: iced::Border { radius: 10.0.into(), ..Default::default() },
                    ..Default::default()
                }
            }));
        let text = widget::column::with_children(vec![
            widget::text(c.name).size(14.0).font(cosmic::font::semibold()).into(),
            widget::text(c.desc)
                .size(12.0)
                .line_height(iced::widget::text::LineHeight::Absolute(17.0.into()))
                .class(theme::Text::Custom(dimmed))
                .height(Length::Fixed(CHOICE_DESC_HEIGHT))
                .into(),
        ])
        .spacing(2);
        let body = widget::column::with_children(vec![ico.into(), text.into()])
            .spacing(10)
            .width(Length::Fill);
        let class = move |hovered: bool, theme: &cosmic::Theme| widget::button::Style {
            background: Some(Background::Color(if sel {
                accent_alpha(theme, 0.10)
            } else {
                on_alpha(theme, if hovered { 0.07 } else { 0.035 })
            })),
            border_radius: CARD_RADIUS.into(),
            border_width: if sel { 2.0 } else { 1.0 },
            border_color: if sel { accent(theme) } else { on_alpha(theme, 0.07) },
            text_color: Some(on_alpha(theme, 1.0)),
            icon_color: Some(on_alpha(theme, 1.0)),
            ..widget::button::Style::new()
        };
        r = r.push(
            button::custom(body)
                .padding(14)
                .width(Length::Fill)
                .class(theme::Button::Custom {
                    active: Box::new(move |_, t| class(false, t)),
                    disabled: Box::new(move |t| class(false, t)),
                    hovered: Box::new(move |_, t| class(true, t)),
                    pressed: Box::new(move |_, t| class(true, t)),
                })
                .on_press_maybe(c.msg),
        );
    }
    widget::container(r).padding(ROW_PADDING).width(Length::Fill).into()
}

/// Um cartão arrastável de [`tile_grid`] (ícones da barra, cartões do monitor).
pub struct DragTile<'a> {
    pub name: &'a str,
    /// Modo atual ("Sempre", "Ativo", "Nunca"...), mostrado na etiqueta.
    pub mode: &'a str,
    pub tone: Tone,
    /// Modo "desligado" (Nunca/Oculto): cartão esmaecido.
    pub off: bool,
    pub dragging_this: bool,
    pub any_dragging: bool,
    pub on_cycle: Message,
    pub on_drag_start: Message,
    pub on_drag_over: Message,
    pub footer: Option<Element<'a, Message>>,
}

/// Cartão compacto do mockup: alça de arrastar, nome e etiqueta do modo; clique alterna o modo.
pub fn drag_tile<'a>(t: DragTile<'a>) -> Element<'a, Message> {
    let off = t.off;
    let dragging_this = t.dragging_this;
    let grip = widget::mouse_area(
        widget::container(widget::icon::icon(icon("view-app-grid-symbolic")).size(12)).padding([2, 2]),
    )
    .on_press(t.on_drag_start)
    .interaction(if dragging_this { iced::mouse::Interaction::Grabbing } else { iced::mouse::Interaction::Grab });
    let tag_el = if off { tag(t.mode, Tone::Neutral) } else { tag(t.mode, t.tone) };
    let body = button::custom(
        widget::row::with_children(vec![
            widget::text(t.name).size(13.0).font(medium()).width(Length::Fill).into(),
            tag_el,
        ])
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding(0)
    .width(Length::Fill)
    .class({
        let style = move |theme: &cosmic::Theme| {
            let fg = on_alpha(theme, if off { 0.62 } else { 1.0 });
            widget::button::Style { text_color: Some(fg), icon_color: Some(fg), ..widget::button::Style::new() }
        };
        theme::Button::Custom {
            active: Box::new(move |_, t| style(t)),
            disabled: Box::new(move |t| style(t)),
            hovered: Box::new(move |_, t| style(t)),
            pressed: Box::new(move |_, t| style(t)),
        }
    })
    .on_press(t.on_cycle);
    let mut col = widget::column::with_capacity(2).spacing(8).push(
        widget::row::with_children(vec![grip.into(), body.into()]).spacing(8).align_y(Alignment::Center),
    );
    if let Some(f) = t.footer {
        col = col.push(f);
    }
    let card_el = widget::container(col).padding(10).width(Length::Fill).class(theme::Container::custom(
        move |theme| {
            let fg = on_alpha(theme, if off { 0.62 } else { 1.0 });
            widget::container::Style {
                background: Some(Background::Color(on_alpha(theme, if off { 0.025 } else { 0.06 }))),
                text_color: Some(fg),
                icon_color: Some(on_alpha(theme, if off { 0.4 } else { 0.55 })),
                border: iced::Border {
                    radius: 10.0.into(),
                    width: if dragging_this { 2.0 } else { 1.0 },
                    color: if dragging_this { accent(theme) } else { on_alpha(theme, if off { 0.05 } else { 0.07 }) },
                },
                ..Default::default()
            }
        },
    ));
    let mut area = widget::mouse_area(card_el).on_enter(t.on_drag_over);
    if t.any_dragging {
        area = area.interaction(iced::mouse::Interaction::Grabbing);
    }
    area.into()
}

/// Grade de cartões com `cols` colunas iguais (gap 8), dentro do preenchimento da linha.
pub fn tile_grid<'a>(tiles: Vec<Element<'a, Message>>, cols: usize) -> Element<'a, Message> {
    let mut col = widget::column::with_capacity(tiles.len() / cols + 1).spacing(8);
    let mut it = tiles.into_iter().peekable();
    while it.peek().is_some() {
        let mut r = widget::row::with_capacity(cols).spacing(8);
        for _ in 0..cols {
            r = match it.next() {
                Some(t) => r.push(widget::container(t).width(Length::FillPortion(1))),
                None => r.push(widget::Space::new().width(Length::FillPortion(1))),
            };
        }
        col = col.push(r);
    }
    widget::container(col).padding(ROW_PADDING).width(Length::Fill).into()
}

/// Legenda de modos (etiquetas lado a lado).
pub fn legend<'a>(items: &[(&'a str, Tone)]) -> Element<'a, Message> {
    widget::row::with_children(items.iter().map(|&(l, t)| tag(l, t)).collect::<Vec<_>>())
        .spacing(6)
        .align_y(Alignment::Center)
        .into()
}

#[cfg(test)]
mod tests {
    #[test]
    fn when_idle_desabilita_enquanto_ocupado() {
        assert!(super::when_idle(true, Message::ApplyPower).is_none());
        assert!(super::when_idle(false, Message::ApplyPower).is_some());
    }

    #[test]
    fn acoes_de_sistema_sao_reconhecidas() {
        assert!(super::is_system_action(&Message::RunAction("x", String::new(), true)));
        assert!(super::is_system_action(&Message::ApplyOsd));
        assert!(!super::is_system_action(&Message::RefreshAll));
    }

    use super::*;

    #[test]
    fn tone_colors_usa_cores_do_tema() {
        let theme = cosmic::Theme::dark();
        let c = theme.cosmic();
        let casos: [(Tone, Color); 4] = [
            (Tone::Ok, c.success_color().into()),
            (Tone::Warn, c.warning_color().into()),
            (Tone::Bad, c.destructive_color().into()),
            (Tone::Info, c.accent_color().into()),
        ];
        for (tone, esperado) in casos {
            let (bg, fg) = tone_colors(&theme, tone);
            assert_eq!(fg, esperado, "texto de {tone:?}");
            assert_eq!(
                (bg.r, bg.g, bg.b),
                (esperado.r, esperado.g, esperado.b),
                "fundo de {tone:?}"
            );
            assert!((bg.a - 0.14).abs() < 1e-6, "alfa do fundo de {tone:?}");
        }
    }

    #[test]
    fn tone_neutral_usa_cor_de_texto_do_tema() {
        let theme = cosmic::Theme::dark();
        let on: Color = theme.cosmic().background(theme.transparent).on.into();
        let (bg, fg) = tone_colors(&theme, Tone::Neutral);
        assert_eq!(fg, on);
        assert!((bg.a - 0.14).abs() < 1e-6);
    }

    #[test]
    fn largura_do_seletor_tem_minimo_e_cresce_com_a_opcao() {
        assert_eq!(select_width(&["Sim".to_owned()]), SELECT_MIN_WIDTH);
        let longa = vec!["Uma opção bem comprida de verdade aqui".to_owned()];
        assert!(select_width(&longa) > SELECT_MIN_WIDTH);
    }
}
