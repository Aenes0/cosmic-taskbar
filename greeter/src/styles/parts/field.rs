// SPDX-License-Identifier: GPL-3.0-only

//! Campo de senha: retângulo do Windows 11 ou pílula.

use cosmic::iced::{self, Alignment, Background, Color, Length};
use cosmic::widget::text_input;
use cosmic::{Element, theme, widget};

use super::{tinted_icon, with_hovers};
use crate::fl;
use crate::login_ui::{SUBMIT_SIZE, fade, mix, shift};
use crate::offset::offset;
use crate::styles::FieldParts;

const PILL_HEIGHT: f32 = 40.0;
const FIELD_PADDING: u16 = 10;
const PILL_PADDING: u16 = 16;
const LOCK_ICON_SIZE: u16 = 16;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldColors {
    pub fill: Color,
    pub text: Color,
    pub placeholder: Color,
    pub border: Color,
    /// Cor do foco e da linha do Win11; `None` usa o destaque do tema.
    pub accent: Option<Color>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldShape {
    /// Retângulo de raio 4, seta quadrada à direita e linha de destaque embaixo.
    Win11,
    /// Pílula de 40 px, seta redonda à direita e, opcionalmente, cadeado à esquerda.
    Pill { lock_icon: bool },
}

fn field_appearance(c: FieldColors, background: Color, border: Color, radius: f32, text_alpha: f32, alpha: f32) -> text_input::Appearance {
    let text = Color { a: c.text.a * text_alpha, ..c.text };
    text_input::Appearance {
        background: Background::Color(fade(background, alpha)),
        border_radius: radius.into(),
        border_offset: None,
        border_width: 1.0,
        border_color: fade(border, alpha),
        label_color: fade(c.text, alpha),
        placeholder_color: fade(c.placeholder, alpha),
        selected_text_color: fade(c.text, alpha),
        icon_color: Some(fade(text, alpha)),
        text_color: Some(fade(text, alpha)),
        selected_fill: fade(Color { a: 0.3, ..c.text }, alpha),
    }
}

/// Fundo `fill` (um pouco mais denso no hover); a borda do foco vai de `border` ao destaque.
fn field_style(c: FieldColors, radius: f32, hover: f32, focus: f32, alpha: f32) -> theme::TextInput {
    let background = mix(c.fill, Color { a: (c.fill.a + 0.06).min(1.0), ..c.fill }, hover);
    theme::TextInput::Custom {
        active: Box::new(move |_| field_appearance(c, background, c.border, radius, 1.0, alpha)),
        error: Box::new(move |_| field_appearance(c, c.fill, c.border, radius, 1.0, alpha)),
        hovered: Box::new(move |_| field_appearance(c, background, c.border, radius, 1.0, alpha)),
        focused: Box::new(move |theme| {
            let accent = c.accent.unwrap_or_else(|| theme.cosmic().accent_color().into());
            field_appearance(c, c.fill, mix(c.border, accent, focus), radius, 1.0, alpha)
        }),
        disabled: Box::new(move |_| field_appearance(c, c.fill, c.border, radius, 0.5, alpha)),
    }
}

/// Seta de enviar: fundo `ink` translúcido (some em repouso no Win11, sempre visível na pílula).
fn submit_style(ink: Color, rest: f32, hover: f32, radius: [f32; 4], alpha: f32) -> theme::Button {
    let style = move |background: f32| {
        let mut s = widget::button::Style::new();
        s.background = (background > 0.0).then_some(Background::Color(fade(Color { a: background, ..ink }, alpha)));
        s.border_radius = radius.into();
        s.icon_color = Some(fade(ink, alpha));
        s.text_color = Some(fade(ink, alpha));
        s
    };
    let current = rest + 0.12 * hover.clamp(0.0, 1.0);
    theme::Button::Custom {
        active: Box::new(move |_, _| style(current)),
        disabled: Box::new(move |_| {
            let mut s = style(rest);
            s.icon_color = Some(fade(Color { a: ink.a * 0.4, ..ink }, alpha));
            s
        }),
        hovered: Box::new(move |_, _| style(current)),
        pressed: Box::new(move |_, _| style(0.2)),
    }
}

/// Campo de senha (ou de usuário) com a seta de enviar dentro, na `width` pedida.
pub fn field<'a, M: Clone + 'static>(
    parts: FieldParts<'a, M>,
    colors: FieldColors,
    shape: FieldShape,
    width: f32,
) -> Element<'a, M> {
    let FieldParts { input, submit, fx } = parts;
    let alpha = fx.look.alpha;
    let (height, radius, lock) = match shape {
        FieldShape::Win11 => (SUBMIT_SIZE, 4.0, false),
        FieldShape::Pill { lock_icon } => (PILL_HEIGHT, PILL_HEIGHT / 2.0, lock_icon),
    };
    let (inset, arrow_radius, arrow_rest) = match shape {
        FieldShape::Win11 => (1.0, [0.0, 3.0, 3.0, 0.0], 0.0),
        FieldShape::Pill { .. } => (4.0, [(height - 8.0) / 2.0; 4], 0.10),
    };
    let arrow_size = height - 2.0 * inset;
    let left = match shape {
        FieldShape::Win11 => FIELD_PADDING,
        FieldShape::Pill { .. } if lock => PILL_PADDING + LOCK_ICON_SIZE + 8,
        FieldShape::Pill { .. } => PILL_PADDING,
    };
    let input = with_hovers(
        input
            .width(Length::Fill)
            .padding([0, height as u16, 0, left])
            .line_height(iced::widget::text::LineHeight::Absolute(height.into()))
            .style(field_style(colors, radius, fx.hover, fx.focus, alpha)),
        fx.field_hovers,
    );
    let arrow = widget::tooltip(
        with_hovers(
            widget::button::custom(widget::icon::from_name("go-next-symbolic").size(16).icon())
                .width(Length::Fixed(arrow_size))
                .height(Length::Fixed(arrow_size))
                .padding(((arrow_size - 16.0) / 2.0) as u16)
                .class(submit_style(colors.text, arrow_rest, fx.submit_hover, arrow_radius, alpha))
                .on_press_maybe(submit),
            fx.submit_hovers,
        ),
        widget::text(fl!("sign-in")),
        widget::tooltip::Position::Top,
    );
    let mut layers: Vec<Element<'a, M>> = vec![input];
    if lock {
        layers.push(
            widget::container(tinted_icon("system-lock-screen-symbolic", LOCK_ICON_SIZE, fade(colors.placeholder, alpha)))
                .width(Length::Fill)
                .height(Length::Fixed(height))
                .padding([0, 0, 0, PILL_PADDING])
                .align_y(Alignment::Center)
                .into(),
        );
    }
    layers.push(
        widget::container(arrow)
            .width(Length::Fill)
            .height(Length::Fixed(height))
            .padding(inset as u16)
            .align_x(Alignment::End)
            .align_y(Alignment::Center)
            .into(),
    );
    let body: Element<'a, M> = iced::widget::Stack::with_children(layers)
        .width(Length::Fill)
        .height(Length::Fixed(height))
        .into();
    let mut column = widget::column![body].width(Length::Fixed(width));
    if shape == FieldShape::Win11 {
        let accent = colors.accent;
        column = column.push(
            widget::container(widget::space::horizontal())
                .width(Length::Fill)
                .height(Length::Fixed(2.0))
                .class(theme::Container::custom(move |theme| {
                    let accent = accent.unwrap_or_else(|| theme.cosmic().accent_color().into());
                    widget::container::Style {
                        background: Some(Background::Color(fade(accent, alpha))),
                        ..Default::default()
                    }
                })),
        );
    }
    offset(column, shift(fx.look)).into()
}
