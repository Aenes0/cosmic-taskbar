// SPDX-License-Identifier: GPL-3.0-only

//! cosmic-taskbar: the taskbar and the Start menu of the COSMIC panel, one
//! program. The first argument says which part this process is; each part
//! runs in a process of its own, so one cannot stall or take down the other.
//!
//! - `taskbar`: the panel's applet with the taskbar;
//! - `start`: the panel's applet with the Start button;
//! - `--menu`, `--tools`, `--show-desktop`: the Start menu's service and its
//!   shortcuts (Super, Win+X).
//!
//! Called by the old names (`cosmic-ext-applet-taskbar`, `cosmic-ext-applet-start`),
//! as the old shortcuts and panel entries still do, it is that part.

mod bar;
mod shared;
mod start;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Taskbar,
    Start,
}

/// The part to run, from the name the program was called by and its first argument.
fn mode(program: &str, first_arg: Option<&str>) -> Option<Mode> {
    match first_arg {
        Some("taskbar") => Some(Mode::Taskbar),
        Some("start" | "--menu" | "--tools" | "--show-desktop") => Some(Mode::Start),
        _ => {
            let name = std::path::Path::new(program)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            match name {
                "cosmic-ext-applet-taskbar" => Some(Mode::Taskbar),
                "cosmic-ext-applet-start" => Some(Mode::Start),
                _ => None,
            }
        }
    }
}

fn main() -> cosmic::iced::Result {
    let program = std::env::args().next().unwrap_or_default();
    let first = std::env::args().nth(1);
    match mode(&program, first.as_deref()) {
        Some(Mode::Taskbar) => bar::run(),
        Some(Mode::Start) => start::run(),
        None => {
            eprintln!(
                "cosmic-taskbar: unknown mode {first:?}; use taskbar, start, --menu, --tools or --show-desktop"
            );
            std::process::exit(2);
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
        assert_eq!(mode("cosmic-taskbar", Some("--show-desktop")), Some(Mode::Start));
    }

    #[test]
    fn the_old_names_still_work_with_or_without_an_argument() {
        let start = "/home/a/.local/bin/cosmic-ext-applet-start";
        assert_eq!(mode(start, None), Some(Mode::Start));
        assert_eq!(mode(start, Some("--menu")), Some(Mode::Start));
        let bar = "/home/a/.local/bin/cosmic-ext-applet-taskbar";
        assert_eq!(mode(bar, None), Some(Mode::Taskbar));
    }

    #[test]
    fn an_unknown_mode_is_refused_instead_of_guessed() {
        assert_eq!(mode("cosmic-taskbar", None), None);
        assert_eq!(mode("cosmic-taskbar", Some("--nonsense")), None);
        assert_eq!(mode("", None), None);
    }
}
