// Copyright 2023 System76 <info@system76.com>, 2026 Aenes0
// SPDX-License-Identifier: GPL-3.0-only

use crate::bar::wayland_subscription::{
    OutputUpdate, ToplevelRequest, ToplevelUpdate, WaylandImage, WaylandRequest, WaylandUpdate,
};
use crate::shared::capture::{self, CaptureState, CaptureWorker, start_worker};
use std::os::{
    fd::{FromRawFd, RawFd},
    unix::net::UnixStream,
};

use cctk::{
    sctk::{
        self,
        activation::{RequestData, RequestDataExt},
        output::{OutputHandler, OutputState},
        reexports::{calloop, calloop_wayland_source::WaylandSource},
        seat::{SeatHandler, SeatState},
    },
    toplevel_info::{ToplevelInfoHandler, ToplevelInfoState},
    toplevel_management::{ToplevelManagerHandler, ToplevelManagerState},
    wayland_client::{
        Connection, QueueHandle, WEnum,
        globals::registry_queue_init,
        protocol::{wl_output, wl_seat::WlSeat, wl_surface::WlSurface},
    },
    wayland_protocols::ext::{
        foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
        workspace::v1::client::ext_workspace_handle_v1::State as WorkspaceUpdateState,
    },
    workspace::{WorkspaceHandler, WorkspaceState},
};
use cosmic_protocols::{
    toplevel_info::v1::client::zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1,
    toplevel_management::v1::client::zcosmic_toplevel_manager_v1,
};
use futures::channel::mpsc::UnboundedSender;
use sctk::{
    activation::{ActivationHandler, ActivationState},
    registry::{ProvidesRegistryState, RegistryState},
};
struct AppData {
    exit: bool,
    tx: UnboundedSender<WaylandUpdate>,
    queue_handle: QueueHandle<Self>,
    output_state: OutputState,
    workspace_state: WorkspaceState,
    toplevel_info_state: ToplevelInfoState,
    toplevel_manager_state: ToplevelManagerState,
    /// Read by the capture handlers of `delegate_capture!`.
    capture: CaptureState,
    /// `None` when the compositor cannot capture windows.
    worker: Option<CaptureWorker<ExtForeignToplevelHandleV1>>,
    registry_state: RegistryState,
    seat_state: SeatState,
    activation_state: Option<ActivationState>,
}

// Workspace and toplevel handling

// Need to bind output globals just so workspace can get output events
impl OutputHandler for AppData {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if let Some(info) = self.output_state.info(&output) {
            let _ = self
                .tx
                .unbounded_send(WaylandUpdate::Output(OutputUpdate::Add(output, info)));
        }
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if let Some(info) = self.output_state.info(&output) {
            let _ = self
                .tx
                .unbounded_send(WaylandUpdate::Output(OutputUpdate::Update(output, info)));
        }
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        let _ = self
            .tx
            .unbounded_send(WaylandUpdate::Output(OutputUpdate::Remove(output)));
    }
}

impl WorkspaceHandler for AppData {
    fn workspace_state(&mut self) -> &mut WorkspaceState {
        &mut self.workspace_state
    }

    fn done(&mut self) {
        let active_workspaces = self
            .workspace_state
            .workspace_groups()
            .filter_map(|x| {
                x.workspaces
                    .iter()
                    .filter_map(|handle| self.workspace_state.workspace_info(handle))
                    .find(|w| w.state.contains(WorkspaceUpdateState::Active))
                    .map(|workspace| workspace.handle.clone())
            })
            .collect::<Vec<_>>();
        let _ = self
            .tx
            .unbounded_send(WaylandUpdate::Workspace(active_workspaces));
    }
}

impl ProvidesRegistryState for AppData {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    sctk::registry_handlers!();
}

struct ExecRequestData {
    data: RequestData,
    exec: String,
    gpu_idx: Option<usize>,
    terminal: bool,
}

impl RequestDataExt for ExecRequestData {
    fn app_id(&self) -> Option<&str> {
        self.data.app_id()
    }

    fn seat_and_serial(&self) -> Option<(&WlSeat, u32)> {
        self.data.seat_and_serial()
    }

    fn surface(&self) -> Option<&WlSurface> {
        self.data.surface()
    }
}

impl ActivationHandler for AppData {
    type RequestData = ExecRequestData;
    fn new_token(&mut self, token: String, data: &ExecRequestData) {
        let _ = self.tx.unbounded_send(WaylandUpdate::ActivationToken {
            token: Some(token),
            app_id: data.app_id().map(String::from),
            exec: data.exec.clone(),
            gpu_idx: data.gpu_idx,
            terminal: data.terminal,
        });
    }
}

impl SeatHandler for AppData {
    fn seat_state(&mut self) -> &mut sctk::seat::SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat) {}

    fn new_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: WlSeat,
        _: sctk::seat::Capability,
    ) {
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: WlSeat,
        _: sctk::seat::Capability,
    ) {
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat) {}
}

impl ToplevelManagerHandler for AppData {
    fn toplevel_manager_state(&mut self) -> &mut cctk::toplevel_management::ToplevelManagerState {
        &mut self.toplevel_manager_state
    }

    fn capabilities(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: Vec<WEnum<zcosmic_toplevel_manager_v1::ZcosmicToplelevelManagementCapabilitiesV1>>,
    ) {
        // TODO capabilities could affect the options in the applet
    }
}

impl ToplevelInfoHandler for AppData {
    fn toplevel_info_state(&mut self) -> &mut ToplevelInfoState {
        &mut self.toplevel_info_state
    }

    fn new_toplevel(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        toplevel: &ExtForeignToplevelHandleV1,
    ) {
        if let Some(info) = self.toplevel_info_state.info(toplevel) {
            let _ = self
                .tx
                .unbounded_send(WaylandUpdate::Toplevel(ToplevelUpdate::Add(info.clone())));
        }
    }

    fn update_toplevel(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        toplevel: &ExtForeignToplevelHandleV1,
    ) {
        if let Some(info) = self.toplevel_info_state.info(toplevel) {
            let _ = self
                .tx
                .unbounded_send(WaylandUpdate::Toplevel(ToplevelUpdate::Update(
                    info.clone(),
                )));
        }
    }

    fn toplevel_closed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        toplevel: &ExtForeignToplevelHandleV1,
    ) {
        let _ = self
            .tx
            .unbounded_send(WaylandUpdate::Toplevel(ToplevelUpdate::Remove(
                toplevel.clone(),
            )));
    }
}

// Screencopy handling: the shared capture worker, as in the Start menu.

/// Largest side, in pixels, of a window thumbnail: small enough, sharp on a
/// scaled screen.
const THUMB_MAX: u32 = 400;

/// The app's message for a finished capture: always one, image or failure.
fn image_update(
    handle: ExtForeignToplevelHandleV1,
    img: Option<image::RgbaImage>,
) -> WaylandUpdate {
    match img {
        Some(img) => WaylandUpdate::Image(handle, WaylandImage::new(img)),
        None => WaylandUpdate::ImageFailed(handle),
    }
}

fn send_update(tx: &UnboundedSender<WaylandUpdate>, update: WaylandUpdate) {
    if let Err(err) = tx.unbounded_send(update) {
        tracing::error!("Failed to send image event to subscription {err:?}");
    }
}

impl AppData {
    fn cosmic_toplevel(
        &self,
        handle: &ExtForeignToplevelHandleV1,
    ) -> Option<ZcosmicToplevelHandleV1> {
        self.toplevel_info_state
            .info(handle)?
            .cosmic_toplevel
            .clone()
    }

    /// Queues a thumbnail; the answer comes as `Image` or `ImageFailed`.
    fn send_image(&self, handle: ExtForeignToplevelHandleV1) {
        match &self.worker {
            Some(worker) => worker.request(handle.clone(), Some(handle)),
            None => send_update(&self.tx, image_update(handle, None)),
        }
    }
}

pub(crate) fn wayland_handler(
    tx: UnboundedSender<WaylandUpdate>,
    rx: calloop::channel::Channel<WaylandRequest>,
) {
    let socket = std::env::var("X_PRIVILEGED_WAYLAND_SOCKET")
        .ok()
        .and_then(|fd| {
            fd.parse::<RawFd>()
                .ok()
                .map(|fd| unsafe { UnixStream::from_raw_fd(fd) })
        });

    let conn = if let Some(socket) = socket {
        Connection::from_socket(socket).unwrap()
    } else {
        Connection::connect_to_env().unwrap()
    };
    let (globals, event_queue) = registry_queue_init(&conn).unwrap();

    let mut event_loop = calloop::EventLoop::<AppData>::try_new().unwrap();
    let qh = event_queue.handle();
    let wayland_source = WaylandSource::new(conn.clone(), event_queue);
    let handle = event_loop.handle();
    wayland_source
        .insert(handle.clone())
        .expect("Failed to insert wayland source.");

    if handle
        .insert_source(rx, |event, (), state| match event {
            calloop::channel::Event::Msg(req) => match req {
                WaylandRequest::Screencopy(handle) => {
                    state.send_image(handle.clone());
                }
                WaylandRequest::Toplevel(req) => match req {
                    ToplevelRequest::Activate(handle) => {
                        if let Some(cosmic_toplevel) = state.cosmic_toplevel(&handle) {
                            if let Some(seat) = state.seat_state.seats().next() {
                                let manager = &state.toplevel_manager_state.manager;
                                manager.activate(&cosmic_toplevel, &seat);
                            }
                        }
                    }
                    ToplevelRequest::Minimize(handle) => {
                        if let Some(cosmic_toplevel) = state.cosmic_toplevel(&handle) {
                            let manager = &state.toplevel_manager_state.manager;
                            manager.set_minimized(&cosmic_toplevel);
                        }
                    }
                    ToplevelRequest::Quit(handle) => {
                        if let Some(cosmic_toplevel) = state.cosmic_toplevel(&handle) {
                            let manager = &state.toplevel_manager_state.manager;
                            manager.close(&cosmic_toplevel);
                        }
                    }
                },
                WaylandRequest::TokenRequest {
                    app_id,
                    exec,
                    gpu_idx,
                    terminal,
                } => {
                    if let Some(activation_state) = state.activation_state.as_ref() {
                        activation_state.request_token_with_data(
                            &state.queue_handle,
                            ExecRequestData {
                                data: RequestData {
                                    app_id: Some(app_id),
                                    seat_and_serial: state
                                        .seat_state
                                        .seats()
                                        .next()
                                        .map(|seat| (seat, 0)),
                                    surface: None,
                                },
                                exec,
                                gpu_idx,
                                terminal,
                            },
                        );
                    } else {
                        let _ = state.tx.unbounded_send(WaylandUpdate::ActivationToken {
                            token: None,
                            app_id: Some(app_id),
                            exec,
                            gpu_idx,
                            terminal,
                        });
                    }
                }
            },
            calloop::channel::Event::Closed => {
                state.exit = true;
            }
        })
        .is_err()
    {
        return;
    }
    let registry_state = RegistryState::new(&globals);
    let capture = CaptureState::new(&globals, &qh);
    let worker = capture.data(&conn, &qh).map(|data| {
        let tx = tx.clone();
        let done = move |handle, img| send_update(&tx, image_update(handle, img));
        start_worker(data, THUMB_MAX, || true, done)
    });

    let mut app_data = AppData {
        exit: false,
        tx,
        output_state: OutputState::new(&globals, &qh),
        workspace_state: WorkspaceState::new(&registry_state, &qh),
        toplevel_info_state: ToplevelInfoState::new(&registry_state, &qh),
        toplevel_manager_state: ToplevelManagerState::new(&registry_state, &qh),
        capture,
        worker,
        registry_state,
        seat_state: SeatState::new(&globals, &qh),
        activation_state: ActivationState::bind::<AppData>(&globals, &qh).ok(),
        queue_handle: qh,
    };

    loop {
        if app_data.exit {
            break;
        }
        event_loop.dispatch(None, &mut app_data).unwrap();
    }
}

sctk::delegate_seat!(AppData);
sctk::delegate_registry!(AppData);
cctk::delegate_toplevel_info!(AppData);
cctk::delegate_workspace!(AppData);
cctk::delegate_toplevel_manager!(AppData);
capture::delegate_capture!(AppData);

sctk::delegate_activation!(AppData, ExecRequestData);

sctk::delegate_output!(AppData);

#[cfg(test)]
mod tests {
    use super::*;
    use cctk::wayland_client::Proxy;

    /// A handle bound to no compositor: enough to be compared and sent.
    fn inert_handle() -> ExtForeignToplevelHandleV1 {
        let (sock, _peer) = UnixStream::pair().expect("socket pair");
        let conn = Connection::from_socket(sock).expect("connection");
        ExtForeignToplevelHandleV1::inert(conn.backend().downgrade())
    }

    #[test]
    fn a_thumbnail_becomes_an_image_of_its_window() {
        let handle = inert_handle();
        let img = image::RgbaImage::new(400, 225);
        match image_update(handle.clone(), Some(img)) {
            WaylandUpdate::Image(h, pic) => {
                assert_eq!(h, handle);
                assert_eq!((pic.width, pic.height), (400, 225));
            }
            other => panic!("expected Image, got {other:?}"),
        }
    }

    #[test]
    fn no_thumbnail_still_answers_image_failed() {
        let handle = inert_handle();
        match image_update(handle.clone(), None) {
            WaylandUpdate::ImageFailed(h) => assert_eq!(h, handle),
            other => panic!("expected ImageFailed, got {other:?}"),
        }
    }

    #[test]
    fn a_closed_channel_does_not_panic() {
        let (tx, rx) = futures::channel::mpsc::unbounded();
        drop(rx);
        send_update(&tx, image_update(inert_handle(), None));
    }

    #[test]
    fn thumbnails_are_at_most_400_px() {
        assert_eq!(THUMB_MAX, 400);
        let big = vec![0u8; 3840 * 2160 * 4];
        let img = capture::shrink(&big, 3840, 2160, 3840 * 4, THUMB_MAX, true).expect("shrink");
        assert_eq!(img.width().max(img.height()), 400);
    }
}
