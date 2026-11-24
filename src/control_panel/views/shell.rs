// SPDX-License-Identifier: GPL-3.0-only

//! Moldura do Painel de Controle (direção B): trilho de categorias, trilha "Início › Categoria",
//! abas das seções, cabeçalho da página, "Ir para", botão de avançado, barra "Aplicar" e a pilha
//! de avisos. As seções só desenham o corpo; ações e selo do cabeçalho vêm das funções
//! `<seção>_actions` / `<seção>_badge` dos arquivos delas. Medidas do mockup `Direcao-B`.

use cosmic::{
    Element,
    iced::{self, Alignment, Background, Color, Length},
    theme,
    widget::{self, button},
};

use super::{
    backup, bar, bars, camera, capture, central, displays, home, login, monitor, network,
    notifications, optional, osd, palette, power,
    row::{self, BtnStyle, Tone},
    security, system,
};
use crate::central::tiles::{chip_class, icon};
use crate::control_panel::{
    ControlPanel, Message, Section, chrome,
    nav::{Category, category_of},
};

/// Largura do trilho de categorias.
const RAIL_WIDTH: f32 = 92.0;
/// Largura do item do trilho (pílula + rótulo).
const RAIL_ITEM_WIDTH: f32 = 76.0;
const RAIL_PILL: (f32, f32) = (44.0, 30.0);
const RAIL_ICON: u16 = 19;
const RAIL_LABEL_SIZE: f32 = 11.0;
/// Largura máxima do conteúdo de uma página de seção e da Visão geral.
const PAGE_MAX_WIDTH: f32 = 900.0;
const HOME_MAX_WIDTH: f32 = 1060.0;
const PAGE_ICON_BOX: f32 = 44.0;
const TITLE_SIZE: f32 = 22.0;
const SUBTITLE_SIZE: f32 = 13.5;
const TAB_ICON: u16 = 16;
const TAB_UNDERLINE: f32 = 2.0;
/// Mais ações que isso no cabeçalho vão para uma linha própria abaixo do título.
const MAX_INLINE_ACTIONS: usize = 2;
const ADVANCED_ICON_BOX: f32 = 32.0;

pub(super) fn view(app: &ControlPanel) -> Element<'_, Message> {
    super::row::set_view_busy(app.busy);
    let body = widget::row::with_children(vec![rail(app), page(app)])
        .width(Length::Fill)
        .height(Length::Fill);
    let mut layers: Vec<Element<'_, Message>> = vec![body.into()];
    if let Some(toasts) = bars::toast_stack(app) {
        layers.push(toasts);
    }
    if let Some(palette) = palette::palette_view(app) {
        layers.push(palette);
    }
    iced::widget::stack(layers)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

// ── Estilos ──────────────────────────────────────────────────────────────

fn dim(theme: &cosmic::Theme) -> iced::widget::text::Style {
    let mut color = theme.cosmic().background(theme.transparent).on;
    color.alpha *= 0.65;
    iced::widget::text::Style { color: Some(color.into()), ..Default::default() }
}

fn accent_text(theme: &cosmic::Theme) -> iced::widget::text::Style {
    iced::widget::text::Style { color: Some(theme.cosmic().accent_color().into()), ..Default::default() }
}

/// Botão sem fundo; no hover ganha o fundo do cartão.
fn flat_class(radius: f32) -> theme::Button {
    let style = move |hovered: bool, theme: &cosmic::Theme| {
        let c = crate::central::widgets::card(theme.cosmic());
        widget::button::Style {
            background: hovered.then(|| Background::Color(c.hover.into())),
            border_radius: radius.into(),
            text_color: Some(c.on.into()),
            icon_color: Some(c.on.into()),
            ..widget::button::Style::new()
        }
    };
    theme::Button::Custom {
        active: Box::new(move |_, t| style(false, t)),
        disabled: Box::new(move |t| style(false, t)),
        hovered: Box::new(move |_, t| style(true, t)),
        pressed: Box::new(move |_, t| style(true, t)),
    }
}

/// Pílula com texto próprio (o `row::pill` só aceita `&str` emprestado).
fn owned_pill(text: String, tone: Tone) -> Element<'static, Message> {
    widget::container(widget::text(text).size(12.5).font(cosmic::font::semibold()))
        .padding([2, theme::spacing().space_xxs])
        .class(theme::Container::custom(move |theme| {
            let (bg, fg) = row::tone_colors(theme, tone);
            widget::container::Style {
                background: Some(Background::Color(bg)),
                text_color: Some(fg),
                border: iced::Border { radius: theme.cosmic().radius_xl().into(), ..Default::default() },
                ..Default::default()
            }
        }))
        .into()
}

// ── Trilho ───────────────────────────────────────────────────────────────

fn rail(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let current = category_of(app.section);
    let mut col = widget::column::with_capacity(Category::ALL.len() + 2)
        .spacing(spacing.space_xxs)
        .align_x(Alignment::Center)
        .width(Length::Fill)
        .height(Length::Fill);
    for cat in Category::ALL {
        col = col.push(rail_item(app, cat, cat == current));
    }
    col = col.push(widget::space::vertical().height(Length::Fill));
    col = col.push(widget::tooltip(
        button::icon(icon("view-refresh-symbolic"))
            .icon_size(17)
            .padding(spacing.space_xs)
            .class(bars::row_class())
            .on_press(Message::RefreshAll),
        widget::text::caption("Atualizar status"),
        widget::tooltip::Position::Right,
    ));
    widget::container(col)
        .width(Length::Fixed(RAIL_WIDTH))
        .height(Length::Fill)
        .padding([spacing.space_s, 0])
        .class(theme::Container::custom(|theme| {
            let bg = theme.cosmic().background(theme.transparent);
            let mut div: Color = bg.divider.into();
            div.a *= 0.65;
            widget::container::Style {
                background: Some(Background::Color(bg.base.into())),
                border: iced::Border { width: 1.0, color: div, ..Default::default() },
                ..Default::default()
            }
        }))
        .into()
}

fn rail_item(app: &ControlPanel, cat: Category, selected: bool) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let alert = cat != Category::Home && app.category_alert(cat);
    let icon_el = widget::icon::icon(icon(cat.icon())).size(RAIL_ICON);
    let pill = widget::container(icon_el)
        .width(Length::Fixed(RAIL_PILL.0))
        .height(Length::Fixed(RAIL_PILL.1))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .class(theme::Container::custom(move |theme| {
            let accent: Color = theme.cosmic().accent_color().into();
            widget::container::Style {
                background: selected.then(|| Background::Color(Color { a: 0.22, ..accent })),
                icon_color: selected.then_some(accent),
                border: iced::Border {
                    radius: (RAIL_PILL.1 / 2.0).into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        }));
    let mut layers: Vec<Element<'_, Message>> = vec![pill.into()];
    if alert {
        layers.push(
            widget::container(widget::container(widget::Space::new().width(8.0).height(8.0)).class(
                theme::Container::custom(|theme| widget::container::Style {
                    background: Some(Background::Color(theme.cosmic().warning_color().into())),
                    border: iced::Border { radius: 4.0.into(), ..Default::default() },
                    ..Default::default()
                }),
            ))
            .width(Length::Fixed(RAIL_PILL.0))
            .height(Length::Fixed(RAIL_PILL.1))
            .align_x(Alignment::End)
            .align_y(Alignment::Start)
            .into(),
        );
    }
    let label = widget::text(cat.label())
        .size(RAIL_LABEL_SIZE)
        .font(cosmic::font::semibold())
        .align_x(Alignment::Center);
    let label = if selected { label.class(theme::Text::Custom(accent_text)) } else { label };
    let content = widget::column::with_children(vec![
        iced::widget::stack(layers)
            .width(Length::Fixed(RAIL_PILL.0))
            .height(Length::Fixed(RAIL_PILL.1))
            .into(),
        label.into(),
    ])
    .spacing(spacing.space_xxxs)
    .align_x(Alignment::Center);
    button::custom(content)
        .padding([spacing.space_xxs, 0])
        .width(Length::Fixed(RAIL_ITEM_WIDTH))
        .class(flat_class(12.0))
        .on_press(Message::SelectCategory(cat))
        .into()
}

// ── Página ───────────────────────────────────────────────────────────────

fn page(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let content: Element<'_, Message> = if app.section == Section::Home {
        widget::container(home::home_view(app))
            .max_width(HOME_MAX_WIDTH)
            .width(Length::Fill)
            .into()
    } else {
        widget::container(section_page(app))
            .max_width(PAGE_MAX_WIDTH)
            .width(Length::Fill)
            .into()
    };
    let scroll = widget::scrollable(
        widget::container(content)
            .padding([spacing.space_m, spacing.space_l])
            .center_x(Length::Fill),
    )
    .id(super::page_scroll_id())
    .width(Length::Fill)
    .height(Length::Fill);

    let mut col = widget::column::with_capacity(2).push(scroll);
    if let Some(bar) = bars::apply_bar(app) {
        col = col.push(
            widget::container(bar).padding([spacing.space_xs, spacing.space_l]).width(Length::Fill),
        );
    }
    col.width(Length::Fill).height(Length::Fill).into()
}

fn section_page(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let mut col = widget::column::with_capacity(6)
        .spacing(spacing.space_m)
        .push(trail_and_tabs(app));
    // A Tela de Login desenha o próprio título e botões dentro de `login_view` (intacto).
    if app.section != Section::Login {
        col = col.push(page_header(app));
    }
    if let Some(jump) = jump_row(app) {
        col = col.push(jump);
    }
    col = col.push(body(app));
    if let Some(toggle) = advanced_toggle(app) {
        col = col.push(toggle);
    }
    col.into()
}

/// Trilha "Início › Categoria" e abas sublinhadas das seções da categoria (quebram linha).
fn trail_and_tabs(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let cat = category_of(app.section);
    let crumbs = widget::row::with_children(vec![
        button::custom(widget::text::caption("Início").class(theme::Text::Custom(dim)))
            .padding(0)
            .class(theme::Button::Link)
            .on_press(Message::SelectCategory(Category::Home))
            .into(),
        widget::text::caption("›").class(theme::Text::Custom(dim)).into(),
        widget::text::caption(cat.label()).class(theme::Text::Custom(dim)).into(),
    ])
    .spacing(spacing.space_xxs)
    .align_y(Alignment::Center);

    let tabs: Vec<Element<'_, Message>> = cat
        .sections()
        .iter()
        .map(|&s| tab(s, s == app.section))
        .collect();
    let tab_row = widget::row::with_children(tabs)
        .spacing(spacing.space_m)
        .wrap()
        .vertical_spacing(spacing.space_xxs);

    widget::column::with_children(vec![crumbs.into(), tab_row.into(), widget::divider::horizontal::default().into()])
        .spacing(spacing.space_xxs)
        .into()
}

fn tab(section: Section, selected: bool) -> Element<'static, Message> {
    let spacing = theme::spacing();
    let mut label = widget::text::body(section.label());
    if selected {
        label = label.class(theme::Text::Custom(accent_text)).font(cosmic::font::semibold());
    }
    let content = widget::row::with_children(vec![
        widget::icon::icon(icon(section.icon())).size(TAB_ICON).into(),
        label.into(),
    ])
    .spacing(spacing.space_xxs)
    .align_y(Alignment::Center);
    let underline = widget::container(widget::Space::new().height(TAB_UNDERLINE))
        .width(Length::Fill)
        .class(theme::Container::custom(move |theme| widget::container::Style {
            background: selected.then(|| Background::Color(theme.cosmic().accent_color().into())),
            ..Default::default()
        }));
    widget::column::with_children(vec![
        button::custom(content)
            .padding([spacing.space_xxs, spacing.space_xxxs])
            .class(flat_class(8.0))
            .on_press(Message::SelectSection(section))
            .into(),
        underline.into(),
    ])
    .into()
}

fn page_header(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let sec = app.section;
    let badge = page_badge(app);
    let actions = page_actions(app);

    let icon_box = widget::container(widget::icon::icon(icon(sec.icon())).size(22))
        .width(Length::Fixed(PAGE_ICON_BOX))
        .height(Length::Fixed(PAGE_ICON_BOX))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .class(theme::Container::custom(|theme| {
            let accent: Color = theme.cosmic().accent_color().into();
            widget::container::Style {
                background: Some(Background::Color(Color { a: 0.14, ..accent })),
                icon_color: Some(accent),
                border: iced::Border { radius: theme.cosmic().radius_s().into(), ..Default::default() },
                ..Default::default()
            }
        }));

    let mut title = widget::row::with_capacity(2)
        .spacing(spacing.space_xs)
        .align_y(Alignment::Center)
        .push(widget::text(sec.label()).size(TITLE_SIZE).font(cosmic::font::bold()));
    if let Some((text, tone)) = badge {
        title = title.push(owned_pill(text, tone));
    }
    let text_col = widget::column::with_children(vec![
        title.into(),
        widget::text(section_subtitle(sec))
            .size(SUBTITLE_SIZE)
            .class(theme::Text::Custom(dim))
            .into(),
    ])
    .spacing(spacing.space_xxxs)
    .width(Length::Fill);

    let inline = actions.len() <= MAX_INLINE_ACTIONS;
    let mut head = widget::row::with_capacity(3)
        .spacing(spacing.space_s)
        .align_y(Alignment::Start)
        .push(icon_box)
        .push(text_col);
    if inline && !actions.is_empty() {
        head = head.push(
            widget::row::with_children(actions).spacing(spacing.space_xxs).align_y(Alignment::Center),
        );
        return head.into();
    }
    if actions.is_empty() {
        return head.into();
    }
    widget::column::with_children(vec![
        head.into(),
        widget::row::with_children(actions)
            .spacing(spacing.space_xxs)
            .wrap()
            .vertical_spacing(spacing.space_xxs)
            .into(),
    ])
    .spacing(spacing.space_xs)
    .into()
}

/// "Ir para": um atalho por grupo comum da seção (grupos avançados só aparecem abertos).
fn jump_row(app: &ControlPanel) -> Option<Element<'_, Message>> {
    let spacing = theme::spacing();
    let groups = chrome::section_groups(app.section);
    let shown: Vec<&'static str> = groups
        .iter()
        .filter(|(_, adv)| app.advanced_open || !adv)
        .map(|(g, _)| *g)
        .collect();
    if shown.len() < 2 {
        return None;
    }
    let mut chips: Vec<Element<'_, Message>> = vec![widget::text::caption("Ir para")
        .class(theme::Text::Custom(dim))
        .into()];
    for g in shown {
        chips.push(
            button::custom(widget::text::caption(g))
                .padding([spacing.space_xxxs, spacing.space_xs])
                .class(chip_class(app.scroll_to == Some(g)))
                .on_press(Message::JumpTo(g))
                .into(),
        );
    }
    Some(
        widget::row::with_children(chips)
            .spacing(spacing.space_xxs)
            .align_y(Alignment::Center)
            .wrap()
            .vertical_spacing(spacing.space_xxs)
            .into(),
    )
}

/// "Mostrar/Ocultar ajustes avançados", com a lista dos grupos que ele revela.
fn advanced_toggle(app: &ControlPanel) -> Option<Element<'_, Message>> {
    let names = chrome::advanced_groups(app.section);
    if names.is_empty() {
        return None;
    }
    let spacing = theme::spacing();
    let open = app.advanced_open;
    let badge = widget::container(widget::icon::icon(icon("preferences-other-symbolic")).size(TAB_ICON))
        .width(Length::Fixed(ADVANCED_ICON_BOX))
        .height(Length::Fixed(ADVANCED_ICON_BOX))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .class(theme::Container::custom(|theme| {
            let warn: Color = theme.cosmic().warning_color().into();
            widget::container::Style {
                background: Some(Background::Color(Color { a: 0.14, ..warn })),
                icon_color: Some(warn),
                border: iced::Border { radius: theme.cosmic().radius_s().into(), ..Default::default() },
                ..Default::default()
            }
        }));
    let content = widget::row::with_children(vec![
        badge.into(),
        widget::column::with_children(vec![
            widget::text(if open { "Ocultar ajustes avançados" } else { "Mostrar ajustes avançados" })
                .size(14.0)
                .font(cosmic::font::semibold())
                .into(),
            widget::text(names.join(" · ")).size(12.5).class(theme::Text::Custom(dim)).into(),
        ])
        .spacing(spacing.space_xxxs)
        .width(Length::Fill)
        .into(),
        widget::icon::icon(icon(if open { "go-up-symbolic" } else { "go-down-symbolic" }))
            .size(TAB_ICON)
            .into(),
    ])
    .spacing(spacing.space_xs)
    .align_y(Alignment::Center);
    Some(
        button::custom(content)
            .padding([spacing.space_xs, spacing.space_s])
            .width(Length::Fill)
            .class(bars::row_class())
            .on_press(Message::ToggleAdvanced)
            .into(),
    )
}

// ── Despacho por seção ───────────────────────────────────────────────────

fn body(app: &ControlPanel) -> Element<'_, Message> {
    match app.section {
        Section::Home => home::home_view(app),
        Section::Capture => capture::capture_view(app),
        Section::Power => power::power_view(app),
        Section::Bar => bar::bar_view(app),
        Section::Monitor => monitor::monitor_view(app),
        Section::Central => central::central_view(app),
        Section::Notifications => notifications::notifications_view(app),
        Section::DevicesOsd => osd::devices_osd_view(app),
        Section::Camera => camera::camera_view(app),
        Section::Network => network::network_view(app),
        Section::Security => security::security_view(app),
        Section::Backup => backup::backup_view(app),
        Section::Displays => displays::displays_view(app),
        Section::Login => login::login_view(app),
        Section::Optional => optional::optional_view(app),
        Section::System => system::system_view(app),
    }
}

/// Botões do cabeçalho da página. A Tela de Login mantém os dela dentro de `login_view`.
fn page_actions(app: &ControlPanel) -> Vec<Element<'_, Message>> {
    match app.section {
        Section::Home | Section::Login => Vec::new(),
        Section::Capture => capture::capture_actions(app),
        Section::Power => power::power_actions(app),
        Section::Bar => bar::bar_actions(app),
        Section::Monitor => monitor::monitor_actions(app),
        Section::Central => central::central_actions(app),
        Section::Notifications => notifications::notifications_actions(app),
        Section::DevicesOsd => osd::devices_osd_actions(app),
        Section::Camera => camera::camera_actions(app),
        Section::Network => network::network_actions(app),
        Section::Security => security::security_actions(app),
        Section::Backup => backup::backup_actions(app),
        Section::Displays => displays::displays_actions(app),
        Section::Optional => optional::optional_actions(app),
        Section::System => system::system_actions(app),
    }
}

/// Selo de estado ao lado do título (texto e tom).
fn page_badge(app: &ControlPanel) -> Option<(String, Tone)> {
    match app.section {
        Section::Home | Section::Login => None,
        Section::Capture => capture::capture_badge(app),
        Section::Power => power::power_badge(app),
        Section::Bar => bar::bar_badge(app),
        Section::Monitor => monitor::monitor_badge(app),
        Section::Central => central::central_badge(app),
        Section::Notifications => notifications::notifications_badge(app),
        Section::DevicesOsd => osd::devices_osd_badge(app),
        Section::Camera => camera::camera_badge(app),
        Section::Network => network::network_badge(app),
        Section::Security => security::security_badge(app),
        Section::Backup => backup::backup_badge(app),
        Section::Displays => displays::displays_badge(app),
        Section::Optional => optional::optional_badge(app),
        Section::System => system::system_badge(app),
    }
}

/// Botão de ação do cabeçalho; `enabled` = `false` (ação de fundo rodando) deixa-o sem clique.
pub(super) fn action_button<'a>(
    label: &'a str,
    msg: Message,
    style: BtnStyle,
    enabled: bool,
) -> Element<'a, Message> {
    let msg = enabled.then_some(msg);
    match style {
        BtnStyle::Primary => widget::button::suggested(label).on_press_maybe(msg).into(),
        BtnStyle::Normal => widget::button::standard(label).on_press_maybe(msg).into(),
        BtnStyle::Danger => widget::button::destructive(label).on_press_maybe(msg).into(),
    }
}

/// Subtítulo da página (cópia do de `legacy`, que fica até o Task 15).
fn section_subtitle(sec: Section) -> &'static str {
    match sec {
        Section::Home => "Resumo do estado do sistema e atalhos.",
        Section::Capture => "Atalhos e comportamento da captura de tela.",
        Section::Power => "Perfis de energia, bateria e tempos de tela/suspensão.",
        Section::Bar => "Ícones da barra direita, espaçamento e apps em segundo plano.",
        Section::Monitor => "Cartões do painel de medições à esquerda e tempos de atualização.",
        Section::Central => "Seções e atalhos exibidos ao abrir o Central pelo relógio.",
        Section::Notifications => "Posição, estilo, duração e comportamento dos avisos flutuantes.",
        Section::DevicesOsd => "Avisos visuais na tela para teclado e mouse Logitech.",
        Section::Camera => "Câmeras conectadas, microfone e privacidade de vídeo/áudio.",
        Section::Network => "Conexão ativa, Wi-Fi, IP e diagnóstico rápido de rede.",
        Section::Security => "Firewall, DNS criptografado, privacidade agora e blindagem.",
        Section::Backup => "Backup diário Restic, snapshots Btrfs e sincronização.",
        Section::Displays => "Brilho, escala, folga de janela dividida e foco de janelas.",
        Section::Login => "Aparência da tela de bloqueio e login no boot.",
        Section::Optional => "Componentes opcionais e integrações extras do ambiente.",
        Section::System => "Serviços em segundo plano, atualizações e saúde do sistema.",
    }
}
