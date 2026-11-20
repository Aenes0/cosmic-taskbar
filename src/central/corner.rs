// SPDX-License-Identifier: GPL-3.0-only

//! The bar's right corner as one applet: the Central (quick settings + clock)
//! and "show desktop" in one process per screen. Each runs as it would alone,
//! taking clicks and the pointer only on its own part, and only this one fits
//! the window. The system tray is left to COSMIC's own Status Area applet so
//! this applet opens zero xdg_popups inside cosmic-panel.

use cosmic::{
    Element, Task, app,
    iced::{self, Subscription, window},
    widget,
};

use crate::central::{
    app::{self as central, App as Central},
    show_desktop::{self, ShowDesktop},
};

pub const APP_ID: &str = "io.github.Aenes0.CosmicCentral.Corner";

pub struct Corner {
    core: app::Core,
    central: Central,
    show: ShowDesktop,
}

#[derive(Debug, Clone)]
pub enum Message {
    Central(central::Message),
    Show(show_desktop::Message),
}

/// A child's task as the corner's.
fn lift<M: Send + 'static>(task: app::Task<M>, wrap: fn(M) -> Message) -> app::Task<Message> {
    task.map(move |action| match action {
        cosmic::Action::App(message) => cosmic::Action::App(wrap(message)),
        cosmic::Action::Cosmic(action) => cosmic::Action::Cosmic(action),
        // libcosmic 5a8bd94: surface actions carry the message type too.
        cosmic::Action::Surface(action) => cosmic::Action::Surface(action.map(wrap)),
        cosmic::Action::None => cosmic::Action::None,
    })
}

impl Corner {
    /// The children read the panel's size and theme from their own copy.
    fn sync(&mut self) {
        let core = self.core.clone();
        *cosmic::Application::core_mut(&mut self.central) = core.clone();
        *cosmic::Application::core_mut(&mut self.show) = core;
    }
}

impl cosmic::Application for Corner {
    type Executor = cosmic::SingleThreadExecutor;
    type Message = Message;
    type Flags = ();
    const APP_ID: &'static str = APP_ID;

    fn init(mut core: app::Core, _flags: ()) -> (Self, app::Task<Message>) {
        // See-through, but no blur behind, as each of them alone.
        core.set_auto_blur(cosmic::core::Auto::System.into());
        let (central, central_task) = central::in_corner(core.clone());
        let (show, show_task) = show_desktop::in_corner(core.clone());
        let mut corner = Self { core, central, show };
        corner.sync();
        let tasks = Task::batch([
            lift(central_task, Message::Central),
            lift(show_task, Message::Show),
        ]);
        (corner, tasks)
    }

    fn core(&self) -> &app::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut app::Core {
        &mut self.core
    }

    fn style(&self) -> Option<iced::theme::Style> {
        Some(cosmic::applet::style())
    }

    fn update(&mut self, message: Message) -> app::Task<Message> {
        self.sync();
        match message {
            Message::Central(message) => {
                lift(cosmic::Application::update(&mut self.central, message), Message::Central)
            }
            Message::Show(message) => {
                lift(cosmic::Application::update(&mut self.show, message), Message::Show)
            }
        }
    }

    // These change the core without a message: the children follow at once.
    fn system_theme_update(
        &mut self,
        _keys: &[&'static str],
        _theme: &cosmic::cosmic_theme::Theme,
    ) -> app::Task<Message> {
        self.sync();
        Task::none()
    }

    fn system_theme_mode_update(
        &mut self,
        _keys: &[&'static str],
        _mode: &cosmic::cosmic_theme::ThemeMode,
    ) -> app::Task<Message> {
        self.sync();
        Task::none()
    }

    fn on_window_resize(&mut self, _id: window::Id, _width: f32, _height: f32) {
        self.sync();
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            cosmic::Application::subscription(&self.central).map(Message::Central),
            cosmic::Application::subscription(&self.show).map(Message::Show),
        ])
    }

    fn view(&self) -> Element<'_, Message> {
        let parts = [
            self.central.corner_view().map(Message::Central),
            self.show.corner_view().map(Message::Show),
        ];
        let line: Element<'_, Message> = if self.core.applet.is_horizontal() {
            widget::row::with_children(parts).align_y(iced::Alignment::Center).into()
        } else {
            widget::column::with_children(parts).align_x(iced::Alignment::Center).into()
        };
        self.core.applet.autosize_window(line).into()
    }
}
