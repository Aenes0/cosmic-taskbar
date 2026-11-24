// SPDX-License-Identifier: GPL-3.0-only

//! Views for the sections of the native COSMIC Control Panel.
//!
//! `shell` is the frame (rail, tabs, page header, apply bar, toasts); each section file exposes
//! its body plus `<section>_actions` / `<section>_badge` for the page header, so the frame never
//! has to know what a section contains.

mod backup;
mod bar;
mod camera;
mod capture;
mod central;
mod displays;
mod home;
mod legacy;
mod login;
mod monitor;
mod network;
mod notifications;
mod optional;
mod osd;
pub(super) mod palette;
mod power;
pub(super) mod row;
mod security;
mod shell;
mod system;
mod bars;

use cosmic::{
    Element,
    iced::widget::scrollable::{RelativeOffset, snap_to},
    widget,
};

use crate::control_panel::{ControlPanel, Message};

/// Identificador do rolável da página (alvo de "Ir para" e do topo ao trocar de seção).
const PAGE_SCROLL: &str = "cp-page-scroll";

pub(super) fn page_scroll_id() -> widget::Id {
    widget::Id::new(PAGE_SCROLL)
}

/// Rola a página até `fraction` (0 = topo, 1 = fim) da altura rolável.
pub(super) fn scroll_page_to(fraction: f32) -> cosmic::app::Task<Message> {
    snap_to(
        page_scroll_id(),
        RelativeOffset { x: None, y: Some(fraction.clamp(0.0, 1.0)) },
    )
}

/// Id do campo de busca da paleta (para focá-lo ao abrir).
pub(super) fn palette_input_id() -> widget::Id {
    palette::input_id()
}

pub fn view(app: &ControlPanel) -> Element<'_, Message> {
    shell::view(app)
}

pub(super) fn header_start(app: &ControlPanel) -> Vec<Element<'_, Message>> {
    bars::header_start(app)
}

pub(super) fn header_center(app: &ControlPanel) -> Vec<Element<'_, Message>> {
    bars::header_center(app)
}

pub(super) fn header_end(app: &ControlPanel) -> Vec<Element<'_, Message>> {
    bars::header_end(app)
}
