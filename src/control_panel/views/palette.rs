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

use super::row;
use crate::central::tiles::icon;
use crate::control_panel::{
    ControlPanel, Message, chrome,
    search_index::{Entry, EntryKind},
};

const INPUT_ID: &str = "cp-palette-input";
/// Medidas do mockup `Main` (paleta B): caixa de 640, até 560 de altura, a 96 px do topo.
const DIALOG_WIDTH: f32 = 640.0;
const DIALOG_MAX_HEIGHT: f32 = 560.0;
const DIALOG_TOP: f32 = 96.0;
const ITEM_HEIGHT: f32 = 44.0;
/// Altura do que não rola na caixa: campo (54), rodapé (~36) e as duas divisórias.
const LIST_CHROME: f32 = 92.0;

pub(super) fn input_id() -> widget::Id {
    widget::Id::new(INPUT_ID)
}

fn faint(theme: &cosmic::Theme) -> iced::widget::text::Style {
    iced::widget::text::Style { color: Some(row::on_alpha(theme, 0.58)), ..Default::default() }
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
    widget::container(
        widget::text(group_title(kind)).size(11.5).font(cosmic::font::semibold()).class(theme::Text::Custom(faint)),
    )
    .padding([8, 10, 4, 10])
    .into()
}

fn entry_icon(entry: &Entry) -> &'static str {
    match entry.kind {
        EntryKind::Setting => entry.section.icon(),
        EntryKind::Action => "media-playback-start-symbolic",
        EntryKind::Native => "preferences-system-symbolic",
    }
}

fn result_row(i: usize, entry: &'static Entry, selected: bool) -> Element<'static, Message> {
    let where_ = match entry.kind {
        EntryKind::Setting => format!("{} › {}", entry.section.label(), entry.group),
        EntryKind::Action => entry.section.label().to_owned(),
        EntryKind::Native => "COSMIC Settings".to_owned(),
    };
    let hint = match entry.kind {
        EntryKind::Setting => "",
        EntryKind::Action => "Ctrl Enter",
        EntryKind::Native => "Abre fora",
    };
    let icon_box = widget::container(widget::icon::icon(icon(entry_icon(entry))).size(15))
        .width(Length::Fixed(28.0))
        .height(Length::Fixed(28.0))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .class(theme::Container::custom(|theme| widget::container::Style {
            background: Some(Background::Color(row::on_alpha(theme, 0.07))),
            icon_color: Some(row::on_alpha(theme, 0.68)),
            border: iced::Border { radius: 8.0.into(), ..Default::default() },
            ..Default::default()
        }));
    let mut line = widget::row::with_capacity(3).spacing(12).align_y(Alignment::Center).push(icon_box).push(
        widget::column::with_children(vec![
            widget::text(entry.label).size(13.5).into(),
            widget::text(where_).size(12.0).class(theme::Text::Custom(faint)).into(),
        ])
        .spacing(1)
        .width(Length::Fill),
    );
    if !hint.is_empty() {
        line = line.push(widget::text(hint).size(11.5).class(theme::Text::Custom(faint)));
    }
    let style = move |hovered: bool, theme: &cosmic::Theme| widget::button::Style {
        background: if selected {
            Some(Background::Color(row::accent_alpha(theme, 0.12)))
        } else {
            hovered.then(|| Background::Color(row::on_alpha(theme, 0.05)))
        },
        border_radius: 10.0.into(),
        text_color: Some(row::on_alpha(theme, 1.0)),
        icon_color: Some(row::on_alpha(theme, 1.0)),
        ..widget::button::Style::new()
    };
    button::custom(line.height(Length::Fill))
        .height(Length::Fixed(ITEM_HEIGHT))
        .padding([0, 10])
        .width(Length::Fill)
        .class(theme::Button::Custom {
            active: Box::new(move |_, t| style(false, t)),
            disabled: Box::new(move |t| style(false, t)),
            hovered: Box::new(move |_, t| style(true, t)),
            pressed: Box::new(move |_, t| style(true, t)),
        })
        .on_press(Message::PaletteClick(i))
        .into()
}

/// Etiqueta de tecla ("Esc").
fn key_chip(text: &'static str) -> Element<'static, Message> {
    widget::container(widget::text(text).size(11.0).class(theme::Text::Custom(row::dimmed)))
        .padding([3, 7])
        .class(theme::Container::custom(|theme| widget::container::Style {
            border: iced::Border { radius: 6.0.into(), width: 1.0, color: row::on_alpha(theme, 0.14) },
            ..Default::default()
        }))
        .into()
}

fn footer() -> Element<'static, Message> {
    let t = |s: &'static str| -> Element<'static, Message> {
        widget::text(s).size(11.5).class(theme::Text::Custom(faint)).into()
    };
    widget::container(
        widget::row::with_children(vec![
            t("↑ ↓ navegar"),
            t("Enter abrir"),
            t("Ctrl Enter executar ação"),
            widget::Space::new().width(Length::Fill).into(),
            t("Inclui páginas do COSMIC Settings"),
        ])
        .spacing(16)
        .align_y(Alignment::Center),
    )
    .padding([10, 16])
    .width(Length::Fill)
    .into()
}

fn message_line(text: &'static str) -> Element<'static, Message> {
    widget::container(widget::text(text).size(12.5).class(theme::Text::Custom(row::dimmed)))
        .padding([12, 10])
        .into()
}

/// A camada da paleta (fundo + caixa), ou `None` com a paleta fechada.
pub(super) fn palette_view(app: &ControlPanel) -> Option<Element<'_, Message>> {
    let state = app.palette.as_ref()?;
    let results = chrome::flat_results(&state.query);
    // O cursor "dá a volta"; destaca a mesma linha que `PaletteActivate` usaria.
    let selected = cursor_target(&results, state.cursor);

    let input = widget::text_input::inline_input("Buscar ajuste, ação ou página do COSMIC…", state.query.as_str())
        .id(input_id())
        .on_input(Message::PaletteQuery)
        .size(15.0)
        .width(Length::Fill);
    let head = widget::container(
        widget::row::with_children(vec![
            widget::icon::icon(icon("system-search-symbolic"))
                .size(18)
                .class(theme::Svg::Custom(std::rc::Rc::new(|theme| iced::widget::svg::Style {
                    color: Some(row::accent(theme)),
                })))
                .into(),
            input.into(),
            button::custom(key_chip("Esc")).padding(0).class(theme::Button::Transparent).on_press(Message::ClosePalette).into(),
        ])
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .height(Length::Fixed(54.0))
    .align_y(Alignment::Center)
    .padding([0, 16]);

    let mut list = widget::column::with_capacity(results.len() + 4).spacing(2);
    if state.query.trim().is_empty() {
        list = list.push(message_line("Digite para buscar ajustes, ações e páginas do COSMIC Settings."));
    } else if results.is_empty() {
        list = list.push(message_line("Nenhum resultado."));
    }
    let mut last_kind: Option<EntryKind> = None;
    for (i, entry) in results.iter().enumerate() {
        if last_kind != Some(entry.kind) {
            list = list.push(header(entry.kind));
            last_kind = Some(entry.kind);
        }
        list = list.push(result_row(i, entry, selected.is_some_and(|t| std::ptr::eq(t, *entry))));
    }

    let col = widget::column::with_children(vec![
        head.into(),
        row::hairline(),
        // A lista rola; cabeçalho e rodapé ficam sempre à vista (caixa de até 560 px).
        widget::container(widget::scrollable(widget::container(list).padding(8)).height(Length::Shrink))
            .max_height(DIALOG_MAX_HEIGHT - LIST_CHROME)
            .into(),
        row::hairline(),
        footer(),
    ]);

    let dialog = widget::container(col)
        .width(Length::Fixed(DIALOG_WIDTH))
        .class(theme::Container::custom(|theme| {
            let c = theme.cosmic();
            widget::container::Style {
                background: Some(Background::Color(c.primary(theme.transparent).base.into())),
                text_color: Some(c.primary(theme.transparent).on.into()),
                border: iced::Border { radius: 16.0.into(), width: 1.0, color: row::on_alpha(theme, 0.09) },
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.6),
                    offset: iced::Vector::new(0.0, 24.0),
                    blur_radius: 64.0,
                },
                ..Default::default()
            }
        }));

    let backdrop = widget::mouse_area(
        widget::container(widget::Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .class(theme::Container::custom(|_| widget::container::Style {
                background: Some(Background::Color(Color::from_rgba(0.03, 0.035, 0.04, 0.62))),
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
