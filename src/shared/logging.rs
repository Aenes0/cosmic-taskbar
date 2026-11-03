// SPDX-License-Identifier: GPL-3.0-only

//! The log every part starts with: to stderr (the journal, under the panel or
//! a unit), filtered by `RUST_LOG` or a default, with chatty per-frame targets
//! kept at `warn` whatever `RUST_LOG` says.

use tracing_subscriber::EnvFilter;

/// The filter: `rust_log` if it parses, else `default`; then every target of
/// `quiet` at `warn` at most.
pub fn env_filter(rust_log: Option<&str>, default: &str, quiet: &[&str]) -> EnvFilter {
    let base = rust_log
        .filter(|s| !s.trim().is_empty())
        .and_then(|s| EnvFilter::try_new(s).ok())
        .unwrap_or_else(|| EnvFilter::new(default));
    quiet
        .iter()
        .filter_map(|target| format!("{target}=warn").parse().ok())
        .fold(base, |filter, directive| filter.add_directive(directive))
}

/// Starts the log for this process. A write that fails is dropped: told on
/// stderr, if that is what failed, it would abort the program.
pub fn init(default: &str, quiet: &[&str]) {
    let rust_log = std::env::var("RUST_LOG").ok();
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .log_internal_errors(false)
        .with_env_filter(env_filter(rust_log.as_deref(), default, quiet))
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use tracing_subscriber::layer::SubscriberExt;

    struct Counter(Arc<AtomicUsize>);

    impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Counter {
        fn on_event(&self, _: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn count(filter: EnvFilter, emit: impl FnOnce()) -> usize {
        let n = Arc::new(AtomicUsize::new(0));
        let subscriber = tracing_subscriber::registry().with(filter).with(Counter(n.clone()));
        tracing::subscriber::with_default(subscriber, emit);
        n.load(Ordering::SeqCst)
    }

    #[test]
    fn without_rust_log_the_default_applies() {
        let n = count(env_filter(None, "warn,mine=info", &[]), || {
            tracing::info!(target: "mine", "counted");
            tracing::debug!(target: "mine", "not counted");
            tracing::info!(target: "other", "not counted");
            tracing::warn!(target: "other", "counted");
        });
        assert_eq!(n, 2);
    }

    #[test]
    fn quiet_targets_stay_at_warn_even_with_rust_log_debug() {
        let n = count(env_filter(Some("debug"), "warn", &["noisy", "sctk"]), || {
            tracing::debug!(target: "noisy", "not counted");
            tracing::info!(target: "sctk", "not counted");
            tracing::warn!(target: "noisy", "counted");
            tracing::debug!(target: "mine", "counted");
        });
        assert_eq!(n, 2);
    }

    #[test]
    fn a_rust_log_that_does_not_parse_falls_back_to_the_default() {
        let n = count(env_filter(Some("=+=nonsense[["), "warn,mine=info", &[]), || {
            tracing::info!(target: "mine", "counted");
            tracing::info!(target: "other", "not counted");
        });
        assert_eq!(n, 1);
    }

    #[test]
    fn the_start_menu_default_still_drops_usvg_warnings() {
        let n = count(env_filter(None, "warn,cosmic_taskbar=info,usvg=error", &[]), || {
            tracing::warn!(target: "usvg::parser::svgtree", "not counted");
            tracing::error!(target: "usvg::parser::svgtree", "counted");
            tracing::warn!(target: "cosmic_taskbar::start", "counted");
        });
        assert_eq!(n, 2);
    }
}
