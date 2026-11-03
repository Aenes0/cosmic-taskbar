use super::*;

/// The panel's workspaces button, in place of COSMIC's (`cosmic-panel-button
/// com.system76.CosmicWorkspaces`): the same icon and the same overview on a
/// click, with the taskbar's zoom and hop.
pub struct WorkspacesButton {
    core: app::Core,
    zoom: crate::shared::button_zoom::ButtonZoom,
}

/// What COSMIC's button runs (the Exec of `com.system76.CosmicWorkspaces`).
const OVERVIEW: &str = "cosmic-workspaces";
const ICON: &str = "com.system76.CosmicWorkspaces";

#[derive(Clone, Debug)]
pub enum WorkspacesMessage {
    Click,
    Hover(bool),
    ZoomTick,
}

impl cosmic::Application for WorkspacesButton {
    type Message = WorkspacesMessage;
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    const APP_ID: &'static str = crate::start::WORKSPACES_APP_ID;

    fn init(core: app::Core, _flags: ()) -> (Self, app::Task<WorkspacesMessage>) {
        (Self { core, zoom: Default::default() }, Task::none())
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

    fn update(&mut self, message: WorkspacesMessage) -> app::Task<WorkspacesMessage> {
        match message {
            WorkspacesMessage::Click => {
                self.zoom.press();
                // The overview toggles itself when run again, as with COSMIC's button.
                std::thread::spawn(|| spawn_clean(OVERVIEW, &[]));
            }
            WorkspacesMessage::Hover(entering) => self.zoom.hover(entering),
            WorkspacesMessage::ZoomTick => self.zoom.tick(),
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<WorkspacesMessage> {
        self.zoom.frames().map(|_| WorkspacesMessage::ZoomTick)
    }

    fn view(&self) -> Element<'_, WorkspacesMessage> {
        let applet = &self.core.applet;
        let (major, minor) = applet.suggested_padding(true);
        let size = applet.suggested_size(true).0;
        let size_px = f32::from(size);
        let icon = widget::icon::from_name(ICON)
            .size(size)
            .icon()
            .width(Length::Fixed(size_px))
            .height(Length::Fixed(size_px));
        // The button's smaller side: the icon must fit it whichever way the panel runs.
        let cell = size_px + 2.0 * f32::from(minor.min(major));
        let icon = self.zoom.view(icon, size_px, cell);
        let horizontal = applet.is_horizontal();
        let button = button::custom(icon)
            .padding(if horizontal { [minor, major] } else { [major, minor] })
            .class(theme::Button::AppletIcon)
            .on_press(WorkspacesMessage::Click);
        applet
            .autosize_window(
                widget::mouse_area(button)
                    .on_enter(WorkspacesMessage::Hover(true))
                    .on_exit(WorkspacesMessage::Hover(false)),
            )
            .into()
    }
}
