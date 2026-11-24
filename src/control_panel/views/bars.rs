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
    vec![widget::row::with_children(vec![
        widget::icon::icon(icon("preferences-other-symbolic"))
            .size(18)
            .class(theme::Svg::Custom(std::rc::Rc::new(|theme| iced::widget::svg::Style {
                color: Some(row::accent(theme)),
            })))
            .into(),
        widget::text("Painel de Controle").size(13.0).font(cosmic::font::semibold()).into(),
    ])
    .spacing(8)
    .align_y(Alignment::Center)
    .into()]
}

/// Campo de busca falso: abre a paleta (Ctrl K).
pub(super) fn header_center(_app: &ControlPanel) -> Vec<Element<'_, Message>> {
    let key = widget::container(widget::text("Ctrl K").size(11.0).class(theme::Text::Custom(dim)))
        .padding([2, 6])
        .class(theme::Container::custom(|theme| widget::container::Style {
            border: iced::Border { radius: 5.0.into(), width: 1.0, color: row::on_alpha(theme, 0.14) },
            ..Default::default()
        }));
    let content = widget::row::with_children(vec![
        widget::icon::icon(icon("system-search-symbolic")).size(SMALL_ICON).into(),
        widget::text("Buscar ajuste ou ação…")
            .size(13.0)
            .class(theme::Text::Custom(dim))
            .width(Length::Fill)
            .into(),
        key.into(),
    ])
    .spacing(8)
    .height(Length::Fill)
    .align_y(Alignment::Center);
    vec![
        widget::container(
            button::custom(content)
                .height(Length::Fixed(32.0))
                .padding([0, 8, 0, 12])
                .width(Length::Fill)
                .class(search_class())
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
    let ok = count == 0;
    let style: fn(&cosmic::Theme) -> iced::widget::text::Style = if ok { ok_text } else { warn_text };
    let dot = widget::container(widget::Space::new().width(7.0).height(7.0)).class(theme::Container::custom(
        move |theme| {
            let c: Color = if ok { theme.cosmic().success_color().into() } else { theme.cosmic().warning_color().into() };
            widget::container::Style {
                background: Some(Background::Color(c)),
                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                ..Default::default()
            }
        },
    ));
    vec![widget::container(
        widget::row::with_children(vec![
            dot.into(),
            widget::text(text).size(12.0).class(theme::Text::Custom(style)).into(),
        ])
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([0, 6, 0, 0])
    .into()]
}

/// Campo de busca do cabeçalho: fundo elevado, contorno fino, raio 9 (mockup).
fn search_class() -> theme::Button {
    let style = |hovered: bool, theme: &cosmic::Theme| cosmic::widget::button::Style {
        background: Some(Background::Color(row::on_alpha(theme, if hovered { 0.09 } else { 0.06 }))),
        border_radius: 9.0.into(),
        border_width: 1.0,
        border_color: row::on_alpha(theme, 0.08),
        text_color: Some(row::on_alpha(theme, 0.68)),
        icon_color: Some(row::on_alpha(theme, 0.68)),
        ..cosmic::widget::button::Style::new()
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
    let go = row::action_btn(bar.button, (!app.busy).then_some(bar.msg), row::BtnStyle::Primary, 32.0);
    Some(
        widget::container(
            widget::row::with_children(vec![
                widget::icon::icon(icon("dialog-warning-symbolic")).size(SMALL_ICON).into(),
                widget::text(bar.text).size(13.0).width(Length::Fill).into(),
                go,
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
    let tone = tone_of(t.kind);
    let badge = widget::container(widget::icon::icon(icon(toast_icon(t.kind))).size(13))
        .width(Length::Fixed(22.0))
        .height(Length::Fixed(22.0))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .class(theme::Container::custom(move |theme| {
            let (_, fg) = row::tone_colors(theme, tone);
            widget::container::Style {
                background: Some(Background::Color(Color { a: 0.18, ..fg })),
                icon_color: Some(fg),
                border: iced::Border { radius: 11.0.into(), ..Default::default() },
                ..Default::default()
            }
        }));
    let mut r = widget::row::with_capacity(4)
        .spacing(10)
        .align_y(Alignment::Center)
        .push(badge)
        .push(widget::text(t.text.as_str()).size(13.0).width(Length::Fill));
    if let Some(undo) = &t.undo {
        r = r.push(
            button::custom(widget::text("Desfazer").size(12.5).font(row::medium()))
                .padding([5, 8])
                .class(row::btn_class(row::Btn::Link, 7.0))
                .on_press(undo.clone()),
        );
    }
    r = r.push(
        button::icon(icon("window-close-symbolic"))
            .icon_size(14)
            .padding(4)
            .on_press(Message::DismissToast(t.id)),
    );
    widget::container(r)
        .padding([10, 12, 10, 14])
        .width(Length::Fixed(TOAST_WIDTH))
        .class(theme::Container::custom(move |theme| {
            let c = card(theme.cosmic());
            widget::container::Style {
                background: Some(Background::Color(theme.cosmic().primary(theme.transparent).base.into())),
                text_color: Some(c.on.into()),
                border: iced::Border { radius: 12.0.into(), width: 1.0, color: row::on_alpha(theme, 0.08) },
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                    offset: iced::Vector::new(0.0, 12.0),
                    blur_radius: 32.0,
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
