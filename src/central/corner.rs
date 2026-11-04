// SPDX-License-Identifier: GPL-3.0-only

//! The bar's right corner as one applet: the tray, the Central and "show
//! desktop", each still its own program's code, in one process per screen
//! instead of three. Each runs as it would alone, but takes clicks and the
//! pointer only on its own part, and only this one fits the window.

use cosmic::{
    Element, Task, app,
    iced::{self, Subscription, runtime::platform_specific::wayland::popup::SctkPopupSettings, window},
    surface, widget,
};

use crate::central::{
    app::{self as central, App as Central},
    show_desktop::{self, ShowDesktop},
    tray::components::app::{App as Tray, Msg as TrayMsg},
};

pub const APP_ID: &str = "io.github.Aenes0.CosmicCentral.Corner";

/// Room the tray leaves the others when the panel squeezes the corner: the
/// "show desktop" sliver and the Central (clock and a few icons), in buttons.
const OTHERS_IN_BUTTONS: f32 = 4.0;
const SHOW_DESKTOP_LEN: f32 = 10.0;

pub struct Corner {
    core: app::Core,
    tray: Tray,
    central: Central,
    show: ShowDesktop,
}

#[derive(Debug, Clone)]
pub enum Message {
    Tray(TrayMsg),
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

/// The tray's popup settings, as it asks for them: run on the tray.
type TraySettings = Box<dyn Fn(&mut Tray) -> SctkPopupSettings + Send + Sync>;

/// The tray's surface actions as the corner's. Its menus and the ^ box come
/// as `AppPopup`, with settings that run on the app; `map` leaves those as
/// they are, typed for the tray, and the runtime, holding the corner, would
/// drop them ("Invalid settings for popup"). So they run on the corner's tray.
fn tray_surface(action: surface::Action<TrayMsg>) -> surface::Action<Message> {
    match action {
        surface::Action::AppPopup(settings, live, None) => {
            let settings = std::sync::Arc::try_unwrap(settings)
                .ok()
                .and_then(|settings| settings.downcast::<TraySettings>().ok());
            match settings {
                Some(settings) => surface::action::app_popup::<Corner>(
                    |_| Default::default(),
                    move |corner: &mut Corner| settings(&mut corner.tray),
                    None,
                ),
                None => {
                    tracing::warn!("tray popup with settings of another kind, not opened");
                    surface::Action::AppPopup(std::sync::Arc::new(Box::new(())), live, None)
                }
            }
        }
        action => action.map(Message::Tray),
    }
}

/// The tray's task as the corner's, its popups included.
fn lift_tray(task: app::Task<TrayMsg>) -> app::Task<Message> {
    task.map(|action| match action {
        cosmic::Action::Surface(action) => cosmic::Action::Surface(tray_surface(action)),
        cosmic::Action::App(message) => cosmic::Action::App(Message::Tray(message)),
        cosmic::Action::Cosmic(action) => cosmic::Action::Cosmic(action),
        cosmic::Action::None => cosmic::Action::None,
    })
}

/// What the tray leaves to the others, given one of its buttons' length.
fn reserved(button_len: f32) -> f32 {
    SHOW_DESKTOP_LEN + OTHERS_IN_BUTTONS * button_len
}

impl Corner {
    /// The children read the panel's size and theme from their own copy.
    fn sync(&mut self) {
        let core = self.core.clone();
        *cosmic::Application::core_mut(&mut self.tray) = core.clone();
        *cosmic::Application::core_mut(&mut self.central) = core.clone();
        *cosmic::Application::core_mut(&mut self.show) = core;
        let len = self.tray.button_len();
        self.tray.set_reserved(reserved(len));
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
        let (tray, tray_task) = Tray::in_corner(core.clone());
        let (central, central_task) = central::in_corner(core.clone());
        let (show, show_task) = show_desktop::in_corner(core.clone());
        let mut corner = Self { core, tray, central, show };
        corner.sync();
        let tasks = Task::batch([
            lift_tray(tray_task),
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
            Message::Tray(message) => lift_tray(cosmic::Application::update(&mut self.tray, message)),
            Message::Central(message) => {
                lift(cosmic::Application::update(&mut self.central, message), Message::Central)
            }
            Message::Show(message) => lift(cosmic::Application::update(&mut self.show, message), Message::Show),
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
            cosmic::Application::subscription(&self.tray).map(Message::Tray),
            cosmic::Application::subscription(&self.central).map(Message::Central),
            cosmic::Application::subscription(&self.show).map(Message::Show),
        ])
    }

    fn view(&self) -> Element<'_, Message> {
        let parts = [
            self.tray.corner_view().map(Message::Tray),
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

    // Only the tray opens surfaces of its own (its menus and the ^ box).
    fn view_window(&self, id: window::Id) -> Element<'_, Message> {
        cosmic::Application::view_window(&self.tray, id).map(Message::Tray)
    }

    fn on_close_requested(&self, id: window::Id) -> Option<Message> {
        cosmic::Application::on_close_requested(&self.tray, id).map(Message::Tray)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tray_popup_opens_on_the_corner() {
        let action = surface::action::app_popup::<Tray>(
            |_| Default::default(),
            |_tray: &mut Tray| -> SctkPopupSettings { unreachable!("only its type is checked") },
            None,
        );
        let surface::Action::AppPopup(settings, _, _) = tray_surface(action) else {
            panic!("still a popup");
        };
        let settings = std::sync::Arc::try_unwrap(settings).ok().unwrap();
        assert!(
            settings.downcast::<Box<dyn Fn(&mut Corner) -> SctkPopupSettings + Send + Sync>>().is_ok(),
            "the runtime, holding the corner, can run it"
        );
    }

    #[test]
    fn the_tray_leaves_room_for_the_central_and_show_desktop() {
        assert_eq!(reserved(30.0), 10.0 + 4.0 * 30.0);
    }
}
