// SPDX-License-Identifier: GPL-3.0-only

//! cosmic-taskbar: everything the COSMIC panel shows from this project, one
//! program. The first argument says which part this process is; each part
//! runs in a process of its own (started by the panel or by its systemd unit),
//! so one that hangs or crashes takes nothing else with it.
//!
//! - `taskbar`: the panel's applet with the taskbar;
//! - `start`: the panel's applet with the Start button;
//! - `--menu`, `--tools`, `--show-desktop`: the Start menu's service and its
//!   shortcuts (Super, Win+X);
//! - `central [flags]`: the Central (quick settings, the left and right panels,
//!   the tray, the notifications' and the clipboard's services);
//! - `logi [flags]`: the Logitech devices' service.
//!
//! Called by the old names (`cosmic-ext-applet-taskbar`, `cosmic-ext-applet-start`,
//! `cosmic-ext-applet-central`, `logi-osd`), as old shortcuts, panel entries and
//! units still do, it is that part, with the arguments it always had.

mod bar;
mod central;
mod logi;
mod shared;
mod start;

use std::process::ExitCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Taskbar,
    Start,
    Central,
    /// The Logitech service; its own arguments start at this index.
    Logi(usize),
}

/// The part to run, from the name the program was called by and its first argument.
fn mode(program: &str, first_arg: Option<&str>) -> Option<Mode> {
    match first_arg {
        Some("taskbar") => return Some(Mode::Taskbar),
        Some("start" | "--menu" | "--tools") => return Some(Mode::Start),
        Some("central") => return Some(Mode::Central),
        // Only the Central has these: it runs its own executable with them
        // (the panels' host, when there is no unit), which is this program.
        Some("--panels" | "--tray" | "--monitor" | "--clipboard" | "--notifications") => {
            return Some(Mode::Central);
        }
        Some("logi") => return Some(Mode::Logi(2)),
        _ => {}
    }
    let name = std::path::Path::new(program)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    match (name, first_arg) {
        ("cosmic-ext-applet-taskbar", _) => Some(Mode::Taskbar),
        ("cosmic-ext-applet-start", _) => Some(Mode::Start),
        ("cosmic-ext-applet-central", _) => Some(Mode::Central),
        ("logi-osd", _) => Some(Mode::Logi(1)),
        // The Start menu's "show desktop" shortcut, by the new name.
        (_, Some("--show-desktop")) => Some(Mode::Start),
        _ => None,
    }
}

fn exit_code(result: cosmic::iced::Result) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("cosmic-taskbar: {err}");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let program = args.first().map(String::as_str).unwrap_or_default();
    match mode(program, args.get(1).map(String::as_str)) {
        Some(Mode::Taskbar) => exit_code(bar::run()),
        Some(Mode::Start) => exit_code(start::run()),
        Some(Mode::Central) => exit_code(central::run()),
        Some(Mode::Logi(from)) => logi::run_service(args.get(from..).unwrap_or_default()),
        None => {
            eprintln!(
                "cosmic-taskbar: unknown mode {:?}; use taskbar, start, --menu, --tools, --show-desktop, central or logi",
                args.get(1)
            );
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_argument_picks_the_part() {
        assert_eq!(mode("cosmic-taskbar", Some("taskbar")), Some(Mode::Taskbar));
        assert_eq!(mode("cosmic-taskbar", Some("start")), Some(Mode::Start));
        assert_eq!(mode("cosmic-taskbar", Some("--menu")), Some(Mode::Start));
        assert_eq!(mode("cosmic-taskbar", Some("--tools")), Some(Mode::Start));
        assert_eq!(mode("cosmic-taskbar", Some("central")), Some(Mode::Central));
        assert_eq!(mode("cosmic-taskbar", Some("logi")), Some(Mode::Logi(2)));
    }

    #[test]
    fn the_central_flags_reach_the_central_by_the_new_name_too() {
        // The Central starts its panels' host by its own executable with
        // `--panels` when there is no unit: that is `cosmic-taskbar --panels`.
        for flag in ["--panels", "--tray", "--monitor", "--clipboard", "--notifications"] {
            assert_eq!(mode("/home/a/.local/bin/cosmic-taskbar", Some(flag)), Some(Mode::Central), "{flag}");
        }
    }

    #[test]
    fn show_desktop_goes_to_the_menu_by_the_new_name_and_to_the_central_by_its_old_one() {
        assert_eq!(mode("cosmic-taskbar", Some("--show-desktop")), Some(Mode::Start));
        assert_eq!(mode("/x/cosmic-ext-applet-start", Some("--show-desktop")), Some(Mode::Start));
        assert_eq!(mode("/x/cosmic-ext-applet-central", Some("--show-desktop")), Some(Mode::Central));
    }

    #[test]
    fn the_old_names_still_work_with_or_without_an_argument() {
        assert_eq!(mode("/home/a/.local/bin/cosmic-ext-applet-start", None), Some(Mode::Start));
        assert_eq!(mode("/home/a/.local/bin/cosmic-ext-applet-taskbar", None), Some(Mode::Taskbar));
        assert_eq!(mode("/home/a/.local/bin/cosmic-ext-applet-central", None), Some(Mode::Central));
        assert_eq!(mode("/home/a/.local/bin/cosmic-ext-applet-central", Some("--panels")), Some(Mode::Central));
        assert_eq!(mode("/home/a/.local/bin/logi-osd", None), Some(Mode::Logi(1)));
        assert_eq!(mode("/home/a/.local/bin/logi-osd", Some("--demo")), Some(Mode::Logi(1)));
    }

    #[test]
    fn an_unknown_mode_is_refused_instead_of_guessed() {
        assert_eq!(mode("cosmic-taskbar", None), None);
        assert_eq!(mode("cosmic-taskbar", Some("--nonsense")), None);
        assert_eq!(mode("", None), None);
    }
}
