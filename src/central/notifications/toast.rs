// SPDX-License-Identifier: GPL-3.0-only

//! Floating Vulkan `layer-shell` notification toasts rendered by `central --panels`
//! using 100% of the user's `NotifConfig` (duration, position, format, opacity,
//! corner radius, accent & background colors, body lines, action buttons, and
//! `image-path` thumbnails for `Win+Shift+S` screen captures).

use std::time::{Duration, Instant};

use cosmic::{
    Element,
    iced::{
        self, Alignment, Background, Color, Length,
        core::text::{Ellipsize, EllipsizeHeightLimit},
        platform_specific::{
            runtime::wayland::layer_surface::IcedMargin,
            shell::commands::layer_surface::Anchor,
        },
    },
    theme,
    widget::{self, button},
};

use super::{
    Notification,
    config::{NotifConfig, NotifFormat, NotifPosition},
};
use crate::central::{tiles::icon, widgets::{card, readable}};

/// Maximum simultaneous floating toasts stacked on screen.
pub const MAX_TOASTS: usize = 4;
/// Logical width of each toast card.
pub const TOAST_WIDTH: f32 = 368.0;

#[derive(Clone, Debug)]
pub struct Toast {
    pub notification: Notification,
    pub shown_at: Instant,
    /// Accumulated time spent paused while the pointer hovers over the toast stack.
    pub paused_for: Duration,
}

impl Toast {
    pub fn new(notification: Notification) -> Self {
        Self {
            notification,
            shown_at: Instant::now(),
            paused_for: Duration::ZERO,
        }
    }

    pub fn pause_step(&mut self, delta: Duration) {
        self.paused_for += delta;
    }

    pub fn is_expired(&self, timeout_secs: u32) -> bool {
        timeout_secs > 0
            && self.shown_at.elapsed().saturating_sub(self.paused_for)
                >= Duration::from_secs(u64::from(timeout_secs))
    }
}

/// Maps `NotifPosition` to Wayland `layer-shell` anchor + margins.
pub fn surface_placement(position: NotifPosition) -> (Anchor, IcedMargin) {
    match position {
        NotifPosition::TopRight => (
            Anchor::TOP | Anchor::RIGHT,
            IcedMargin { top: 16, right: 16, bottom: 16, left: 16 },
        ),
        NotifPosition::TopCenter => (
            Anchor::TOP,
            IcedMargin { top: 16, right: 16, bottom: 16, left: 16 },
        ),
        NotifPosition::BottomRight => (
            Anchor::BOTTOM | Anchor::RIGHT,
            IcedMargin { top: 16, right: 16, bottom: 56, left: 16 },
        ),
        NotifPosition::BottomLeft => (
            Anchor::BOTTOM | Anchor::LEFT,
            IcedMargin { top: 16, right: 16, bottom: 56, left: 16 },
        ),
    }
}

fn toast_card_style(cfg: &NotifConfig) -> theme::Container<'static> {
    let opacity = cfg.opacity.clamp(0.40, 1.00);
    let radius = cfg.corner_radius.clamp(4.0, 24.0);
    let bg_rgb = cfg.bg_rgb();
    let accent_rgb = cfg.accent_rgb();
    theme::Container::custom(move |theme| {
        let cosmic = theme.cosmic();
        let c = card(cosmic);
        let (bg, fg) = match bg_rgb {
            Some([r, g, b]) => {
                let base = Color::from_rgb(r, g, b);
                let fg = readable(c.on, base);
                (Color::from_rgba(r, g, b, opacity), fg)
            }
            None => {
                let mut base: Color = c.base.into();
                base.a = opacity;
                (base, c.on.into())
            }
        };
        let mut border_color: Color = match accent_rgb {
            Some([r, g, b]) => Color::from_rgb(r, g, b),
            None => cosmic.accent_color().into(),
        };
        border_color.a = 0.75;
        widget::container::Style {
            background: Some(Background::Color(bg)),
            text_color: Some(fg),
            icon_color: Some(fg),
            border: iced::Border {
                radius: radius.into(),
                width: 1.25,
                color: border_color,
            },
            shadow: crate::shared::tokens::popup_shadow(),
            ..Default::default()
        }
    })
}

fn action_pill_style(accent_rgb: Option<[f32; 3]>) -> theme::Button {
    let style = move |hovered: bool, theme: &cosmic::Theme| {
        let cosmic = theme.cosmic();
        let base: Color = match accent_rgb {
            Some([r, g, b]) => Color::from_rgb(r, g, b),
            None => cosmic.accent_color().into(),
        };
        let mut bg = base;
        bg.a = if hovered { 0.36 } else { 0.22 };
        button::Style {
            background: Some(Background::Color(bg)),
            border_radius: 8.0.into(),
            border_width: 1.0,
            border_color: Color { a: 0.55, ..base },
            text_color: Some(cosmic.background(theme.transparent).on.into()),
            icon_color: Some(cosmic.background(theme.transparent).on.into()),
            ..button::Style::new()
        }
    };
    theme::Button::Custom {
        active: Box::new(move |_, theme| style(false, theme)),
        disabled: Box::new(move |theme| style(false, theme)),
        hovered: Box::new(move |_, theme| style(true, theme)),
        pressed: Box::new(move |_, theme| style(true, theme)),
    }
}

/// Extracts a 2FA/OTP verification code (4–8 digits) when the notification text
/// clearly refers to a verification/login code.
pub fn extract_otp_code(summary: &str, body: &str) -> Option<String> {
    let combined = format!("{summary} {body}");
    let lower = combined.to_lowercase();
    const KEYWORDS: &[&str] = &[
        "código",
        "codigo",
        "code",
        "otp",
        "pin",
        "token",
        "verificação",
        "verificacao",
        "verification",
        "autenticação",
        "autenticacao",
        "2fa",
        "confirmação",
        "confirmacao",
        "acesso",
        "login",
    ];
    if !KEYWORDS.iter().any(|k| lower.contains(k)) {
        return None;
    }
    for raw in combined.split(|c: char| !c.is_ascii_alphanumeric() && c != '-') {
        let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
        // Match pure digit tokens or `123-456` split codes (4..=8 digits).
        if (4..=8).contains(&digits.len())
            && raw.chars().all(|c| c.is_ascii_digit() || c == '-')
            && !(digits.len() == 4 && (digits.starts_with("19") || digits.starts_with("20")))
        {
            return Some(digits);
        }
    }
    None
}

/// Renders the stack of active floating toast notifications.
pub fn view<'a, M: Clone + 'static>(
    toasts: &'a [Toast],
    cfg: &'a NotifConfig,
    on_dismiss: impl Fn(u32) -> M + Copy + 'a,
    on_invoke: impl Fn(u32, String) -> M + Copy + 'a,
    on_hover: impl Fn(bool) -> M + Copy + 'a,
    on_copy_otp: impl Fn(u32, String) -> M + Copy + 'a,
) -> Element<'a, M> {
    let gap = match cfg.format {
        NotifFormat::Full => 10,
        NotifFormat::Compact | NotifFormat::Minimal => 6,
    };
    let cards: Vec<Element<'a, M>> = toasts
        .iter()
        .map(|toast| render_one(&toast.notification, cfg, on_dismiss, on_invoke, on_copy_otp))
        .collect();
    let stack = widget::column::with_children(cards)
        .spacing(gap)
        .width(Length::Fixed(TOAST_WIDTH));
    widget::mouse_area(stack)
        .on_enter(on_hover(true))
        .on_exit(on_hover(false))
        .into()
}

fn render_one<'a, M: Clone + 'static>(
    n: &'a Notification,
    cfg: &'a NotifConfig,
    on_dismiss: impl Fn(u32) -> M + Copy + 'a,
    on_invoke: impl Fn(u32, String) -> M + Copy + 'a,
    on_copy_otp: impl Fn(u32, String) -> M + Copy + 'a,
) -> Element<'a, M> {
    let app_icon = if n.icon.starts_with('/') {
        widget::icon::from_path(n.icon.clone().into())
    } else if n.icon.is_empty() {
        icon("preferences-system-notifications-symbolic")
    } else {
        widget::icon::from_name(n.icon.clone()).handle()
    };
    let one_line = || Ellipsize::End(EllipsizeHeightLimit::Lines(1));
    let body_lines = || Ellipsize::End(EllipsizeHeightLimit::Lines(cfg.max_body_lines.clamp(1, 5) as usize));
    let accent_rgb = cfg.accent_rgb();
    let id = n.id;

    let close_btn: Element<'a, M> = button::icon(icon("window-close-symbolic"))
        .icon_size(14)
        .padding(4)
        .on_press(on_dismiss(id))
        .into();

    let pad = match cfg.format {
        NotifFormat::Full => [12, 14],
        NotifFormat::Compact => [8, 12],
        NotifFormat::Minimal => [6, 10],
    };

    if cfg.format == NotifFormat::Minimal {
        let title = if n.summary.is_empty() { &n.app } else { &n.summary };
        let row = widget::row::with_children(vec![
            widget::icon::icon(app_icon).size(18).into(),
            widget::text::body(title)
                .font(cosmic::font::semibold())
                .ellipsize(one_line())
                .width(Length::Fill)
                .into(),
            close_btn,
        ])
        .spacing(8)
        .align_y(Alignment::Center);
        return widget::container(row)
            .padding(pad)
            .width(Length::Fixed(TOAST_WIDTH))
            .class(toast_card_style(cfg))
            .into();
    }

    let mut rows: Vec<Element<'a, M>> = Vec::new();

    // Header: icon + app/summary + close
    let header_title = if cfg.format == NotifFormat::Compact {
        if n.summary.is_empty() {
            n.app.as_str()
        } else {
            n.summary.as_str()
        }
    } else if n.app.is_empty() {
        "Notificação"
    } else {
        n.app.as_str()
    };

    rows.push(
        widget::row::with_children(vec![
            widget::icon::icon(app_icon)
                .size(if cfg.format == NotifFormat::Compact { 18 } else { 20 })
                .into(),
            widget::text::caption(header_title)
                .font(cosmic::font::semibold())
                .ellipsize(one_line())
                .width(Length::Fill)
                .into(),
            close_btn,
        ])
        .spacing(8)
        .align_y(Alignment::Center)
        .into(),
    );

    if cfg.format == NotifFormat::Full && !n.summary.is_empty() {
        rows.push(
            widget::text::body(&n.summary)
                .font(cosmic::font::semibold())
                .ellipsize(one_line())
                .into(),
        );
    }

    if cfg.show_body && !n.body.is_empty() {
        rows.push(
            widget::text::caption(&n.body)
                .ellipsize(body_lines())
                .into(),
        );
    }

    // Thumbnail preview when `image-path` hint is present (e.g. Win+Shift+S screenshot)
    if let Some(path) = n.image_path.as_deref()
        && std::path::Path::new(path).is_file()
    {
        let thumb_h = if cfg.format == NotifFormat::Compact { 100.0 } else { 150.0 };
        let img = widget::image(widget::image::Handle::from_path(path))
            .width(Length::Fill)
            .height(Length::Fixed(thumb_h))
            .content_fit(iced::ContentFit::Contain)
            .border_radius(8.0);
        let default_action = n
            .actions
            .iter()
            .find(|(k, _)| k == "default")
            .or_else(|| n.actions.first());
        if let Some((key, _)) = default_action {
            rows.push(
                button::custom(img)
                    .padding(0)
                    .width(Length::Fill)
                    .class(theme::Button::Text)
                    .on_press(on_invoke(id, key.clone()))
                    .into(),
            );
        } else {
            rows.push(img.into());
        }
    }

    // Action buttons (including automatic 1-click "Copiar código" for 2FA/OTP notifications)
    let mut action_btns: Vec<Element<'a, M>> = Vec::new();
    if let Some(code) = extract_otp_code(&n.summary, &n.body) {
        let label = format!("Copiar {code}");
        action_btns.push(
            button::custom(widget::text::caption(label))
                .padding([5, 12])
                .class(action_pill_style(accent_rgb))
                .on_press(on_copy_otp(id, code))
                .into(),
        );
    }
    if cfg.show_actions && !n.actions.is_empty() {
        let named: Vec<&(String, String)> = n
            .actions
            .iter()
            .filter(|(k, l)| k != "default" && !l.is_empty())
            .collect();
        let to_show: Vec<&(String, String)> = if named.is_empty() {
            n.actions.iter().filter(|(_, l)| !l.is_empty()).collect()
        } else {
            named
        };
        let remaining = 3usize.saturating_sub(action_btns.len());
        for (key, label) in to_show.into_iter().take(remaining) {
            action_btns.push(
                button::custom(widget::text::caption(label.as_str()))
                    .padding([5, 12])
                    .class(action_pill_style(accent_rgb))
                    .on_press(on_invoke(id, key.clone()))
                    .into(),
            );
        }
    }
    if !action_btns.is_empty() {
        rows.push(widget::row::with_children(action_btns).spacing(6).into());
    }

    widget::container(widget::column::with_children(rows).spacing(6))
        .padding(pad)
        .width(Length::Fixed(TOAST_WIDTH))
        .class(toast_card_style(cfg))
        .into()
}

/// Transient centered OSD pill (e.g. `logi-osd` CapsLock, NumLock, FnLock,
/// keyboard backlight dots, Easy-Switch host, charging).
#[derive(Clone, Debug)]
pub struct OsdPill {
    pub icon: String,
    pub summary: String,
    pub body: String,
    pub tone: String,
    pub dots_on: i32,
    pub dots_total: i32,
    pub expires_at: Instant,
}

impl OsdPill {
    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }
}

fn osd_icon_name(raw: &str) -> &str {
    match raw {
        "logi-caps-lock" | "logi-num-lock" | "logi-insert" | "logi-fn-lock" => {
            "input-keyboard-symbolic"
        }
        "logi-backlight" => "display-brightness-symbolic",
        "logi-battery" => "battery-low-symbolic",
        "logi-battery-charging" => "battery-good-charging-symbolic",
        "logi-switch" => "network-wireless-symbolic",
        "" => "input-keyboard-symbolic",
        other => other,
    }
}

/// Renders the bottom-center stadium OSD pill matching the classic `logi-osd`
/// geometry (38–42px tall, 260–420px wide, full pill radius, left icon + label,
/// right tone-colored value or 9px level dots).
pub fn osd_view<'a, M: 'a>(pill: &'a OsdPill) -> Element<'a, M> {
    let tone = pill.tone.clone();
    let icon_sym = osd_icon_name(&pill.icon);
    let tone_color = move |theme: &cosmic::Theme| -> Color {
        match tone.as_str() {
            "atencao" => Color::from_rgb(0.98, 0.72, 0.22),
            "critico" => Color::from_rgb(0.95, 0.32, 0.34),
            _ => theme.cosmic().accent_color().into(),
        }
    };

    let mut content: Vec<Element<'a, M>> = vec![
        widget::icon::icon(icon(icon_sym)).size(18).into(),
        widget::text::body(pill.summary.as_str())
            .width(Length::Fill)
            .into(),
    ];

    if pill.dots_total > 0 {
        let total = pill.dots_total.clamp(1, 10);
        let on = pill.dots_on.clamp(0, total);
        let dots: Vec<Element<'a, M>> = (0..total)
            .map(|i| {
                let lit = i < on;
                widget::container(widget::space::horizontal().width(Length::Fixed(9.0)))
                    .width(Length::Fixed(9.0))
                    .height(Length::Fixed(9.0))
                    .class(theme::Container::custom(move |theme| {
                        let c: Color = if lit {
                            theme.cosmic().accent_color().into()
                        } else {
                            Color::from_rgba(1.0, 1.0, 1.0, 0.18)
                        };
                        widget::container::Style {
                            background: Some(Background::Color(c)),
                            border: iced::Border {
                                radius: 4.5.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    }))
                    .into()
            })
            .collect();
        content.push(
            widget::row::with_children(dots)
                .spacing(6)
                .align_y(Alignment::Center)
                .into(),
        );
    } else if !pill.body.is_empty() {
        fn style_dim(theme: &cosmic::Theme) -> iced::widget::text::Style {
            let mut base: Color = theme.cosmic().background(theme.transparent).on.into();
            base.a *= 0.72;
            iced::widget::text::Style { color: Some(base), ..Default::default() }
        }
        fn style_warn(_: &cosmic::Theme) -> iced::widget::text::Style {
            iced::widget::text::Style { color: Some(Color::from_rgb(0.98, 0.72, 0.22)), ..Default::default() }
        }
        fn style_crit(_: &cosmic::Theme) -> iced::widget::text::Style {
            iced::widget::text::Style { color: Some(Color::from_rgb(0.95, 0.32, 0.34)), ..Default::default() }
        }
        fn style_accent(theme: &cosmic::Theme) -> iced::widget::text::Style {
            iced::widget::text::Style { color: Some(theme.cosmic().accent_color().into()), ..Default::default() }
        }
        let style_fn: fn(&cosmic::Theme) -> iced::widget::text::Style = match pill.tone.as_str() {
            "apagado" => style_dim,
            "atencao" => style_warn,
            "critico" => style_crit,
            _ => style_accent,
        };
        content.push(
            widget::text::body(pill.body.as_str())
                .class(theme::Text::Custom(style_fn))
                .into(),
        );
    }

    widget::container(
        widget::row::with_children(content)
            .spacing(12)
            .align_y(Alignment::Center),
    )
    .padding([9, 18])
    .width(Length::Fixed(300.0))
    .class(theme::Container::custom(move |theme| {
        let cosmic = theme.cosmic();
        let c = card(cosmic);
        let mut bg: Color = c.base.into();
        bg.a = 0.94;
        let mut border = tone_color(theme);
        border.a = 0.55;
        widget::container::Style {
            background: Some(Background::Color(bg)),
            text_color: Some(c.on.into()),
            icon_color: Some(tone_color(theme)),
            border: iced::Border {
                radius: 21.0.into(),
                width: 1.0,
                color: border,
            },
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.45),
                offset: iced::Vector::new(0.0, 8.0),
                blur_radius: 16.0,
            },
            ..Default::default()
        }
    }))
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_placement_maps_all_positions() {
        for pos in NotifPosition::ALL {
            let (anchor, margin) = surface_placement(pos);
            assert!(!anchor.is_empty());
            assert!(margin.top >= 16);
        }
        let (bottom_anchor, bottom_margin) = surface_placement(NotifPosition::BottomRight);
        assert!(bottom_anchor.contains(Anchor::BOTTOM));
        assert!(bottom_anchor.contains(Anchor::RIGHT));
        assert!(bottom_margin.bottom >= 48);
    }
}
