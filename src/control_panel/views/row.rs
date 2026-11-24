// SPDX-License-Identifier: GPL-3.0-only

//! Vocabulário de controles do Painel de Controle: a linha de ajuste única
//! (rótulo + descrição à esquerda, controle à direita) e seus controles.

use std::ops::RangeInclusive;
use std::sync::atomic::{AtomicBool, Ordering};

use cosmic::{
    Element,
    iced::{self, Alignment, Background, Color, Length},
    theme,
    widget::{self, button},
};

use crate::central::tiles::chip_class;
use crate::central::widgets::card;
use crate::control_panel::Message;

/// Preenchimento da linha (vertical, horizontal), medido no mockup `Linha`.
const ROW_PADDING: [u16; 2] = [14, 18];
const LABEL_SIZE: f32 = 14.0;
const DESC_SIZE: f32 = 12.5;
const DESC_OPACITY: f32 = 0.65;
/// O controle ocupa até 60% da linha: texto 2 partes, controle 3 partes.
const TEXT_PORTION: u16 = 2;
const CONTROL_PORTION: u16 = 3;
/// Alfa do fundo de um tom (pílula, selo).
const TONE_BG_ALPHA: f32 = 0.14;
const SWATCH_SIZE: f32 = 28.0;
const INPUT_MAX_WIDTH: f32 = 320.0;

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

fn dimmed(theme: &cosmic::Theme) -> iced::widget::text::Style {
    let mut color = theme.cosmic().background(theme.transparent).on;
    color.alpha *= DESC_OPACITY;
    iced::widget::text::Style {
        color: Some(color.into()),
        ..Default::default()
    }
}

fn label_text<'a>(label: &'a str) -> Element<'a, Message> {
    widget::text(label)
        .size(LABEL_SIZE)
        .font(cosmic::font::semibold())
        .into()
}

fn desc_text<'a>(desc: &'a str) -> Element<'a, Message> {
    widget::text(desc)
        .size(DESC_SIZE)
        .class(theme::Text::Custom(dimmed))
        .into()
}

fn title_block<'a>(label: &'a str, desc: Option<&'a str>) -> Element<'a, Message> {
    let mut col = widget::column::with_capacity(2)
        .spacing(cosmic::theme::spacing().space_xxxs)
        .push(label_text(label));
    if let Some(d) = desc {
        col = col.push(desc_text(d));
    }
    col.into()
}

/// Linha base: texto à esquerda, controle à direita (até 60%).
fn shell<'a>(
    label: &'a str,
    desc: Option<&'a str>,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    let spacing = cosmic::theme::spacing();
    widget::row::with_children(vec![
        widget::container(title_block(label, desc))
            .width(Length::FillPortion(TEXT_PORTION))
            .into(),
        widget::container(control)
            .width(Length::FillPortion(CONTROL_PORTION))
            .align_x(Alignment::End)
            .into(),
    ])
    .spacing(spacing.space_s)
    .align_y(Alignment::Center)
    .padding(ROW_PADDING)
    .into()
}

/// Cartão com título, descrição opcional e linhas separadas por divisória.
pub fn group<'a>(
    title: &'a str,
    desc: Option<&'a str>,
    advanced: bool,
    rows: Vec<Element<'a, Message>>,
) -> Element<'a, Message> {
    let spacing = cosmic::theme::spacing();
    let mut head = widget::row::with_capacity(2)
        .spacing(spacing.space_xxs)
        .align_y(Alignment::Center)
        .push(widget::text(title).size(LABEL_SIZE).font(cosmic::font::bold()));
    if advanced {
        head = head.push(pill("Avançado", Tone::Neutral));
    }
    let mut header = widget::column::with_capacity(2)
        .spacing(spacing.space_xxxs)
        .push(head);
    if let Some(d) = desc {
        header = header.push(desc_text(d));
    }

    let mut body = widget::column::with_capacity(rows.len() * 2);
    for (i, r) in rows.into_iter().enumerate() {
        if i > 0 {
            body = body.push(widget::divider::horizontal::default());
        }
        body = body.push(r);
    }

    let card_box = widget::container(body)
        .width(Length::Fill)
        .class(theme::Container::custom(|theme| {
            let c = card(theme.cosmic());
            let mut border: Color = theme.cosmic().background(theme.transparent).divider.into();
            border.a *= 0.55;
            widget::container::Style {
                background: Some(Background::Color(c.base.into())),
                text_color: Some(c.on.into()),
                icon_color: Some(c.on.into()),
                border: iced::Border {
                    radius: crate::shared::tokens::POPUP_RADIUS.into(),
                    width: crate::shared::tokens::HAIRLINE_WIDTH,
                    color: border,
                },
                ..Default::default()
            }
        }));

    widget::column::with_children(vec![
        widget::container(header)
            .padding([0, ROW_PADDING[1]])
            .into(),
        card_box.into(),
    ])
    .spacing(spacing.space_xxs)
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

pub fn select<'a>(
    label: &'a str,
    desc: Option<&'a str>,
    options: &'a [String],
    selected: Option<usize>,
    on: impl Fn(usize) -> Message + Send + Sync + 'static,
) -> Element<'a, Message> {
    shell(label, desc, widget::dropdown(options, selected, on).into())
}

pub fn segmented<'a>(
    label: &'a str,
    desc: Option<&'a str>,
    options: &[&'a str],
    selected: usize,
    on: impl Fn(usize) -> Message + 'a,
) -> Element<'a, Message> {
    let spacing = cosmic::theme::spacing();
    let mut r = widget::row::with_capacity(options.len())
        .spacing(spacing.space_xxxs)
        .align_y(Alignment::Center);
    for (i, opt) in options.iter().enumerate() {
        r = r.push(
            button::custom(widget::text::caption(*opt))
                .padding([5, 12])
                .class(chip_class(i == selected))
                .on_press(on(i)),
        );
    }
    shell(label, desc, r.into())
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
    let spacing = cosmic::theme::spacing();
    let mut s = widget::slider(range, value, on)
        .step(step)
        .width(Length::Fill);
    if let Some(m) = on_release {
        s = s.on_release(m);
    }
    let badge = widget::container(
        widget::text(shown)
            .size(LABEL_SIZE)
            .font(cosmic::font::bold()),
    )
    .padding([2, spacing.space_xxs])
    .class(theme::Container::custom(|theme| {
        let (bg, fg) = tone_colors(theme, Tone::Info);
        widget::container::Style {
            background: Some(Background::Color(bg)),
            text_color: Some(fg),
            border: iced::Border {
                radius: theme.cosmic().radius_xl().into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }));
    let ends = widget::row::with_children(vec![
        widget::text::caption(min_max.0)
            .class(theme::Text::Custom(dimmed))
            .width(Length::Fill)
            .into(),
        widget::text::caption(min_max.1)
            .class(theme::Text::Custom(dimmed))
            .into(),
    ]);
    let control = widget::column::with_children(vec![
        widget::row::with_children(vec![s.into(), badge.into()])
            .spacing(spacing.space_xs)
            .align_y(Alignment::Center)
            .into(),
        ends.into(),
    ])
    .spacing(spacing.space_xxxs);
    shell(label, desc, control.into())
}

pub fn status<'a>(
    label: &'a str,
    desc: Option<&'a str>,
    pill_text: &'a str,
    tone: Tone,
    buttons: Vec<Element<'a, Message>>,
) -> Element<'a, Message> {
    let spacing = cosmic::theme::spacing();
    let mut r = widget::row::with_capacity(buttons.len() + 1)
        .spacing(spacing.space_xxs)
        .align_y(Alignment::Center)
        .push(pill(pill_text, tone));
    for b in buttons {
        r = r.push(b);
    }
    shell(label, desc, r.into())
}

pub fn kv<'a>(label: &'a str, value: String) -> Element<'a, Message> {
    shell(label, None, widget::text(value).size(LABEL_SIZE).into())
}

pub fn input<'a>(
    label: &'a str,
    desc: Option<&'a str>,
    placeholder: &'a str,
    value: &'a str,
    on: impl Fn(String) -> Message + 'a,
    action: Option<(&'a str, Message)>,
) -> Element<'a, Message> {
    let spacing = cosmic::theme::spacing();
    let mut r = widget::row::with_capacity(2)
        .spacing(spacing.space_xxs)
        .align_y(Alignment::Center)
        .push(
            widget::text_input(placeholder, value)
                .on_input(on)
                .width(Length::Fill),
        );
    if let Some((text, msg)) = action {
        r = r.push(button(text, msg, BtnStyle::Normal));
    }
    shell(
        label,
        desc,
        widget::container(r)
            .width(Length::Fill)
            .max_width(INPUT_MAX_WIDTH)
            .into(),
    )
}

pub fn buttons<'a>(
    label: &'a str,
    desc: Option<&'a str>,
    items: Vec<Element<'a, Message>>,
) -> Element<'a, Message> {
    let spacing = cosmic::theme::spacing();
    let mut r = widget::row::with_capacity(items.len())
        .spacing(spacing.space_xxs)
        .align_y(Alignment::Center);
    for b in items {
        r = r.push(b);
    }
    shell(label, desc, r.into())
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
    match style {
        BtnStyle::Primary => widget::button::suggested(label).on_press_maybe(msg).into(),
        BtnStyle::Normal => widget::button::standard(label).on_press_maybe(msg).into(),
        BtnStyle::Danger => widget::button::destructive(label).on_press_maybe(msg).into(),
    }
}

pub fn pill<'a>(text: &'a str, tone: Tone) -> Element<'a, Message> {
    widget::container(
        widget::text(text)
            .size(DESC_SIZE)
            .font(cosmic::font::semibold()),
    )
    .padding([2, cosmic::theme::spacing().space_xxs])
    .class(theme::Container::custom(move |theme| {
        let (bg, fg) = tone_colors(theme, tone);
        widget::container::Style {
            background: Some(Background::Color(bg)),
            text_color: Some(fg),
            border: iced::Border {
                radius: theme.cosmic().radius_xl().into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }))
    .into()
}

pub fn note<'a>(text: &'a str) -> Element<'a, Message> {
    widget::container(desc_text(text))
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
    let spacing = cosmic::theme::spacing();
    let mut r = widget::row::with_capacity(3)
        .spacing(spacing.space_xs)
        .align_y(Alignment::Center)
        .padding(ROW_PADDING)
        .push(
            widget::column::with_children(vec![
                label_text(name),
                widget::text(meta)
                    .size(DESC_SIZE)
                    .class(theme::Text::Custom(dimmed))
                    .into(),
            ])
            .spacing(spacing.space_xxxs)
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
    let spacing = cosmic::theme::spacing();
    let mut r = widget::row::with_capacity(items.len())
        .spacing(spacing.space_xxs)
        .align_y(Alignment::Center);
    for (i, (name, color)) in items.iter().enumerate() {
        let color = *color;
        let chosen = selected == Some(i);
        let dot = widget::container(widget::Space::new().width(SWATCH_SIZE).height(SWATCH_SIZE)).class(
            theme::Container::custom(move |theme| widget::container::Style {
                background: Some(Background::Color(color)),
                border: iced::Border {
                    radius: (SWATCH_SIZE / 2.0).into(),
                    width: if chosen { 2.0 } else { 0.0 },
                    color: theme.cosmic().accent_color().into(),
                },
                ..Default::default()
            }),
        );
        r = r.push(widget::tooltip(
            button::custom(dot)
                .padding(2)
                .class(theme::Button::Transparent)
                .on_press(on(i)),
            widget::text::caption(*name),
            widget::tooltip::Position::Bottom,
        ));
    }
    shell(label, desc, r.into())
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
}
