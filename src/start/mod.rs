// SPDX-License-Identifier: GPL-3.0-only

mod anim;
mod backend;
mod menu;
mod texts;
mod uninstall;
mod tools;

pub const APP_ID: &str = "io.github.Aenes0.CosmicStart";

/// TOKIO_WORKER_THREADS as it came, for the programs started from here.
pub static WORKERS: std::sync::OnceLock<Option<std::ffi::OsString>> = std::sync::OnceLock::new();
/// ICED_BACKEND as it came, for the programs started from here.
pub static BACKEND: std::sync::OnceLock<Option<std::ffi::OsString>> = std::sync::OnceLock::new();

/// Log filter when RUST_LOG is unset. usvg's SVG warnings filled the
/// journal (~7 100 lines), so only its errors pass.
const DEFAULT_LOG: &str = "warn,cosmic_taskbar=info,usvg=error";

/// The Start menu's modes: the button the panel runs, the `--menu` service,
/// `--tools` and `--show-desktop`.
pub fn run() -> cosmic::iced::Result {
    // The D-Bus library starts a runtime with a thread per core (32 here),
    // for the little it does: one is plenty. Set before any thread starts.
    WORKERS.get_or_init(|| std::env::var_os("TOKIO_WORKER_THREADS"));
    // SAFETY: nothing else runs yet.
    unsafe { std::env::set_var("TOKIO_WORKER_THREADS", "1") };
    crate::shared::logging::init(DEFAULT_LOG, &[]);
    // The menu, which stays running, hidden between uses: a second run
    // only asks the first to show or hide.
    // Win+X's "Desktop", by itself.
    if std::env::args().any(|arg| arg == "--show-desktop") {
        menu::show_desktop();
        return Ok(());
    }
    let tools = std::env::args().any(|arg| arg == "--tools");
    if tools || std::env::args().any(|arg| arg == "--menu") {
        // Before anything else: it may set ICED_BACKEND.
        backend::init_menu();
        let running = if tools { menu::tools_running() } else { menu::toggle_running() };
        if running {
            return Ok(());
        }
        own_output();
        menu::migrate();
        let settings = cosmic::app::Settings::default()
            .no_main_window(true)
            .exit_on_close(false)
            .transparent(true);
        return cosmic::app::run::<menu::StartMenu>(settings, tools);
    }
    // Else the button, as the panel runs it. It draws on the CPU: the GPU
    // would cost it memory and the driver's wakeups, for a button. The
    // menu, which scrolls long lists, draws on the GPU.
    backend::init_button();
    cosmic::applet::run::<menu::StartButton>(())
}

/// The service's own line to the journal (see `shared::journal`).
fn own_output() {
    crate::shared::journal::own_output("cosmic-taskbar-menu");
}

#[cfg(test)]
mod tests {
    use super::DEFAULT_LOG;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use tracing_subscriber::{EnvFilter, layer::SubscriberExt};

    struct Counter(Arc<AtomicUsize>);

    impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Counter {
        fn on_event(
            &self,
            _event: &tracing::Event<'_>,
            _ctx: tracing_subscriber::layer::Context<'_, S>,
        ) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn the_default_filter_parses_and_quiets_usvg() {
        assert!(DEFAULT_LOG.contains("usvg=error"));
        assert!(EnvFilter::try_new(DEFAULT_LOG).is_ok());
    }

    #[test]
    fn usvg_warnings_are_dropped_by_default() {
        let count = Arc::new(AtomicUsize::new(0));
        let subscriber = tracing_subscriber::registry()
            .with(EnvFilter::new(DEFAULT_LOG))
            .with(Counter(count.clone()));
        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!(target: "usvg::parser::svgtree", "x");
            tracing::warn!(target: "cosmic_ext_applet_start::menu", "y");
            tracing::error!(target: "usvg::parser::svgtree", "z");
        });
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }
}
