// Contadores de desempenho, desligados por padrão.
//
// Ligam só com COSMIC_GREETER_PERF_LOG=<arquivo>: uma thread grava a cada 1 s uma linha
// `t=<epoch_ms> updates=.. views=.. wallpaper_views=.. frames=..` e zera os contadores.
// Desligado, `count` é um load atômico e nada mais.
#![allow(dead_code)]

use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const PERF_LOG_ENV: &str = "COSMIC_GREETER_PERF_LOG";

#[derive(Clone, Copy)]
pub enum Counter {
    Update,
    View,
    WallpaperView,
    Frame,
}

static ENABLED: AtomicBool = AtomicBool::new(false);
static FIRST_VIEW_DONE: AtomicBool = AtomicBool::new(false);
static COUNTS: [AtomicU64; 4] = [
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
];
static LOG: OnceLock<Mutex<File>> = OnceLock::new();

/// Caminho do log a partir do valor da variável; vazio ou ausente = desligado.
pub fn log_path_from(value: Option<&OsStr>) -> Option<PathBuf> {
    value
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

pub fn format_counts(epoch_ms: u128, counts: [u64; 4]) -> String {
    format!(
        "t={epoch_ms} updates={} views={} wallpaper_views={} frames={}",
        counts[0], counts[1], counts[2], counts[3]
    )
}

pub fn format_first_view(epoch_ms: u128) -> String {
    format!("first_view t={epoch_ms}")
}

pub fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn write_line(text: &str) {
    if let Some(log) = LOG.get() {
        if let Ok(mut file) = log.lock() {
            if let Err(err) = writeln!(file, "{text}") {
                tracing::warn!("perf log: {err}");
            }
        }
    }
}

/// Liga os contadores se COSMIC_GREETER_PERF_LOG estiver definida e não vazia.
pub fn init_from_env() {
    let Some(path) = log_path_from(std::env::var_os(PERF_LOG_ENV).as_deref()) else {
        return;
    };
    let file = match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(file) => file,
        Err(err) => {
            tracing::warn!("perf log {}: {err}", path.display());
            return;
        }
    };
    if LOG.set(Mutex::new(file)).is_err() {
        return;
    }
    ENABLED.store(true, Ordering::Relaxed);
    let spawned = std::thread::Builder::new()
        .name("perf-log".into())
        .spawn(|| loop {
            std::thread::sleep(Duration::from_secs(1));
            let mut counts = [0u64; 4];
            for (slot, counter) in counts.iter_mut().zip(COUNTS.iter()) {
                *slot = counter.swap(0, Ordering::Relaxed);
            }
            write_line(&format_counts(now_ms(), counts));
        });
    if let Err(err) = spawned {
        tracing::warn!("perf log thread: {err}");
    }
}

/// Contadores ligados (`COSMIC_GREETER_PERF_LOG`)? Para pular formatação cara.
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn count(counter: Counter) {
    if ENABLED.load(Ordering::Relaxed) {
        COUNTS[counter as usize].fetch_add(1, Ordering::Relaxed);
    }
}

/// Grava `first_view t=..` na primeira chamada (e só nela).
pub fn mark_first_view() {
    if ENABLED.load(Ordering::Relaxed) && !FIRST_VIEW_DONE.swap(true, Ordering::Relaxed) {
        write_line(&format_first_view(now_ms()));
    }
}

/// Linha livre no log (só se ligado).
pub fn line(text: &str) {
    if ENABLED.load(Ordering::Relaxed) {
        write_line(text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::path::PathBuf;

    #[test]
    fn counts_line_format() {
        assert_eq!(
            format_counts(1_700_000_000_123, [3, 6, 3, 0]),
            "t=1700000000123 updates=3 views=6 wallpaper_views=3 frames=0"
        );
    }
    #[test]
    fn first_view_line_format() {
        assert_eq!(format_first_view(1_700_000_000_500), "first_view t=1700000000500");
    }
    #[test]
    fn disabled_without_env() {
        assert_eq!(log_path_from(None), None);
        assert_eq!(log_path_from(Some(OsStr::new(""))), None);
        assert_eq!(log_path_from(Some(OsStr::new("/tmp/x.log"))), Some(PathBuf::from("/tmp/x.log")));
    }
}
