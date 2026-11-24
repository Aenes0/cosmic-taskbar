// SPDX-License-Identifier: GPL-3.0-only

//! Lightweight, non-blocking overlay mutual-exclusion coordinator.
//!
//! Opening any top-level panel overlay (`StartMenu`, `CentralPanels`, or
//! `TaskbarPopups`) immediately dismisses the others so the user never has to
//! click twice or wait for compositor focus-loss races. None of these calls
//! ever spawn a process that is not already running.

use tokio::sync::OnceCell;

/// Which overlay is taking ownership of the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    Start,
    Central,
    Taskbar,
}

const START_BUS: &str = "io.github.Aenes0.CosmicStart";
const START_PATH: &str = "/io/github/Aenes0/CosmicStart";
const PANELS_BUS: &str = "io.github.Aenes0.CosmicCentral.Panels";
const PANELS_PATH: &str = "/io/github/Aenes0/CosmicCentral/Panels";

async fn session_bus() -> Option<&'static zbus::Connection> {
    static BUS: OnceCell<zbus::Connection> = OnceCell::const_new();
    BUS.get_or_try_init(zbus::Connection::session).await.ok()
}

async fn send_dbus_close(bus_name: &'static str, path: &'static str) {
    let Some(conn) = session_bus().await else {
        return;
    };
    let Ok(msg) = zbus::Message::method_call(path, "Close")
        .and_then(|b| b.destination(bus_name))
        .and_then(|b| b.interface(bus_name))
        .and_then(|b| b.with_flags(zbus::message::Flags::NoReplyExpected))
        .and_then(|b| b.build(&()))
    else {
        return;
    };
    let _ = conn.send(&msg).await;
}

/// Dismisses every other panel overlay asynchronously without blocking UI or
/// starting stopped services.
pub async fn dismiss_others(owner: Overlay) {
    let dismiss_start = async {
        if owner != Overlay::Start {
            send_dbus_close(START_BUS, START_PATH).await;
        }
    };
    let dismiss_central = async {
        if owner != Overlay::Central {
            send_dbus_close(PANELS_BUS, PANELS_PATH).await;
        }
    };
    let dismiss_taskbar = async {
        if owner != Overlay::Taskbar {
            // `send_command` never spawns the host for `HideAll`.
            crate::bar::popups::proto::send_command(crate::bar::popups::proto::PopupCommand::HideAll).await;
        }
    };
    tokio::join!(dismiss_start, dismiss_central, dismiss_taskbar);
}
