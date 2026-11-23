// SPDX-License-Identifier: GPL-3.0-only

//! Standalone Wayland toplevel + screencopy worker for the isolated window
//! switcher. Connects directly to `WAYLAND_DISPLAY` (`cosmic-comp`) and speaks
//! `ext_foreign_toplevel_list_v1` + `zcosmic_toplevel_info_v1` +
//! `zcosmic_toplevel_manager_v1` + `ext_image_copy_capture_v1`.

use std::os::{
    fd::{FromRawFd, RawFd},
    unix::net::UnixStream,
};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

use cosmic::cctk::{
    self,
    cosmic_protocols::{
        toplevel_info::v1::client::zcosmic_toplevel_handle_v1,
        toplevel_management::v1::client::zcosmic_toplevel_manager_v1,
    },
    sctk::{
        self,
        output::{OutputHandler, OutputState},
        reexports::{calloop, calloop_wayland_source::WaylandSource},
        registry::{ProvidesRegistryState, RegistryState},
        seat::{Capability, SeatHandler, SeatState},
    },
    toplevel_info::{ToplevelInfoHandler, ToplevelInfoState},
    toplevel_management::{ToplevelManagerHandler, ToplevelManagerState},
    wayland_client::{
        Connection, QueueHandle, WEnum,
        globals::registry_queue_init,
        protocol::{wl_output::WlOutput, wl_seat::WlSeat},
    },
    wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
};
use cosmic::iced::{
    Subscription,
    futures::{self, SinkExt, channel::mpsc},
    stream,
};

use crate::shared::capture::{self, CaptureState, CaptureWorker, start_worker};
use crate::shared::preview_card::Picture;

/// Largest side (in pixels) of a switcher window thumbnail.
const THUMB_MAX: u32 = 480;

/// Only capture thumbnails while the switcher is open.
pub static CAPTURE_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Snapshot of one open window from the compositor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WinInfo {
    pub id: String,
    pub title: String,
    pub app_id: String,
    pub active: bool,
    pub minimized: bool,
    pub outputs: Vec<String>,
}

/// Commands sent from the UI to the Wayland worker thread.
#[derive(Debug, Clone)]
pub enum WaylandCmd {
    Activate(String),
    Close(String),
    Capture(Vec<String>),
}

/// Events sent from the Wayland worker thread to the UI.
#[derive(Debug, Clone)]
pub enum WaylandEvent {
    Ready(calloop::channel::Sender<WaylandCmd>),
    Windows(Vec<WinInfo>),
    Thumb(String, Option<Picture>),
}

pub fn subscription() -> Subscription<WaylandEvent> {
    struct SwitcherWayland;
    Subscription::run_with(std::any::TypeId::of::<SwitcherWayland>(), |_| {
        stream::channel(32, |mut output: mpsc::Sender<WaylandEvent>| async move {
            let (tx, rx) = calloop::channel::channel();
            let _ = output.send(WaylandEvent::Ready(tx)).await;
            std::thread::spawn(move || {
                if let Err(err) = run_worker(output, rx) {
                    tracing::error!("switcher wayland worker stopped: {err}");
                }
            });
            futures::future::pending::<()>().await;
        })
    })
}

struct State {
    conn: Connection,
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    info: ToplevelInfoState,
    manager: ToplevelManagerState,
    capture: CaptureState,
    worker: Option<CaptureWorker>,
    tx: mpsc::Sender<WaylandEvent>,
    dirty: bool,
    last: Option<Vec<WinInfo>>,
}

impl State {
    fn report(&mut self) {
        if !std::mem::take(&mut self.dirty) {
            return;
        }
        use zcosmic_toplevel_handle_v1::State as S;
        let windows: Vec<WinInfo> = self
            .info
            .toplevels()
            .map(|t| {
                let mut outputs: Vec<String> = t
                    .output
                    .iter()
                    .filter_map(|o| self.output_state.info(o).and_then(|info| info.name))
                    .collect();
                outputs.sort();
                WinInfo {
                    id: t.identifier.clone(),
                    title: t.title.clone(),
                    app_id: t.app_id.clone(),
                    active: t.state.contains(&S::Activated),
                    minimized: t.state.contains(&S::Minimized),
                    outputs,
                }
            })
            .collect();

        if self.last.as_ref() == Some(&windows) {
            return;
        }
        if futures::executor::block_on(self.tx.send(WaylandEvent::Windows(windows.clone()))).is_ok()
        {
            self.last = Some(windows);
        }
    }

    fn cosmic_handle(&self, id: &str) -> Option<zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1> {
        self.info
            .toplevels()
            .find(|t| t.identifier == id)
            .and_then(|t| t.cosmic_toplevel.clone())
    }

    fn foreign(&self, id: &str) -> Option<ExtForeignToplevelHandleV1> {
        self.info
            .toplevels()
            .find(|t| t.identifier == id)
            .map(|t| t.foreign_toplevel.clone())
    }

    fn handle_cmd(&mut self, cmd: WaylandCmd) {
        match cmd {
            WaylandCmd::Activate(id) => {
                let Some(handle) = self.cosmic_handle(&id) else {
                    return;
                };
                self.manager.manager.unset_minimized(&handle);
                if let Some(seat) = self.seat_state.seats().next() {
                    self.manager.manager.activate(&handle, &seat);
                }
                let _ = self.conn.flush();
            }
            WaylandCmd::Close(id) => {
                if let Some(handle) = self.cosmic_handle(&id) {
                    self.manager.manager.close(&handle);
                    let _ = self.conn.flush();
                }
            }
            WaylandCmd::Capture(ids) => {
                for id in ids {
                    match &self.worker {
                        Some(worker) => worker.request(id.clone(), self.foreign(&id)),
                        None => {
                            let _ = futures::executor::block_on(
                                self.tx.send(WaylandEvent::Thumb(id, None)),
                            );
                        }
                    }
                }
            }
        }
    }
}

fn run_worker(
    tx: mpsc::Sender<WaylandEvent>,
    rx: calloop::channel::Channel<WaylandCmd>,
) -> Result<(), Box<dyn std::error::Error>> {
    let socket = std::env::var("X_PRIVILEGED_WAYLAND_SOCKET")
        .ok()
        .and_then(|fd| fd.parse::<RawFd>().ok())
        .map(|fd| unsafe { UnixStream::from_raw_fd(fd) });
    let conn = match socket {
        Some(socket) => Connection::from_socket(socket)?,
        None => Connection::connect_to_env()?,
    };
    let (globals, event_queue) = registry_queue_init(&conn)?;
    let qh = event_queue.handle();
    let registry_state = RegistryState::new(&globals);
    let info = ToplevelInfoState::try_new(&registry_state, &qh).ok_or("no toplevel info")?;
    let manager =
        ToplevelManagerState::try_new(&registry_state, &qh).ok_or("no toplevel manager")?;
    let sender = Mutex::new(tx.clone());
    let mut state = State {
        conn: conn.clone(),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        capture: CaptureState::new(&globals, &qh),
        worker: None,
        registry_state,
        info,
        manager,
        tx,
        dirty: true,
        last: None,
    };
    state.worker = state.capture.data(&conn, &qh).map(|data| {
        let done = move |id: String, image: Option<image::RgbaImage>| {
            let picture = image.map(|img| {
                let (w, h) = img.dimensions();
                (
                    cosmic::widget::image::Handle::from_rgba(w, h, img.into_raw()),
                    w,
                    h,
                )
            });
            let mut guard = sender.lock().unwrap_or_else(|e| e.into_inner());
            let _ = futures::executor::block_on(guard.send(WaylandEvent::Thumb(id, picture)));
        };
        let gate = || CAPTURE_ACTIVE.load(Ordering::Relaxed);
        start_worker(data, THUMB_MAX, gate, done)
    });

    let mut event_loop = calloop::EventLoop::<State>::try_new()?;
    WaylandSource::new(conn, event_queue)
        .insert(event_loop.handle())
        .map_err(|err| err.error)?;
    event_loop
        .handle()
        .insert_source(rx, |event, (), state| {
            if let calloop::channel::Event::Msg(cmd) = event {
                state.handle_cmd(cmd);
            }
        })
        .map_err(|err| err.error)?;

    loop {
        event_loop.dispatch(None, &mut state)?;
        state.report();
    }
}

impl ProvidesRegistryState for State {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    sctk::registry_handlers![SeatState, OutputState];
}

impl SeatHandler for State {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat) {}
    fn new_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat, _: Capability) {}
    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat, _: Capability) {
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat) {}
}

impl OutputHandler for State {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlOutput) {
        self.dirty = true;
    }
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlOutput) {
        self.dirty = true;
    }
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlOutput) {
        self.dirty = true;
    }
}

impl ToplevelInfoHandler for State {
    fn toplevel_info_state(&mut self) -> &mut ToplevelInfoState {
        &mut self.info
    }
    fn new_toplevel(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &ExtForeignToplevelHandleV1,
    ) {
        self.dirty = true;
    }
    fn update_toplevel(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &ExtForeignToplevelHandleV1,
    ) {
        self.dirty = true;
    }
    fn toplevel_closed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &ExtForeignToplevelHandleV1,
    ) {
        self.dirty = true;
    }
}

impl ToplevelManagerHandler for State {
    fn toplevel_manager_state(&mut self) -> &mut ToplevelManagerState {
        &mut self.manager
    }
    fn capabilities(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: Vec<WEnum<zcosmic_toplevel_manager_v1::ZcosmicToplelevelManagementCapabilitiesV1>>,
    ) {
    }
}

cctk::delegate_toplevel_info!(State);
cctk::delegate_toplevel_manager!(State);
capture::delegate_capture!(State);
sctk::delegate_seat!(State);
sctk::delegate_output!(State);
sctk::delegate_registry!(State);
