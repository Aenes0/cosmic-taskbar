// SPDX-License-Identifier: GPL-3.0-only

//! Linha de status (autenticando, Caps Lock, erro, informação) nas cores do estilo.

use std::borrow::Cow;

use cosmic::iced::alignment::Horizontal;
use cosmic::iced::{Alignment, Color, Length};
use cosmic::{Element, widget};

use super::text_line;
use crate::fl;
use crate::login_ui::{StatusLine, fade, shift};
use crate::motion::Look;
use crate::offset::offset;

/// Altura reservada (uma linha) para o layout não pular entre os estados.
const STATUS_MIN_HEIGHT: f32 = 20.0;
const STATUS_SIZE: f32 = 13.0;
const STATUS_MAX_WIDTH: f32 = 320.0;

/// O texto de cada estado; `None` no `Empty`.
fn status_text(status: StatusLine<'_>) -> Option<Cow<'_, str>> {
    match status {
        StatusLine::Empty => None,
        StatusLine::Authenticating => Some(Cow::Owned(fl!("authenticating"))),
        StatusLine::CapsLock => Some(Cow::Owned(fl!("caps-lock"))),
        StatusLine::Error(s) | StatusLine::Info(s) => Some(Cow::Borrowed(s)),
    }
}

/// Indicador de "autenticando" nas cores do estilo.
fn spinner<'a, M: Clone + 'a>(color: Color, alpha: f32) -> Element<'a, M> {
    widget::indeterminate_circular()
        .size(16.0)
        .bar_height(2.0)
        .track_color(fade(Color { a: color.a * 0.25, ..color }, alpha))
        .bar_color(fade(color, alpha))
        .into()
}

/// A linha de status: mesmo conteúdo da `login_ui::status_view_styled`, mas em `text` com a
/// sombra `shadow` (os dois temas ficam legíveis). Altura mínima de uma linha; texto longo do PAM
/// quebra dentro de 320 px. Nunca vermelha.
pub fn status<'a, M: Clone + 'a>(status: StatusLine<'a>, look: Look, text: Color, shadow: Color) -> Element<'a, M> {
    let authenticating = status == StatusLine::Authenticating;
    let content: Element<'a, M> = match status_text(status) {
        None => widget::space::horizontal().into(),
        Some(s) => {
            let line = text_line(s, STATUS_SIZE, cosmic::font::default(), text, shadow, look.alpha, Horizontal::Center);
            if authenticating {
                widget::row![spinner(text, look.alpha), line].spacing(8).align_y(Alignment::Center).into()
            } else {
                line
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_has_no_text_and_other_states_do() {
        crate::localize::localizer();
        assert_eq!(status_text(StatusLine::Empty), None);
        assert_eq!(status_text(StatusLine::Error("Senha incorreta")).as_deref(), Some("Senha incorreta"));
        assert_eq!(status_text(StatusLine::Info("Coloque o dedo")).as_deref(), Some("Coloque o dedo"));
        for s in [StatusLine::Authenticating, StatusLine::CapsLock] {
            assert!(status_text(s).is_some_and(|t| !t.is_empty()));
        }
    }
}
