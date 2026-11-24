// SPDX-License-Identifier: GPL-3.0-only

mod app;
pub(crate) mod audio;
mod background;
pub(crate) mod bar_spacing;
mod board;
mod calendar;
mod clipboard;
mod corner;
mod host;
mod i18n;
pub(crate) mod layout;
mod latest;
mod media;
pub(crate) mod monitor;
pub(crate) mod notifications;
mod pages;
mod panels;
mod privileged;
pub(crate) mod services;
mod show_desktop;
mod slow;
mod texts;
pub(crate) mod tiles;
mod timing;
mod transition;
pub(crate) mod tray;
pub(crate) mod widgets;
mod workspaces;

pub const APP_ID: &str = "io.github.Aenes0.CosmicCentral";

/// The Central's modes: the main applet, `--panels`, `--corner`, `--monitor`, `--tray`,
/// `--show-desktop`, `--clipboard` and `--notifications`.
pub fn run() -> cosmic::iced::Result {
    let args: Vec<String> = std::env::args().collect();
    let panels = args.iter().position(|arg| arg == "--panels");
    // A couple of threads do: an applet waits on events, it doesn't crunch;
    // one per core (24 here) only took memory. Two threads for the panels'
    // host keep D-Bus/click responsiveness separate from background GPU/DDC reads.
    if panels.is_some() {
        // SAFETY: first thing, before any thread starts.
        unsafe { std::env::set_var("TOKIO_WORKER_THREADS", "2") };
    } else if std::env::var_os("TOKIO_WORKER_THREADS").is_none() {
        // SAFETY: first thing, before any thread starts.
        unsafe { std::env::set_var("TOKIO_WORKER_THREADS", "2") };
    }
    // SAFETY: first thing, before any thread starts.
    unsafe { privileged::take() };
    crate::shared::logging::init("warn,cosmic_taskbar=info", &[]);
    // The Central's and the monitor's panels, for every screen: one host,
    // hidden between uses; a second run only hands it what it was asked.
    if let Some(at) = panels {
        let request = panels::place::Request::from_args(&args[at + 1..]);
        // Run by its unit, it serves whatever runs already: it takes the
        // bus over, and one started by hand leaves.
        if !panels::service::under_unit() && panels::service::forward(request.as_ref()) {
            return Ok(());
        }
        panels::own_output();
        return panels::run(request);
    }
    // And the clipboard's history, a service of its own.
    if std::env::args().any(|arg| arg == "--clipboard") {
        clipboard::service::main()
    }
    // And the notifications' history, another service of its own.
    if std::env::args().any(|arg| arg == "--notifications") {
        notifications::service::main()
    }
    // Embedded panel applets run inside cosmic-panel's nested compositor, where
    // wgpu falls back to Mesa EGL and breaks popup rendering (EGL_BAD_ALLOC).
    // `--panels` (handled above) connects directly to cosmic-comp on Vulkan GPU.
    // SAFETY: before starting the UI runtime.
    unsafe { std::env::set_var("ICED_BACKEND", "tiny-skia") };
    // The bar's right corner, the tray, the Central and "show desktop" in one.
    if std::env::args().any(|arg| arg == "--corner") {
        return cosmic::applet::run::<corner::Corner>(());
    }
    // The same program is also the "show desktop" button, its own applet.
    if std::env::args().any(|arg| arg == "--show-desktop") {
        return cosmic::applet::run::<show_desktop::ShowDesktop>(());
    }
    // And the system monitor, at the bar's other end.
    if std::env::args().any(|arg| arg == "--monitor") {
        return cosmic::applet::run::<monitor::Monitor>(());
    }
    // And the tray of background apps.
    if std::env::args().any(|arg| arg == "--tray") {
        return tray::run();
    }
    cosmic::applet::run::<app::App>(())
}
