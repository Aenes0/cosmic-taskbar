// SPDX-License-Identifier: GPL-3.0-only

mod anim;
mod app;
mod config;
pub(crate) mod fx;
mod launcher_entry;
mod texts;
mod wayland_handler;
mod wayland_subscription;
pub(crate) mod zoomed;

pub const APP_ID: &str = "io.github.Aenes0.CosmicTaskbar";

/// The panel's applet: the taskbar.
pub fn run() -> cosmic::iced::Result {
    // A resident applet does not need one tokio worker per core (32 here):
    // each one is a thread, and there is one process per monitor. Two do the
    // work (the Alt+Tab went from 76 to 16 threads this way). Before any thread.
    if std::env::var_os("TOKIO_WORKER_THREADS").is_none() {
        // SAFETY: nothing else runs yet.
        unsafe { std::env::set_var("TOKIO_WORKER_THREADS", "2") };
    }
    // The panel may run us with RUST_LOG=debug; the bar and libcosmic then
    // log on every frame of an animation, which is megabytes of journal.
    // Their per-frame targets stay at warn whatever the environment says.
    crate::shared::logging::init(
        "warn,cosmic_taskbar=info",
        &["cosmic::applet", "iced_core::clipboard", "cosmic_config", "sctk"],
    );
    let _ = tracing_log::LogTracer::init();
    cosmic::applet::run::<app::Taskbar>(())
}
