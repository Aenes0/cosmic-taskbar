// SPDX-License-Identifier: GPL-3.0-only

//! The one connection to the real compositor that the panel hands an applet
//! (`X_PRIVILEGED_WAYLAND_SOCKET`, an open socket, not a name): one socket,
//! so one connection, which every part of the process shares. Alone, each
//! applet had its own process and its own socket; in the corner the
//! workspaces, "show desktop" and libcosmic's activation token would each
//! wrap the same descriptor, close it under the others and mix their
//! messages on it ("Invalid new_id", "Bad file descriptor").

use std::os::{
    fd::{FromRawFd, RawFd},
    unix::net::UnixStream,
};
use std::sync::Mutex;

use cosmic::cctk::wayland_client::{ConnectError, Connection};

const VAR: &str = "X_PRIVILEGED_WAYLAND_SOCKET";

/// The descriptor, taken from the environment once, before any thread.
static SOCKET: Mutex<Option<RawFd>> = Mutex::new(None);
/// The connection made on it, for all.
static SHARED: Mutex<Option<Connection>> = Mutex::new(None);

fn parse(value: Option<&str>) -> Option<RawFd> {
    value?.trim().parse::<RawFd>().ok().filter(|fd| *fd >= 0)
}

/// Takes the panel's socket out of the environment, so that what reads it
/// there on its own (libcosmic's activation token) connects by the display's
/// name instead, as the tray alone always did.
///
/// # Safety
/// First thing in `main`, before any thread starts (it changes the environment).
pub unsafe fn take() {
    let fd = parse(std::env::var(VAR).ok().as_deref());
    if fd.is_some() {
        // SAFETY: by the caller, no other thread yet.
        unsafe { std::env::remove_var(VAR) };
    }
    if let Ok(mut socket) = SOCKET.lock() {
        *socket = fd;
    }
}

/// The real compositor: the panel's socket, shared, if it gave one; else by
/// the display's name.
pub fn connection() -> Result<Connection, ConnectError> {
    let mut shared = SHARED.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(conn) = shared.as_ref() {
        return Ok(conn.clone());
    }
    let fd = SOCKET.lock().ok().and_then(|mut socket| socket.take());
    let conn = match fd {
        // SAFETY: the panel's open socket, ours alone: taken out of the
        // environment and out of `SOCKET`, so wrapped this once.
        Some(fd) => Connection::from_socket(unsafe { UnixStream::from_raw_fd(fd) })?,
        None => Connection::connect_to_env()?,
    };
    *shared = Some(conn.clone());
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_descriptor_is_a_socket() {
        assert_eq!(parse(Some("168")), Some(168));
        assert_eq!(parse(Some(" 7\n")), Some(7));
        assert_eq!(parse(Some("wayland-1")), None);
        assert_eq!(parse(Some("-1")), None);
        assert_eq!(parse(Some("")), None);
        assert_eq!(parse(None), None);
    }
}
