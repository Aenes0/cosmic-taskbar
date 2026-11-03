// SPDX-License-Identifier: GPL-3.0-only

//! A resident service's own line to the journal.
//!
//! A service that outlives what started it (the panel's button, a shortcut,
//! a terminal) outlives the pipe its output went to: once that closed, the
//! next warning written aborted it. It gets its own line to the journal
//! instead, as systemd-cat does; or nowhere.

use std::{
    io::Write,
    os::{fd::AsRawFd, unix::net::UnixStream},
};

/// The stream header journald expects: identifier, unit, priority (info),
/// level prefix, and no copies to syslog, the kernel log or the console.
fn header(identifier: &str) -> Vec<u8> {
    format!("{identifier}\n\n6\n0\n0\n0\n0\n").into_bytes()
}

/// Points stdout and stderr at the journal under `identifier`, or at /dev/null.
pub fn own_output(identifier: &str) {
    let journal = UnixStream::connect("/run/systemd/journal/stdout").and_then(|mut stream| {
        stream.shutdown(std::net::Shutdown::Read)?;
        stream.write_all(&header(identifier))?;
        Ok(std::os::fd::OwnedFd::from(stream))
    });
    let target = match journal {
        Ok(fd) => fd,
        Err(_) => match std::fs::OpenOptions::new().write(true).open("/dev/null") {
            Ok(file) => file.into(),
            Err(_) => return,
        },
    };
    // SAFETY: duplicates an open descriptor over stdout and stderr.
    unsafe {
        libc::dup2(target.as_raw_fd(), 1);
        libc::dup2(target.as_raw_fd(), 2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_header_is_what_journald_reads() {
        assert_eq!(header("cosmic-taskbar-menu"), b"cosmic-taskbar-menu\n\n6\n0\n0\n0\n0\n".to_vec());
    }
}
