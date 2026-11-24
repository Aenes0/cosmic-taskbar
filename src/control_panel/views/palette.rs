// SPDX-License-Identifier: GPL-3.0-only

//! Paleta Ctrl K. Esqueleto do Task 9: caixa de busca + lista plana de resultados na ordem
//! Ajustes, Ações, COSMIC Settings (a mesma lista que o `update` indexa com `PaletteState::cursor`).
//! O Task 11 refaz o visual; o contrato fica: `palette_view` devolve a camada inteira (fundo que
//! fecha ao clicar + caixa), o campo usa `input_id()`, Enter/setas/Esc vêm da assinatura de
//! teclado (por isso o campo NÃO liga `on_submit`) e o clique numa linha manda `PaletteClick(i)`.

use cosmic::{
    Element,
    iced::{self, Alignment, Background, Color, Length},
    theme,
    widget::{self, button},
};

use crate::central::tiles::chip_class;
use crate::control_panel::{
    ControlPanel, Message, chrome,
    search_index::{Entry, EntryKind},
};

const INPUT_ID: &str = "cp-palette-input";
const DIALOG_WIDTH: f32 = 640.0;
const DIALOG_TOP: f32 = 72.0;

pub(super) fn input_id() -> widget::Id {
    widget::Id::new(INPUT_ID)
}

fn dim(theme: &cosmic::Theme) -> iced::widget::text::Style {
    let mut color = theme.cosmic().background(theme.transparent).on;
    color.alpha *= 0.65;
    iced::widget::text::Style { color: Some(color.into()), ..Default::default() }
}

fn kind_label(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Setting => "Ajuste",
        EntryKind::Action => "Ação",
        EntryKind::Native => "COSMIC Settings",
    }
}

/// Entrada sob o cursor; o cursor passando do fim volta ao começo. Sem resultados: `None`.
pub(super) fn cursor_target<'a>(results: &[&'a Entry], cursor: usize) -> Option<&'a Entry> {
    if results.is_empty() {
        return None;
    }
    results.get(cursor % results.len()).copied()
}

fn group_title(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Setting => "Ajustes",
        EntryKind::Action => "Ações",
        EntryKind::Native => "COSMIC Settings",
    }
}

fn header(kind: EntryKind) -> Element<'static, Message> {
    let spacing = theme::spacing();
    widget::container(widget::text::caption(group_title(kind)).class(theme::Text::Custom(dim)))
        .padding([spacing.space_xs, spacing.space_xs, 0, spacing.space_xs])
        .into()
}

fn result_row(i: usize, entry: &'static Entry, selected: bool) -> Element<'static, Message> {
    let spacing = theme::spacing();
    let where_ = match entry.kind {
        EntryKind::Setting => format!("{} › {}", entry.section.label(), entry.group),
        EntryKind::Action => entry.section.label().to_owned(),
        EntryKind::Native => kind_label(entry.kind).to_owned(),
    };
    button::custom(
        widget::row::with_children(vec![
            widget::text::body(entry.label).width(Length::Fill).into(),
            widget::text::caption(where_).class(theme::Text::Custom(dim)).into(),
        ])
        .spacing(spacing.space_xs)
        .align_y(Alignment::Center),
    )
    .padding([spacing.space_xxs, spacing.space_xs])
    .width(Length::Fill)
    .class(chip_class(selected))
    .on_press(Message::PaletteClick(i))
    .into()
}

fn footer() -> Element<'static, Message> {
    let spacing = theme::spacing();
    widget::container(
        widget::text::caption("↑↓ navegar  ·  Enter abrir  ·  Ctrl Enter executar  ·  Esc fechar")
            .class(theme::Text::Custom(dim)),
    )
    .padding([spacing.space_xxs, spacing.space_xs, 0, spacing.space_xs])
    .into()
}

/// A camada da paleta (fundo + caixa), ou `None` com a paleta fechada.
pub(super) fn palette_view(app: &ControlPanel) -> Option<Element<'_, Message>> {
    let state = app.palette.as_ref()?;
    let spacing = theme::spacing();
    let results = chrome::flat_results(&state.query);
    // O cursor "dá a volta"; destaca a mesma linha que `PaletteActivate` usaria.
    let selected = cursor_target(&results, state.cursor);

    let input = widget::search_input("Buscar ajuste ou ação…", state.query.as_str())
        .id(input_id())
        .on_input(Message::PaletteQuery)
        .width(Length::Fill);

    let mut col = widget::column::with_capacity(results.len() + 6)
        .spacing(spacing.space_xxs)
        .push(input);
    if state.query.trim().is_empty() {
        col = col.push(
            widget::container(
                widget::text::caption("Digite para buscar ajustes, ações e páginas do COSMIC Settings.")
                    .class(theme::Text::Custom(dim)),
            )
            .padding(spacing.space_xs),
        );
    } else if results.is_empty() {
        col = col.push(
            widget::container(widget::text::caption("Nenhum resultado.").class(theme::Text::Custom(dim)))
                .padding(spacing.space_xs),
        );
    }
    let mut last_kind: Option<EntryKind> = None;
    for (i, entry) in results.iter().enumerate() {
        if last_kind != Some(entry.kind) {
            col = col.push(header(entry.kind));
            last_kind = Some(entry.kind);
        }
        col = col.push(result_row(i, entry, selected.is_some_and(|t| std::ptr::eq(t, *entry))));
    }
    col = col.push(footer());

    let dialog = widget::container(col)
        .padding(spacing.space_s)
        .width(Length::Fixed(DIALOG_WIDTH))
        .max_width(DIALOG_WIDTH)
        .class(theme::Container::custom(|theme| {
            let bg = theme.cosmic().background(theme.transparent);
            let mut border: Color = bg.divider.into();
            border.a *= 0.8;
            widget::container::Style {
                background: Some(Background::Color(bg.base.into())),
                text_color: Some(bg.on.into()),
                border: iced::Border { radius: theme.cosmic().radius_m().into(), width: 1.0, color: border },
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                    offset: iced::Vector::new(0.0, 12.0),
                    blur_radius: 32.0,
                },
                ..Default::default()
            }
        }));

    let backdrop = widget::mouse_area(
        widget::container(widget::Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .class(theme::Container::custom(|_| widget::container::Style {
                background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.45))),
                ..Default::default()
            })),
    )
    .on_press(Message::ClosePalette);

    let placed = widget::container(widget::mouse_area(dialog).on_press(Message::None))
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .padding([DIALOG_TOP, 0.0]);

    Some(
        iced::widget::stack(vec![backdrop.into(), placed.into()])
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control_panel::chrome::flat_results;

    #[test]
    fn cursor_target_empty_results_is_none() {
        assert!(cursor_target(&[], 0).is_none());
        assert!(cursor_target(&[], 7).is_none());
    }

    #[test]
    fn cursor_target_wraps_past_the_end() {
        let results = flat_results("tela");
        assert!(results.len() >= 2, "índice precisa ter ao menos 2 resultados para 'tela'");
        let first = cursor_target(&results, 0).unwrap();
        assert!(std::ptr::eq(first, results[0]));
        let last = cursor_target(&results, results.len() - 1).unwrap();
        assert!(std::ptr::eq(last, results[results.len() - 1]));
        let wrapped = cursor_target(&results, results.len()).unwrap();
        assert!(std::ptr::eq(wrapped, results[0]));
    }
}
