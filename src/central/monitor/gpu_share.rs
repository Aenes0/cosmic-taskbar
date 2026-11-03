// SPDX-License-Identifier: GPL-3.0-only

//! The GPU's reading, made once for every monitor's applet. Asking NVIDIA's
//! driver (NVML) costs each process that does it ~21 MB and a thread, which it
//! never gives back, and there is an applet per monitor: the panels' host,
//! always up, reads the GPU and leaves its last reading in a small file in the
//! runtime directory (memory, not disk); the applets read that instead. An
//! applet that finds none, or an old one, reads the GPU itself, as before.

use super::sample::Gpu;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Older than this, the host is not there (it reads every 5 s).
pub const MAX_AGE: Duration = Duration::from_secs(15);

#[derive(Serialize, Deserialize)]
struct Shared {
    /// When it was read, in ms since the epoch.
    at_ms: u64,
    gpu: Gpu,
}

/// The energy counter as last read here: published with every reading, so
/// the one counting the energy always has the latest (a counter read again
/// unchanged adds nothing).
static LAST_ENERGY: Mutex<Option<u64>> = Mutex::new(None);

fn file() -> Option<PathBuf> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")?;
    Some(PathBuf::from(dir).join("cosmic-central").join("gpu.json"))
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// Leaves `gpu` for the applets (the host's part).
pub fn publish(gpu: &Gpu) {
    let Some(path) = file() else {
        return;
    };
    let mut gpu = gpu.clone();
    if let Ok(mut last) = LAST_ENERGY.lock() {
        match gpu.energy_mj {
            Some(energy) => *last = Some(energy),
            None => gpu.energy_mj = *last,
        }
    }
    if let Err(err) = publish_to(&path, &gpu, now_ms()) {
        tracing::warn!("GPU reading not shared: {err}");
    }
}

/// The host's last reading, if it is recent (the applets' part).
pub fn latest() -> Option<Gpu> {
    latest_from(&file()?, now_ms(), MAX_AGE)
}

fn publish_to(path: &Path, gpu: &Gpu, at_ms: u64) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_vec(&Shared { at_ms, gpu: gpu.clone() }).map_err(std::io::Error::other)?;
    // Whole or not at all: a reader never sees half a file.
    let partial = path.with_extension("json.part");
    std::fs::write(&partial, json)?;
    std::fs::rename(&partial, path)
}

fn latest_from(path: &Path, now_ms: u64, max_age: Duration) -> Option<Gpu> {
    let bytes = std::fs::read(path).ok()?;
    let shared: Shared = serde_json::from_slice(&bytes).ok()?;
    let age = now_ms.checked_sub(shared.at_ms)?;
    (age <= max_age.as_millis() as u64).then_some(shared.gpu)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gpu-share-{}-{name}", std::process::id()));
        _ = std::fs::remove_dir_all(&dir);
        dir.join("cosmic-central").join("gpu.json")
    }

    fn gpu() -> Gpu {
        Gpu {
            name: "RTX".into(),
            usage: 42.0,
            vram_used: 1,
            vram_total: 2,
            temp: Some(61.0),
            watts: Some(80.5),
            watts_limit: Some(300.0),
            clock: Some(1800),
            fan: None,
            energy_mj: Some(123_456),
        }
    }

    #[test]
    fn a_published_reading_comes_back_whole() {
        let path = scratch("whole");
        publish_to(&path, &gpu(), 1_000).unwrap();
        let back = latest_from(&path, 2_000, MAX_AGE).unwrap();
        assert_eq!(back.name, "RTX");
        assert_eq!(back.usage, 42.0);
        assert_eq!(back.temp, Some(61.0));
        assert_eq!(back.watts, Some(80.5));
        assert_eq!(back.clock, Some(1800));
        assert_eq!(back.energy_mj, Some(123_456));
        assert!(!path.with_extension("json.part").exists(), "no partial file left");
    }

    #[test]
    fn an_old_reading_or_none_at_all_is_no_reading() {
        let path = scratch("old");
        assert!(latest_from(&path, 1_000, MAX_AGE).is_none(), "no file");
        publish_to(&path, &gpu(), 1_000).unwrap();
        assert!(latest_from(&path, 1_000 + 15_000, MAX_AGE).is_some(), "just in time");
        assert!(latest_from(&path, 1_000 + 15_001, MAX_AGE).is_none(), "too old");
        // A clock gone back: not trusted.
        assert!(latest_from(&path, 500, MAX_AGE).is_none());
    }

    #[test]
    fn a_broken_file_is_no_reading_and_no_panic() {
        let path = scratch("broken");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"{\"at_ms\": 1, \"gpu\": ").unwrap();
        assert!(latest_from(&path, 2, MAX_AGE).is_none());
        std::fs::write(&path, b"").unwrap();
        assert!(latest_from(&path, 2, MAX_AGE).is_none());
    }

    #[test]
    fn a_new_reading_replaces_the_last() {
        let path = scratch("replace");
        publish_to(&path, &gpu(), 1_000).unwrap();
        publish_to(&path, &Gpu { usage: 7.0, ..gpu() }, 2_000).unwrap();
        assert_eq!(latest_from(&path, 2_500, MAX_AGE).unwrap().usage, 7.0);
    }
}
