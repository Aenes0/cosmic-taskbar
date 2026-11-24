use super::*;

/// A search result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Hit {
    /// Instant math expression result: `(expression, formatted_answer)`.
    Calc(String, String),
    /// Quick system command: `(icon_name, label, action_id)`.
    Action(&'static str, &'static str, &'static str),
    App(usize),
    Window(String),
    File(usize),
}

struct QuickAction {
    id: &'static str,
    icon: &'static str,
    label: &'static str,
    keywords: &'static [&'static str],
}

const QUICK_ACTIONS: &[QuickAction] = &[
    QuickAction {
        id: "control-panel",
        icon: "preferences-system-symbolic",
        label: "Painel de Controle",
        keywords: &["painel", "controle", "control panel", "ajustes", "sistema"],
    },
    QuickAction {
        id: "lock",
        icon: "system-lock-screen-symbolic",
        label: "Bloquear tela",
        keywords: &["bloquear", "trancar", "lock"],
    },
    QuickAction {
        id: "suspend",
        icon: "system-suspend-symbolic",
        label: "Suspender computador",
        keywords: &["suspender", "dormir", "sleep", "suspend"],
    },
    QuickAction {
        id: "restart",
        icon: "system-reboot-symbolic",
        label: "Reiniciar computador",
        keywords: &["reiniciar", "reboot", "restart"],
    },
    QuickAction {
        id: "shutdown",
        icon: "system-shutdown-symbolic",
        label: "Desligar computador",
        keywords: &["desligar", "shutdown", "poweroff"],
    },
    QuickAction {
        id: "restart-panel",
        icon: "view-refresh-symbolic",
        label: "Reiniciar barra",
        keywords: &["reiniciar barra", "reiniciar painel", "reload panel"],
    },
];

/// Evaluates simple arithmetic expressions (`+ - * / % ^ ( )`, PT-BR `,` decimal)
/// when the query contains at least one operator.
fn eval_calc(raw: &str) -> Option<String> {
    let s = raw.trim().replace(',', ".");
    if s.is_empty() || !s.chars().any(|c| matches!(c, '+' | '-' | '*' | '/' | '%' | '^')) {
        return None;
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '.' | '+' | '-' | '*' | '/' | '%' | '^' | '(' | ')' | ' '))
    {
        return None;
    }
    let bytes = s.as_bytes();
    let mut pos = 0usize;
    let val = parse_expr(bytes, &mut pos)?;
    skip_ws(bytes, &mut pos);
    if pos != bytes.len() || !val.is_finite() {
        return None;
    }
    let rounded = (val * 1_000_000.0).round() / 1_000_000.0;
    if (rounded - rounded.round()).abs() < 1e-9 && rounded.abs() < 1e15 {
        Some(format!("{}", rounded.round() as i64))
    } else {
        let formatted = format!("{rounded:.6}");
        Some(formatted.trim_end_matches('0').trim_end_matches('.').to_owned())
    }
}

fn skip_ws(b: &[u8], pos: &mut usize) {
    while *pos < b.len() && b[*pos].is_ascii_whitespace() {
        *pos += 1;
    }
}

fn parse_expr(b: &[u8], pos: &mut usize) -> Option<f64> {
    let mut left = parse_term(b, pos)?;
    loop {
        skip_ws(b, pos);
        match b.get(*pos) {
            Some(b'+') => {
                *pos += 1;
                left += parse_term(b, pos)?;
            }
            Some(b'-') => {
                *pos += 1;
                left -= parse_term(b, pos)?;
            }
            _ => return Some(left),
        }
    }
}

fn parse_term(b: &[u8], pos: &mut usize) -> Option<f64> {
    let mut left = parse_power(b, pos)?;
    loop {
        skip_ws(b, pos);
        match b.get(*pos) {
            Some(b'*') => {
                *pos += 1;
                left *= parse_power(b, pos)?;
            }
            Some(b'/') => {
                *pos += 1;
                let r = parse_power(b, pos)?;
                if r == 0.0 {
                    return None;
                }
                left /= r;
            }
            Some(b'%') => {
                *pos += 1;
                let r = parse_power(b, pos)?;
                if r == 0.0 {
                    return None;
                }
                left %= r;
            }
            _ => return Some(left),
        }
    }
}

fn parse_power(b: &[u8], pos: &mut usize) -> Option<f64> {
    let base = parse_unary(b, pos)?;
    skip_ws(b, pos);
    if b.get(*pos) == Some(&b'^') {
        *pos += 1;
        let exp = parse_power(b, pos)?;
        Some(base.powf(exp))
    } else {
        Some(base)
    }
}

fn parse_unary(b: &[u8], pos: &mut usize) -> Option<f64> {
    skip_ws(b, pos);
    match b.get(*pos) {
        Some(b'+') => {
            *pos += 1;
            parse_unary(b, pos)
        }
        Some(b'-') => {
            *pos += 1;
            Some(-parse_unary(b, pos)?)
        }
        _ => parse_primary(b, pos),
    }
}

fn parse_primary(b: &[u8], pos: &mut usize) -> Option<f64> {
    skip_ws(b, pos);
    if b.get(*pos) == Some(&b'(') {
        *pos += 1;
        let v = parse_expr(b, pos)?;
        skip_ws(b, pos);
        if b.get(*pos) != Some(&b')') {
            return None;
        }
        *pos += 1;
        return Some(v);
    }
    let start = *pos;
    let mut dot = false;
    while *pos < b.len() {
        let ch = b[*pos];
        if ch.is_ascii_digit() {
            *pos += 1;
        } else if ch == b'.' && !dot {
            dot = true;
            *pos += 1;
        } else {
            break;
        }
    }
    if *pos == start {
        return None;
    }
    std::str::from_utf8(&b[start..*pos]).ok()?.parse::<f64>().ok()
}

fn quick_action_message(action_id: &'static str) -> Message {
    match action_id {
        "control-panel" => Message::Tool(crate::start::tools::Tool::ControlPanel),
        "lock" => Message::Spawn("loginctl", vec!["lock-session".into()]),
        "suspend" => Message::Spawn("systemctl", vec!["suspend".into()]),
        "restart" => Message::Spawn("cosmic-osd", vec!["restart".into()]),
        "shutdown" => Message::Spawn("cosmic-osd", vec!["shutdown".into()]),
        "restart-panel" => Message::Spawn("reiniciar-painel", Vec::new()),
        _ => Message::Nothing,
    }
}

impl StartMenu {
    pub(super) fn open_hit(&mut self, hit: Hit) -> app::Task<Message> {
        match hit {
            Hit::Calc(_, answer) => cosmic::Application::update(self, Message::CopyText(answer)),
            Hit::Action(_, _, id) => cosmic::Application::update(self, quick_action_message(id)),
            Hit::App(i) => self.launch(i, None),
            Hit::Window(id) => self.activate(id),
            Hit::File(i) => {
                let path = self.recents[i].path.clone();
                cosmic::Application::update(self, Message::OpenFile(path))
            }
        }
    }

    pub(super) fn hits(&self) -> Vec<Hit> {
        let raw = self.query.trim();
        let query = fold(raw);
        if query.is_empty() {
            return Vec::new();
        }
        let mut hits: Vec<Hit> = Vec::new();
        if let Some(answer) = eval_calc(raw) {
            hits.push(Hit::Calc(raw.to_owned(), answer));
        }
        if query.len() >= 3 {
            for qa in QUICK_ACTIONS {
                if qa.keywords.iter().any(|k| k.starts_with(&query) || query.contains(k))
                    || fold(qa.label).starts_with(&query)
                {
                    hits.push(Hit::Action(qa.icon, qa.label, qa.id));
                }
            }
        }
        let mut scored: Vec<(u8, &str, Hit)> = self
            .apps
            .iter()
            .enumerate()
            .filter_map(|(i, app)| {
                let score = if app.folded_name.starts_with(&query) {
                    0
                } else if app.folded_name.split([' ', '-', '_', '.']).any(|w| w.starts_with(&query)) {
                    1
                } else if app.folded_name.contains(&query) {
                    2
                } else if app.haystack.contains(&query) {
                    3
                } else {
                    return None;
                };
                Some((score, app.name.as_str(), Hit::App(i)))
            })
            .collect();
        // The closest first: "Configurações" before "Configurações do NVIDIA".
        scored.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| a.1.chars().count().cmp(&b.1.chars().count()))
                .then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase()))
        });
        hits.extend(scored.into_iter().map(|(_, _, hit)| hit).take(10));
        hits.extend(
            self.windows
                .iter()
                .filter(|w| fold(&w.title).contains(&query) || fold(&w.app_id).contains(&query))
                .map(|w| Hit::Window(w.id.clone())),
        );
        hits.extend(
            self.recents
                .iter()
                .enumerate()
                .filter(|(_, f)| fold(&f.name).contains(&query))
                .map(|(i, _)| Hit::File(i))
                .take(8),
        );
        hits
    }

    pub(super) fn search_bar(&self, p: Palette) -> Element<'_, Message> {
        let style = move |focused: bool| widget::text_input::Appearance {
            background: Background::Color(p.field),
            border_radius: 18.0.into(),
            border_offset: None,
            border_width: 1.0,
            border_color: if focused { p.accent } else { p.border },
            label_color: p.text,
            placeholder_color: p.dim,
            selected_text_color: Color::WHITE,
            icon_color: Some(p.dim),
            text_color: Some(p.text),
            selected_fill: p.accent,
        };
        widget::search_input(texts().start_search, &self.query)
            .id(SEARCH.clone())
            .size(14)
            .padding([8, 12])
            .on_input(Message::Query)
            .on_submit(|_| Message::Submit)
            .width(Length::Fill)
            .style(theme::TextInput::Custom {
                active: Box::new(move |_| style(false)),
                error: Box::new(move |_| style(false)),
                hovered: Box::new(move |_| style(false)),
                focused: Box::new(move |_| style(true)),
                disabled: Box::new(move |_| style(false)),
            })
            .into()
    }

    /// Results at the left, the chosen one at the right with what can be
    /// done with it, as Windows' search has them. On a narrow menu, only
    /// the list.
    pub(super) fn results(&self, p: Palette) -> Element<'_, Message> {
        let t = texts();
        let hits = self.hits();
        let Some(chosen) = hits.get(self.selected.min(hits.len().saturating_sub(1))) else {
            return widget::container(widget::text::body(t.no_results).class(theme::Text::Color(p.dim)))
                .padding([40, 0])
                .center_x(Length::Fill)
                .into();
        };
        let mut list = widget::column::with_capacity(hits.len() + 4).spacing(2);
        let mut section = "";
        for (i, hit) in hits.iter().enumerate() {
            let Some((heading, row)) = self.hit_row(i, hit, p) else { continue };
            let heading = if i == 0 { t.best_match } else { heading };
            if heading != section {
                section = heading;
                list = list.push(
                    widget::container(widget::text::heading(heading)).padding([if i == 0 { 0 } else { 10 }, 10, 4, 10]),
                );
            }
            list = list.push(row);
        }
        let pad = [4.0, self.metrics.side, 20.0, self.metrics.side];
        if !self.metrics.detail_panel {
            return widget::container(widget::scrollable(list).width(Length::Fill).height(Length::Fill))
                .padding(pad)
                .into();
        }
        widget::row::with_children(vec![
            widget::scrollable(list).width(Length::FillPortion(6)).height(Length::Fill).into(),
            self.detail_panel(chosen, p),
        ])
        .spacing(16)
        .padding(pad)
        .into()
    }

    /// One line of the results, with the right click of its context menu.
    fn hit_row(&self, i: usize, hit: &Hit, p: Palette) -> Option<(&'static str, Element<'_, Message>)> {
        let t = texts();
        let (heading, icon, title, message, target, key) = match hit {
            Hit::Calc(expr, answer) => (
                "Calculadora",
                symbolic("accessories-calculator-symbolic"),
                format!("{expr} = {answer}"),
                Message::CopyText(answer.clone()),
                None,
                format!("calc:{answer}"),
            ),
            Hit::Action(icon_name, label, id) => (
                "Ação rápida",
                symbolic(icon_name),
                (*label).to_owned(),
                quick_action_message(id),
                None,
                format!("action:{id}"),
            ),
            Hit::App(index) => {
                let app = &self.apps[*index];
                (t.apps, app.icon.clone(), app.name.clone(), Message::Launch(app.id.clone()), Some(Target::App(app.id.clone())), app.id.clone())
            }
            Hit::Window(id) => {
                let window = self.windows.iter().find(|w| w.id == *id)?;
                (t.open_windows, self.window_icon(window), window.title.clone(), Message::Activate(id.clone()), Some(Target::Window(id.clone())), id.clone())
            }
            Hit::File(index) => {
                let file = &self.recents[*index];
                (t.documents, self.file_icon_of(file), file.name.clone(), Message::OpenFile(file.path.clone()), Some(Target::File(file.path.clone())), file.path.clone())
            }
        };
        let size = px(if i == 0 { 32.0 } else { 20.0 });
        let content = widget::row::with_children(vec![
            widget::icon(icon).size(size).into(),
            widget::text::body(title).wrapping(Wrapping::None).width(Length::Fill).into(),
        ])
        .spacing(12)
        .align_y(Alignment::Center);
        let id = scoped("hit", &key);
        let button = button::custom(widget::container(content).clip(true))
            .padding(if i == 0 { [10, 10] } else { [7, 10] })
            .width(Length::Fill)
            .class(flat_class(p, i == self.selected))
            .on_press(message);
        let row: Element<'_, Message> = if let Some(target) = target {
            widget::mouse_area(button)
                .on_right_press(Message::MenuFor(target, id.clone()))
                .into()
        } else {
            button.into()
        };
        Some((heading, widget::container(row).width(Length::Fill).id(id).into()))
    }

    /// The chosen one, large, and its actions.
    fn detail_panel(&self, chosen: &Hit, p: Palette) -> Element<'_, Message> {
        let t = texts();
        let (icon, title, kind, actions): (icon::Handle, String, String, Vec<(String, Message)>) = match chosen {
            Hit::Calc(expr, answer) => (
                symbolic("accessories-calculator-symbolic"),
                answer.clone(),
                format!("Resultado de {expr}"),
                vec![("Copiar resultado".to_owned(), Message::CopyText(answer.clone()))],
            ),
            Hit::Action(icon_name, label, id) => (
                symbolic(icon_name),
                (*label).to_owned(),
                "Ação rápida do sistema".to_owned(),
                vec![("Executar agora".to_owned(), quick_action_message(id))],
            ),
            Hit::App(index) => {
                let app = &self.apps[*index];
                let pinned = self.pinned.iter().any(|id| *id == app.id);
                let mut actions = vec![(t.open_app.to_owned(), Message::Launch(app.id.clone()))];
                actions.extend(app.actions.iter().map(|(name, exec)| {
                    (name.clone(), Message::LaunchAction(app.id.clone(), exec.clone()))
                }));
                actions.push((
                    if pinned { t.unpin_start } else { t.pin_start }.to_owned(),
                    Message::TogglePin(app.id.clone()),
                ));
                if uninstall_ui::uninstall_visible(app) {
                    actions.push((t.uninstall.to_owned(), Message::AskUninstall(app.id.clone())));
                }
                (app.icon.clone(), app.name.clone(), t.app_kind.to_owned(), actions)
            }
            Hit::Window(id) => {
                let window = self.windows.iter().find(|w| w.id == *id);
                (
                    window.map(|w| self.window_icon(w)).unwrap_or_else(|| self.generic_icon.clone()),
                    window.map(|w| w.title.clone()).unwrap_or_default(),
                    t.window_kind.to_owned(),
                    vec![
                        (t.switch_to.to_owned(), Message::Activate(id.clone())),
                        (t.close_window.to_owned(), Message::CloseWindow(id.clone())),
                    ],
                )
            }
            Hit::File(index) => {
                let file = &self.recents[*index];
                (
                    self.file_icon_of(file),
                    file.name.clone(),
                    format!("{} · {}", t.file_kind, ago(file.when)),
                    vec![
                        (t.open_app.to_owned(), Message::OpenFile(file.path.clone())),
                        (t.open_location.to_owned(), Message::OpenLocation(file.path.clone())),
                    ],
                )
            }
        };
        let mut detail = widget::column::with_capacity(actions.len() + 4)
            .push(widget::icon(icon).size(px(64.0)))
            .push(
                widget::text::title4(title)
                    .align_x(Alignment::Center)
                    .width(Length::Fill),
            )
            .push(widget::text::caption(kind).class(theme::Text::Color(p.dim)))
            .push(widget::space::vertical().height(Length::Fixed(8.0)))
            .spacing(6)
            .padding([24, 12])
            .align_x(Alignment::Center);
        for (label, message) in actions {
            let row = widget::row::with_children(vec![
                widget::icon::from_name("go-next-symbolic").size(12).into(),
                widget::text::body(label).into(),
            ])
            .spacing(12)
            .align_y(Alignment::Center);
            detail = detail.push(
                button::custom(row)
                    .padding([8, 12])
                    .width(Length::Fill)
                    .class(flat_class(p, false))
                    .on_press(message),
            );
        }
        widget::container(detail)
            .width(Length::FillPortion(5))
            .height(Length::Fill)
            .class(theme::Container::custom(move |_| widget::container::Style {
                background: Some(Background::Color(p.card)),
                border: iced::Border {
                    radius: 8.0.into(),
                    width: 1.0,
                    color: p.border,
                },
                ..Default::default()
            }))
            .into()
    }
}

#[cfg(test)]
mod calc_tests {
    use super::eval_calc;

    #[test]
    fn evaluates_basic_and_br_decimal_expressions() {
        assert_eq!(eval_calc("2 + 2").as_deref(), Some("4"));
        assert_eq!(eval_calc("1500 * 0,85").as_deref(), Some("1275"));
        assert_eq!(eval_calc("(240 + 60) / 4").as_deref(), Some("75"));
        assert_eq!(eval_calc("2^10").as_deref(), Some("1024"));
    }

    #[test]
    fn ignores_plain_numbers_and_text() {
        assert_eq!(eval_calc("2026"), None);
        assert_eq!(eval_calc("firefox"), None);
        assert_eq!(eval_calc("10 / 0"), None);
    }
}
