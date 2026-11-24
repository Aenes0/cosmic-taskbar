// SPDX-License-Identifier: GPL-3.0-only

//! The overview's live numbers, worked out from two readings of the system.
//! The arithmetic is the Monitor's own (`shared::rates`), so the panel and the
//! bar agree; the GPU is the one the host already shares (`gpu_share`), so the
//! panel never loads NVIDIA's library. The polling loop belongs to the view.
// Until the overview (a later task) calls it.

use std::time::Duration;

use crate::central::monitor::gpu_share;
use crate::central::monitor::sample::{self, Chip, Depth, Gpu, GpuSource, Reading};
use crate::shared::rates;

/// How often to read while the overview is open and the window has focus.
pub const POLL_FOCUSED: Duration = Duration::from_secs(2);
/// How often to read while the window has no focus.
pub const POLL_UNFOCUSED: Duration = Duration::from_secs(10);

/// What the overview's four cards show.
#[derive(Clone, Debug, PartialEq)]
pub struct HomeMetrics {
    /// Percent, all CPUs.
    pub cpu_pct: f32,
    /// °C.
    pub cpu_temp: Option<f32>,
    /// Average clock, GHz.
    pub cpu_ghz: Option<f32>,
    /// Percent.
    pub gpu_pct: Option<f32>,
    pub gpu_temp: Option<f32>,
    pub gpu_watts: Option<f32>,
    /// Percent of memory in use.
    pub mem_pct: f32,
    /// The CPU's and the GPU's, measured, plus the rest, guessed; none when
    /// neither is measured.
    pub total_watts: Option<f32>,
    /// What `total_watts` costs in an hour, by the tariff (per kWh); none
    /// without a total or a tariff.
    pub cost_per_hour: Option<f32>,
}

/// The CPU's own temperature, the way the Monitor picks it.
fn cpu_temp(chips: &[Chip]) -> Option<f32> {
    chips
        .iter()
        .filter(|c| sample::CPU_CHIPS.contains(&c.kind.as_str()))
        .flat_map(|c| &c.temps)
        .find(|t| matches!(t.label.as_str(), "Tctl" | "Tdie" | "Package id 0") || t.label.starts_with("temp"))
        .map(|t| t.celsius)
}

/// The numbers between two readings (`prev` earlier than `now`), with the
/// GPU as the host shares it, the rest of the machine's watts and the
/// tariff per kWh.
pub fn compute(prev: &Reading, now: &Reading, gpu: Option<&Gpu>, rest_watts: f32, tariff: f32) -> HomeMetrics {
    let dt = now.at.saturating_duration_since(prev.at).as_secs_f32().max(0.001);
    let cpu_pct = match (prev.cpu.first(), now.cpu.first()) {
        (Some(a), Some(b)) => rates::busy_pct((a.busy, a.total), (b.busy, b.total)),
        _ => 0.0,
    };
    let cpu_watts = match (&prev.rapl, &now.rapl) {
        (Some(a), Some(b)) => rates::rapl_watts(a, b, dt),
        _ => None,
    };
    let gpu_watts = gpu.and_then(|g| g.watts);
    let total_watts = if cpu_watts.is_none() && gpu_watts.is_none() {
        None
    } else {
        Some(cpu_watts.unwrap_or(0.0) + gpu_watts.unwrap_or(0.0) + rest_watts)
    };
    HomeMetrics {
        cpu_pct,
        cpu_temp: cpu_temp(&now.chips),
        cpu_ghz: now.frequency.map(|(avg_mhz, _)| avg_mhz / 1000.0),
        gpu_pct: gpu.map(|g| g.usage),
        gpu_temp: gpu.and_then(|g| g.temp),
        gpu_watts,
        mem_pct: now.memory.used as f32 / now.memory.total.max(1) as f32 * 100.0,
        total_watts,
        cost_per_hour: total_watts.filter(|_| tariff > 0.0).map(|w| w / 1000.0 * tariff),
    }
}

/// A reading of what the overview needs: CPU, clock, CPU sensors and memory.
/// The GPU is not sampled here (`gpu_sample: false`): it is the host's shared
/// reading (`gpu_share::latest`), attached afterwards, and NVIDIA's library is
/// never loaded here.
pub fn read_now() -> Reading {
    with_gpu(read_without_gpu(), gpu_share::latest())
}

/// `reading` with the GPU the host shares (none when there is no recent one).
fn with_gpu(reading: Reading, gpu: Option<Gpu>) -> Reading {
    Reading { gpu, ..reading }
}

fn read_without_gpu() -> Reading {
    sample::reading(Depth {
        full: true,
        cpu: true,
        gpu_sample: false,
        ram: true,
        disk: false,
        net: false,
        procs: false,
        gpu_energy: false,
        gpu: GpuSource::Shared,
    })
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use sample::{CpuTimes, Memory, Sampled, Temp};

    fn reading(at: Instant, cpu: (u64, u64), rapl_uj: u64, used_mem: u64) -> Reading {
        Reading {
            at,
            sampled: Sampled::default(),
            cpu: vec![CpuTimes { busy: cpu.0, total: cpu.1 }],
            frequency: Some((3500.0, 4200.0)),
            memory: Memory { total: 64_000, used: used_mem, ..Default::default() },
            chips: vec![Chip {
                kind: "k10temp".into(),
                model: String::new(),
                disk: None,
                temps: vec![Temp { label: "Tctl".into(), celsius: 61.0, critical: None }],
                fans: Vec::new(),
            }],
            gpu: None,
            disks: Vec::new(),
            mounts: Vec::new(),
            net: (0, 0),
            route: None,
            rapl: Some(vec![(rapl_uj, 1_000_000_000)]),
            load: [0.0; 3],
            uptime: 0.0,
            tasks: 0,
            procs: None,
            procs_cpu_total: None,
        }
    }

    #[test]
    fn compute_combines_cpu_memory_gpu_and_cost() {
        let t0 = Instant::now();
        let prev = reading(t0, (100, 1000), 0, 16_000);
        let now = reading(t0 + Duration::from_secs(2), (150, 1100), 40_000_000, 16_000);
        let gpu = Gpu { usage: 30.0, temp: Some(55.0), watts: Some(100.0), ..Default::default() };

        let m = compute(&prev, &now, Some(&gpu), 50.0, 0.8);

        assert_eq!(m.cpu_pct, 50.0);
        assert_eq!(m.mem_pct, 25.0);
        assert_eq!(m.cpu_temp, Some(61.0));
        assert_eq!(m.cpu_ghz, Some(3.5));
        assert_eq!(m.gpu_pct, Some(30.0));
        assert_eq!(m.gpu_temp, Some(55.0));
        assert_eq!(m.gpu_watts, Some(100.0));
        // 40 J in 2 s = 20 W of CPU; 20 + 100 + 50 = 170 W.
        let total = m.total_watts.unwrap();
        assert!((total - 170.0).abs() < 1e-3);
        assert!((m.cost_per_hour.unwrap() - 170.0 / 1000.0 * 0.8).abs() < 1e-4);
    }

    #[test]
    fn with_gpu_attaches_the_hosts_shared_reading_and_compute_uses_it() {
        let t0 = Instant::now();
        let prev = reading(t0, (100, 1000), 0, 16_000);
        let now = reading(t0 + Duration::from_secs(2), (150, 1100), 40_000_000, 16_000);
        assert!(now.gpu.is_none());

        let shared = Gpu { usage: 42.0, temp: Some(60.0), watts: Some(80.0), ..Default::default() };
        let now = with_gpu(now, Some(shared));
        let m = compute(&prev, &now, now.gpu.as_ref(), 50.0, 0.0);
        assert_eq!((m.gpu_pct, m.gpu_temp, m.gpu_watts), (Some(42.0), Some(60.0), Some(80.0)));

        let none = with_gpu(now, None);
        assert!(none.gpu.is_none());
    }

    #[test]
    fn compute_without_gpu_leaves_its_fields_empty_and_totals_cpu_plus_rest() {
        let t0 = Instant::now();
        let prev = reading(t0, (100, 1000), 0, 16_000);
        let now = reading(t0 + Duration::from_secs(2), (150, 1100), 40_000_000, 16_000);

        let m = compute(&prev, &now, None, 50.0, 0.8);

        assert_eq!((m.gpu_pct, m.gpu_temp, m.gpu_watts), (None, None, None));
        assert!((m.total_watts.unwrap() - 70.0).abs() < 1e-3);
    }

    #[test]
    fn compute_without_any_measured_watts_has_no_total_or_cost() {
        let t0 = Instant::now();
        let prev = Reading { rapl: None, ..reading(t0, (100, 1000), 0, 16_000) };
        let now = Reading { rapl: None, ..reading(t0 + Duration::from_secs(2), (150, 1100), 0, 16_000) };

        let m = compute(&prev, &now, None, 50.0, 0.8);

        assert_eq!((m.total_watts, m.cost_per_hour), (None, None));
    }

    #[test]
    fn compute_without_tariff_has_no_cost() {
        let t0 = Instant::now();
        let prev = reading(t0, (100, 1000), 0, 16_000);
        let now = reading(t0 + Duration::from_secs(2), (150, 1100), 40_000_000, 16_000);

        let m = compute(&prev, &now, None, 0.0, 0.0);

        assert!(m.total_watts.is_some());
        assert_eq!(m.cost_per_hour, None);
    }
}
