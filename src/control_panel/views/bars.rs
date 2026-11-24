// SPDX-License-Identifier: GPL-3.0-only

//! Peças fixas da moldura que ficam fora da página: botões do cabeçalho da janela
//! (título, busca Ctrl K, indicador de salvamento), barra "Aplicar" do rodapé e a pilha de
//! avisos no canto inferior direito.

use cosmic::{
    Element,
    iced::{self, Alignment, Background, Color, Length},
    theme,
    widget::{self, button},
};

use super::row::{self, Tone};
use crate::central::tiles::icon;
use crate::central::widgets::card;
use crate::control_panel::{
    ControlPanel, Message, Section,
    feedback::{Toast, ToastKind},
    pending::ApplyAction,
};

/// Largura máxima do botão de busca do cabeçalho (mockup `Direcao-B`).
const SEARCH_MAX_WIDTH: f32 = 480.0;
/// Largura de um aviso.
const TOAST_WIDTH: f32 = 380.0;
const SMALL_ICON: u16 = 16;

fn dim(theme: &cosmic::Theme) -> iced::widget::text::Style {
    let mut color = theme.cosmic().background(theme.transparent).on;
    color.alpha *= 0.65;
    iced::widget::text::Style { color: Some(color.into()), ..Default::default() }
}

fn ok_text(theme: &cosmic::Theme) -> iced::widget::text::Style {
    iced::widget::text::Style { color: Some(theme.cosmic().success_color().into()), ..Default::default() }
}

fn warn_text(theme: &cosmic::Theme) -> iced::widget::text::Style {
    iced::widget::text::Style { color: Some(theme.cosmic().warning_color().into()), ..Default::default() }
}

pub(super) fn header_start(_app: &ControlPanel) -> Vec<Element<'_, Message>> {
    vec![widget::text::heading("Painel de Controle").into()]
}

/// Campo de busca falso: abre a paleta (Ctrl K).
pub(super) fn header_center(_app: &ControlPanel) -> Vec<Element<'_, Message>> {
    let spacing = theme::spacing();
    let content = widget::row::with_children(vec![
        widget::icon::icon(icon("system-search-symbolic")).size(SMALL_ICON).into(),
        widget::text::body("Buscar ajuste ou ação…")
            .class(theme::Text::Custom(dim))
            .width(Length::Fill)
            .into(),
        widget::text::caption("Ctrl K").class(theme::Text::Custom(dim)).into(),
    ])
    .spacing(spacing.space_xxs)
    .align_y(Alignment::Center);
    vec![
        widget::container(
            button::custom(content)
                .padding([spacing.space_xxs, spacing.space_xs])
                .width(Length::Fill)
                .class(row_class())
                .on_press(Message::OpenPalette),
        )
        .max_width(SEARCH_MAX_WIDTH)
        .width(Length::Fill)
        .into(),
    ]
}

/// "Tudo salvo" / "N ajuste(s) para aplicar".
pub(super) fn header_end(app: &ControlPanel) -> Vec<Element<'_, Message>> {
    let count = app.pending_count();
    let text = crate::control_panel::save_status(count);
    let style: fn(&cosmic::Theme) -> iced::widget::text::Style = if count == 0 { ok_text } else { warn_text };
    vec![widget::text::caption(text).class(theme::Text::Custom(style)).into()]
}

/// Botão achatado com contorno fino: o campo de busca do cabeçalho e o botão de avançado.
pub(super) fn row_class() -> theme::Button {
    let style = |hovered: bool, theme: &cosmic::Theme| {
        let c = card(theme.cosmic());
        let mut border: Color = theme.cosmic().background(theme.transparent).divider.into();
        border.a *= 0.8;
        cosmic::widget::button::Style {
            background: Some(Background::Color(if hovered { c.hover } else { c.base }.into())),
            border_radius: theme.cosmic().radius_xl().into(),
            border_width: 1.0,
            border_color: border,
            text_color: Some(c.on.into()),
            icon_color: Some(c.on.into()),
            ..cosmic::widget::button::Style::new()
        }
    };
    theme::Button::Custom {
        active: Box::new(move |_, t| style(false, t)),
        disabled: Box::new(move |t| style(false, t)),
        hovered: Box::new(move |_, t| style(true, t)),
        pressed: Box::new(move |_, t| style(true, t)),
    }
}

/// Texto do que a barra "Aplicar" da página atual avisa, com o botão e a mensagem dele.
struct ApplyBar {
    text: String,
    button: String,
    msg: Message,
}

fn apply_bar_for(app: &ControlPanel) -> Option<ApplyBar> {
    match app.section {
        Section::Power => app.power_apply().map(|pending| {
            let plan = app.editing_profile.label();
            let text = match pending {
                ApplyAction::ApplyHardware => {
                    format!("Os ajustes do plano {plan} estão salvos, mas ainda não foram aplicados no hardware.")
                }
                ApplyAction::ActivateAndApply(_) => {
                    format!("O plano {plan} não é o ativo. Ative-o para aplicar os ajustes salvos.")
                }
            };
            ApplyBar {
                text,
                button: crate::control_panel::apply_bar_label(&pending, app.editing_profile),
                msg: Message::ApplyPower,
            }
        }),
        Section::DevicesOsd if app.osd_apply() => Some(ApplyBar {
            text: "A configuração do OSD está salva; o serviço ainda usa a anterior.".to_owned(),
            button: "Aplicar (reinicia o OSD)".to_owned(),
            msg: Message::ApplyOsd,
        }),
        _ => None,
    }
}

/// Barra fixa no rodapé da página, só quando há algo a aplicar. Desabilitada com `busy`.
pub(super) fn apply_bar(app: &ControlPanel) -> Option<Element<'_, Message>> {
    let bar = apply_bar_for(app)?;
    let spacing = theme::spacing();
    let go = widget::button::suggested(bar.button).on_press_maybe((!app.busy).then_some(bar.msg));
    Some(
        widget::container(
            widget::row::with_children(vec![
                widget::icon::icon(icon("dialog-warning-symbolic")).size(SMALL_ICON).into(),
                widget::text::body(bar.text).width(Length::Fill).into(),
                go.into(),
            ])
            .spacing(spacing.space_xs)
            .align_y(Alignment::Center),
        )
        .padding([spacing.space_xs, spacing.space_m])
        .width(Length::Fill)
        .class(theme::Container::custom(|theme| {
            let c = card(theme.cosmic());
            let mut border: Color = theme.cosmic().warning_color().into();
            border.a = 0.55;
            widget::container::Style {
                background: Some(Background::Color(c.base.into())),
                text_color: Some(c.on.into()),
                icon_color: Some(theme.cosmic().warning_color().into()),
                border: iced::Border { width: 1.0, color: border, ..Default::default() },
                ..Default::default()
            }
        }))
        .into(),
    )
}

fn tone_of(kind: ToastKind) -> Tone {
    match kind {
        ToastKind::Success => Tone::Ok,
        ToastKind::Error => Tone::Bad,
        ToastKind::Info | ToastKind::Progress => Tone::Info,
    }
}

fn toast_icon(kind: ToastKind) -> &'static str {
    match kind {
        ToastKind::Success => "emblem-ok-symbolic",
        ToastKind::Error => "dialog-error-symbolic",
        ToastKind::Info => "dialog-information-symbolic",
        ToastKind::Progress => "emblem-synchronizing-symbolic",
    }
}

fn toast_card(t: &Toast<Message>) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let tone = tone_of(t.kind);
    let mut r = widget::row::with_capacity(4)
        .spacing(spacing.space_xs)
        .align_y(Alignment::Center)
        .push(widget::icon::icon(icon(toast_icon(t.kind))).size(SMALL_ICON))
        .push(widget::text::body(t.text.as_str()).width(Length::Fill));
    if let Some(undo) = &t.undo {
        r = r.push(widget::button::text("Desfazer").on_press(undo.clone()));
    }
    r = r.push(
        button::icon(icon("window-close-symbolic"))
            .icon_size(SMALL_ICON)
            .padding(spacing.space_xxxs)
            .on_press(Message::DismissToast(t.id)),
    );
    widget::container(r)
        .padding([spacing.space_xs, spacing.space_s])
        .width(Length::Fixed(TOAST_WIDTH))
        .class(theme::Container::custom(move |theme| {
            let c = card(theme.cosmic());
            let (_, accent) = row::tone_colors(theme, tone);
            widget::container::Style {
                background: Some(Background::Color(
                    theme.cosmic().background(theme.transparent).base.into(),
                )),
                text_color: Some(c.on.into()),
                icon_color: Some(accent),
                border: iced::Border {
                    radius: theme.cosmic().radius_s().into(),
                    width: 1.0,
                    color: accent,
                },
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 16.0,
                },
                ..Default::default()
            }
        }))
        .into()
}

/// Pilha de avisos no canto inferior direito (a camada de cima do `stack`).
pub(super) fn toast_stack(app: &ControlPanel) -> Option<Element<'_, Message>> {
    let toasts = app.toasts.visible();
    if toasts.is_empty() {
        return None;
    }
    let spacing = theme::spacing();
    let cards: Vec<Element<'_, Message>> = toasts.iter().map(toast_card).collect();
    Some(
        widget::container(
            widget::column::with_children(cards)
                .spacing(spacing.space_xxs)
                .align_x(Alignment::End),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::End)
        .align_y(Alignment::End)
        .padding(spacing.space_s)
        .into(),
    )
}
