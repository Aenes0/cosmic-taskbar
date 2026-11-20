// SPDX-License-Identifier: GPL-3.0-only

//! Standalone Wayland toplevel + screencopy worker for the taskbar popup host.
//! Connects directly to `WAYLAND_DISPLAY` (cosmic-comp) and serves window
//! activation, minimization, closing, and live thumbnail captures.

use std::collections::HashMap;
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
        reexports::{calloop, calloop_wayland_source::WaylandSource},
        registry::{ProvidesRegistryState, RegistryState},
        seat::{Capability, SeatHandler, SeatState},
    },
    toplevel_info::{ToplevelInfoHandler, ToplevelInfoState},
    toplevel_management::{ToplevelManagerHandler, ToplevelManagerState},
    wayland_client::{
        Connection, QueueHandle, WEnum, globals::registry_queue_init, protocol::wl_seat::WlSeat,
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

const THUMB_MAX: u32 = 400;

/// Set by the host while a preview surface is open so screencopy stays idle otherwise.
pub static PREVIEW_OPEN: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WinInfo {
    pub title: String,
}

#[derive(Debug, Clone)]
pub enum WaylandCmd {
    ToggleWindow(String),
    Close(String),
    CloseMany(Vec<String>),
    Capture(Vec<String>),
}

#[derive(Debug, Clone)]
pub enum WaylandEvent {
    Ready(calloop::channel::Sender<WaylandCmd>),
    Windows(HashMap<String, WinInfo>),
    Thumb(String, Option<Picture>),
}

pub fn subscription() -> Subscription<WaylandEvent> {
    struct TaskbarPopupsWayland;
    Subscription::run_with(std::any::TypeId::of::<TaskbarPopupsWayland>(), |_| {
        stream::channel(16, |mut output: mpsc::Sender<WaylandEvent>| async move {
            let (tx, rx) = calloop::channel::channel();
            _ = output.send(WaylandEvent::Ready(tx)).await;
            std::thread::spawn(move || {
                if let Err(err) = run_worker(output, rx) {
                    tracing::warn!("taskbar popups wayland worker stopped: {err}");
                }
            });
            futures::future::pending::<()>().await;
        })
    })
}

struct State {
    registry_state: RegistryState,
    seat_state: SeatState,
    info: ToplevelInfoState,
    manager: ToplevelManagerState,
    capture: CaptureState,
    worker: Option<CaptureWorker>,
    tx: mpsc::Sender<WaylandEvent>,
    dirty: bool,
}

impl State {
    fn report(&mut self) {
        if !std::mem::take(&mut self.dirty) {
            return;
        }
        let map: HashMap<String, WinInfo> = self
            .info
            .toplevels()
            .map(|t| {
                (
                    t.identifier.clone(),
                    WinInfo {
                        title: t.title.clone(),
                    },
                )
            })
            .collect();
        let _ = futures::executor::block_on(self.tx.send(WaylandEvent::Windows(map)));
    }

    fn cosmic_handle(&self, id: &str) -> Option<(zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1, bool, bool)> {
        use zcosmic_toplevel_handle_v1::State as S;
        self.info.toplevels().find(|t| t.identifier == id).and_then(|t| {
            Some((
                t.cosmic_toplevel.clone()?,
                t.state.contains(&S::Activated),
                t.state.contains(&S::Minimized),
            ))
        })
    }

    fn foreign(&self, id: &str) -> Option<ExtForeignToplevelHandleV1> {
        self.info
            .toplevels()
            .find(|t| t.identifier == id)
            .map(|t| t.foreign_toplevel.clone())
    }

    fn handle_cmd(&mut self, cmd: WaylandCmd) {
        match cmd {
            WaylandCmd::ToggleWindow(id) => {
                let Some((handle, active, minimized)) = self.cosmic_handle(&id) else {
                    return;
                };
                if active && !minimized {
                    self.manager.manager.set_minimized(&handle);
                } else {
                    self.manager.manager.unset_minimized(&handle);
                    if let Some(seat) = self.seat_state.seats().next() {
                        self.manager.manager.activate(&handle, &seat);
                    }
                }
            }
            WaylandCmd::Close(id) => {
                if let Some((handle, _, _)) = self.cosmic_handle(&id) {
                    self.manager.manager.close(&handle);
                }
            }
            WaylandCmd::CloseMany(ids) => {
                for id in ids {
                    if let Some((handle, _, _)) = self.cosmic_handle(&id) {
                        self.manager.manager.close(&handle);
                    }
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
    let conn = Connection::connect_to_env()?;
    let (globals, event_queue) = registry_queue_init(&conn)?;
    let qh = event_queue.handle();
    let registry_state = RegistryState::new(&globals);
    let info = ToplevelInfoState::try_new(&registry_state, &qh).ok_or("no toplevel info")?;
    let manager =
        ToplevelManagerState::try_new(&registry_state, &qh).ok_or("no toplevel manager")?;
    let sender = Mutex::new(tx.clone());
    let mut state = State {
        seat_state: SeatState::new(&globals, &qh),
        capture: CaptureState::new(&globals, &qh),
        worker: None,
        registry_state,
        info,
        manager,
        tx,
        dirty: true,
    };
    state.worker = state.capture.data(&conn, &qh).map(|data| {
        let done = move |id: String, image: Option<image::RgbaImage>| {
            let picture = image.map(|img| {
                let (w, h) = img.dimensions();
                (cosmic::widget::image::Handle::from_rgba(w, h, img.into_raw()), w, h)
            });
            let mut guard = sender.lock().unwrap_or_else(|e| e.into_inner());
            let _ = futures::executor::block_on(guard.send(WaylandEvent::Thumb(id, picture)));
        };
        let gate = || PREVIEW_OPEN.load(Ordering::Relaxed);
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
    sctk::registry_handlers![SeatState];
}

impl SeatHandler for State {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat) {}
    fn new_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat, _: Capability) {}
    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat, _: Capability) {}
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat) {}
}

impl ToplevelInfoHandler for State {
    fn toplevel_info_state(&mut self) -> &mut ToplevelInfoState {
        &mut self.info
    }
    fn new_toplevel(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &ExtForeignToplevelHandleV1) {
        self.dirty = true;
    }
    fn update_toplevel(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &ExtForeignToplevelHandleV1) {
        self.dirty = true;
    }
    fn toplevel_closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &ExtForeignToplevelHandleV1) {
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
sctk::delegate_registry!(State);
