// SPDX-License-Identifier: GPL-3.0-only

mod anim;
mod app;
mod config;
pub(crate) mod fx;
mod launcher_entry;
pub(crate) mod popups;
mod texts;
mod wayland_handler;
mod wayland_subscription;
pub(crate) mod zoomed;

pub const APP_ID: &str = "io.github.Aenes0.CosmicTaskbar";

/// Original ICED_BACKEND before forcing tiny-skia for the embedded panel applet,
/// restored on apps launched from the taskbar.
pub(crate) static ORIGINAL_BACKEND: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

/// The panel's applet: the taskbar.
pub fn run() -> cosmic::iced::Result {
    // Embedded panel applets live inside cosmic-panel's nested Wayland compositor,
    // where wgpu falls back to Mesa EGL and breaks popup rendering (EGL_BAD_ALLOC).
    ORIGINAL_BACKEND.get_or_init(|| std::env::var("ICED_BACKEND").ok());
    // SAFETY: nothing else runs yet.
    unsafe { std::env::set_var("ICED_BACKEND", "tiny-skia") };
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

/// Standalone layer-shell popup host for the taskbar's previews and right-click
/// context menus: connects directly to cosmic-comp on Vulkan GPU (no tiny-skia).
pub fn run_popups() -> cosmic::iced::Result {
    if std::env::var_os("TOKIO_WORKER_THREADS").is_none() {
        // SAFETY: nothing else runs yet.
        unsafe { std::env::set_var("TOKIO_WORKER_THREADS", "2") };
    }
    crate::shared::logging::init(
        "warn,cosmic_taskbar=info",
        &["cosmic::applet", "iced_core::clipboard", "cosmic_config", "sctk"],
    );
    let _ = tracing_log::LogTracer::init();
    popups::run()
}
