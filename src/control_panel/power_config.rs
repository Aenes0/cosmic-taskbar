// SPDX-License-Identifier: GPL-3.0-only

//! Configurable power profiles (`~/.config/cosmic-taskbar/power-profiles.conf`)
//! and per-profile COSMIC idle / screen-off / lock / suspend timeouts
//! (`~/.config/cosmic/com.system76.CosmicIdle/v1/`).

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};

use tokio::process::Command;

/// Supported `power-profiles-daemon` profiles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerProfileKind {
    PowerSaver,
    Balanced,
    Performance,
}

impl PowerProfileKind {
    pub const ALL: [PowerProfileKind; 3] = [
        PowerProfileKind::PowerSaver,
        PowerProfileKind::Balanced,
        PowerProfileKind::Performance,
    ];

    pub fn id(self) -> &'static str {
        match self {
            PowerProfileKind::PowerSaver => "power-saver",
            PowerProfileKind::Balanced => "balanced",
            PowerProfileKind::Performance => "performance",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PowerProfileKind::PowerSaver => "Economia",
            PowerProfileKind::Balanced => "Equilibrado",
            PowerProfileKind::Performance => "Desempenho",
        }
    }

    pub fn env_prefix(self) -> &'static str {
        match self {
            PowerProfileKind::PowerSaver => "POWER_SAVER",
            PowerProfileKind::Balanced => "BALANCED",
            PowerProfileKind::Performance => "PERFORMANCE",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id.trim() {
            "power-saver" | "on" => Some(PowerProfileKind::PowerSaver),
            "balanced" => Some(PowerProfileKind::Balanced),
            "performance" => Some(PowerProfileKind::Performance),
            _ => None,
        }
    }
}

/// EPP options `(raw_value, human_label)`.
pub const EPP_OPTIONS: &[(&str, &str)] = &[
    ("", "Manter"),
    ("power", "Energia"),
    ("balance_power", "Equil. economia"),
    ("balance_performance", "Equil. desempenho"),
    ("performance", "Desempenho"),
];

/// PCIe ASPM policy options `(raw_value, human_label)`.
pub const ASPM_OPTIONS: &[(&str, &str)] = &[
    ("powersupersave", "Super economia"),
    ("powersave", "Economia"),
    ("default", "Padrão"),
];

/// SATA ALPM policy options `(raw_value, human_label)`.
pub const SATA_ALPM_OPTIONS: &[(&str, &str)] = &[
    ("med_power_with_dipm", "Economia (DIPM)"),
    ("max_performance", "Máx. desempenho"),
];

/// Common idle timeout presets in minutes (`0` = never).
pub const IDLE_MIN_PRESETS: &[u32] = &[0, 1, 2, 3, 5, 10, 15, 20, 30, 45, 60, 120];

/// Complete hardware + idle tuning for one power profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileTuning {
    pub boost: bool,
    /// CPU max scaling frequency in MHz (`0` = hardware max).
    pub cpu_max_mhz: u32,
    /// Energy Performance Preference: `"power"`, `"balance_power"`,
    /// `"balance_performance"`, `"performance"`, or `""` (keep).
    pub epp: String,
    /// Intel `intel_pstate/max_perf_pct` (`10..=100`).
    pub intel_max_perf_pct: u32,
    /// Intel RAPL PL1 sustained limit in Watts (`0` = hardware default).
    pub rapl_pl1_w: u32,
    /// Intel RAPL PL2 short-turbo limit in Watts (`0` = hardware default).
    pub rapl_pl2_w: u32,
    /// On dual-CCD AMD CPUs (e.g. Ryzen 9), offline CCD1 CPUs via
    /// `/sys/devices/system/cpu/cpuN/online` when `true`.
    pub disable_ccd1: bool,
    /// Maximum physical cores kept online (`0` = all online; >0 keeps N physical
    /// cores and their SMT siblings online, always keeping `cpu0`).
    pub max_online_cores: u32,
    /// NVIDIA GPU power limit in Watts (`0` = min in power-saver / default in performance).
    pub nvidia_power_limit_w: u32,
    /// NVIDIA locked graphics clock ceiling in MHz (`0` = unlocked).
    pub nvidia_gpu_clock_mhz: u32,
    /// NVIDIA locked memory clock ceiling in MHz (`0` = unlocked).
    pub nvidia_mem_clock_mhz: u32,
    /// NVIDIA persistence mode (`nvidia-smi -pm 1|0`).
    pub nvidia_persistence: bool,
    /// Intel Arc / i915 / xe iGPU max frequency in MHz (`0` = hardware max / RP0).
    pub intel_igpu_max_mhz: u32,
    /// Display refresh rate string (e.g. `"59.987"`, `"159.962"`, or `""` = unchanged).
    pub display_hz: String,
    /// PCIe ASPM policy (`"powersupersave"`, `"powersave"`, `"default"`).
    pub pcie_aspm: String,
    /// SATA ALPM policy (`"med_power_with_dipm"`, `"max_performance"`).
    pub sata_alpm: String,
    /// `snd_hda_intel` power_save (`true` = 1, `false` = 0).
    pub audio_power_save: bool,
    /// Wi-Fi `iw dev <dev> set power_save on|off`.
    pub wifi_power_save: bool,
    /// Minutes until turning off display (`0` = never; `CosmicIdle/v1/screen_off_time`).
    pub screen_off_mins: u32,
    /// Minutes until locking / asking password (`0` = never; `CosmicIdle/v1/lock_time`).
    pub lock_mins: u32,
    /// Minutes until suspending on AC (`0` = never; `CosmicIdle/v1/suspend_on_ac_time`).
    pub suspend_ac_mins: u32,
    /// Minutes until suspending on battery (`0` = never; `CosmicIdle/v1/suspend_on_battery_time`).
    pub suspend_battery_mins: u32,
}

impl ProfileTuning {
    /// Human-readable duration label for UI summaries (`0` -> `"Nunca"`).
    pub fn format_mins(mins: u32) -> String {
        if mins == 0 {
            "Nunca".to_owned()
        } else if mins < 60 {
            format!("{mins} min")
        } else if mins % 60 == 0 {
            format!("{} h", mins / 60)
        } else {
            format!("{} h {} min", mins / 60, mins % 60)
        }
    }
}

/// All three configurable power profiles persisted in
/// `~/.config/cosmic-taskbar/power-profiles.conf`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PowerProfilesConfig {
    pub power_saver: ProfileTuning,
    pub balanced: ProfileTuning,
    pub performance: ProfileTuning,
}

/// Detected local CPU topology useful for configuring CCD1 and online-core limits.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CpuTopologyInfo {
    pub is_amd_dual_ccd: bool,
    pub physical_cores: u32,
    pub logical_cpus: u32,
    pub hw_max_mhz: u32,
}

impl PowerProfilesConfig {
    /// Hardware-specific defaults preserving the exact original tuning for the
    /// AMD Ryzen 9 desktop and the Intel Core Ultra 7 155H laptop.
    pub fn defaults_for(is_intel: bool, is_laptop: bool) -> Self {
        if is_intel && is_laptop {
            Self {
                power_saver: ProfileTuning {
                    boost: false,
                    cpu_max_mhz: 0,
                    epp: "power".to_owned(),
                    intel_max_perf_pct: 55,
                    rapl_pl1_w: 12,
                    rapl_pl2_w: 18,
                    disable_ccd1: false,
                    max_online_cores: 0,
                    nvidia_power_limit_w: 0,
                    nvidia_gpu_clock_mhz: 0,
                    nvidia_mem_clock_mhz: 0,
                    nvidia_persistence: false,
                    intel_igpu_max_mhz: 800,
                    display_hz: String::new(),
                    pcie_aspm: "powersupersave".to_owned(),
                    sata_alpm: "med_power_with_dipm".to_owned(),
                    audio_power_save: true,
                    wifi_power_save: true,
                    screen_off_mins: 5,
                    lock_mins: 5,
                    suspend_ac_mins: 30,
                    suspend_battery_mins: 15,
                },
                balanced: ProfileTuning {
                    boost: true,
                    cpu_max_mhz: 0,
                    epp: "balance_power".to_owned(),
                    intel_max_perf_pct: 85,
                    rapl_pl1_w: 28,
                    rapl_pl2_w: 45,
                    disable_ccd1: false,
                    max_online_cores: 0,
                    nvidia_power_limit_w: 0,
                    nvidia_gpu_clock_mhz: 0,
                    nvidia_mem_clock_mhz: 0,
                    nvidia_persistence: false,
                    intel_igpu_max_mhz: 1500,
                    display_hz: String::new(),
                    pcie_aspm: "powersave".to_owned(),
                    sata_alpm: "med_power_with_dipm".to_owned(),
                    audio_power_save: true,
                    wifi_power_save: true,
                    screen_off_mins: 10,
                    lock_mins: 10,
                    suspend_ac_mins: 60,
                    suspend_battery_mins: 30,
                },
                performance: ProfileTuning {
                    boost: true,
                    cpu_max_mhz: 0,
                    epp: "performance".to_owned(),
                    intel_max_perf_pct: 100,
                    rapl_pl1_w: 45,
                    rapl_pl2_w: 65,
                    disable_ccd1: false,
                    max_online_cores: 0,
                    nvidia_power_limit_w: 0,
                    nvidia_gpu_clock_mhz: 0,
                    nvidia_mem_clock_mhz: 0,
                    nvidia_persistence: false,
                    intel_igpu_max_mhz: 0,
                    display_hz: String::new(),
                    pcie_aspm: "default".to_owned(),
                    sata_alpm: "max_performance".to_owned(),
                    audio_power_save: false,
                    wifi_power_save: false,
                    screen_off_mins: 15,
                    lock_mins: 15,
                    suspend_ac_mins: 0,
                    suspend_battery_mins: 60,
                },
            }
        } else {
            Self {
                power_saver: ProfileTuning {
                    boost: false,
                    cpu_max_mhz: 2400,
                    epp: "power".to_owned(),
                    intel_max_perf_pct: 100,
                    rapl_pl1_w: 0,
                    rapl_pl2_w: 0,
                    disable_ccd1: false,
                    max_online_cores: 0,
                    nvidia_power_limit_w: 0,
                    nvidia_gpu_clock_mhz: 1000,
                    nvidia_mem_clock_mhz: 810,
                    nvidia_persistence: true,
                    intel_igpu_max_mhz: 0,
                    display_hz: "59.987".to_owned(),
                    pcie_aspm: "powersave".to_owned(),
                    sata_alpm: "med_power_with_dipm".to_owned(),
                    audio_power_save: true,
                    wifi_power_save: false,
                    screen_off_mins: 5,
                    lock_mins: 5,
                    suspend_ac_mins: 30,
                    suspend_battery_mins: 0,
                },
                balanced: ProfileTuning {
                    boost: true,
                    cpu_max_mhz: 4400,
                    epp: "balance_power".to_owned(),
                    intel_max_perf_pct: 100,
                    rapl_pl1_w: 0,
                    rapl_pl2_w: 0,
                    disable_ccd1: false,
                    max_online_cores: 0,
                    nvidia_power_limit_w: 200,
                    nvidia_gpu_clock_mhz: 0,
                    nvidia_mem_clock_mhz: 0,
                    nvidia_persistence: true,
                    intel_igpu_max_mhz: 0,
                    display_hz: "159.962".to_owned(),
                    pcie_aspm: "powersave".to_owned(),
                    sata_alpm: "med_power_with_dipm".to_owned(),
                    audio_power_save: true,
                    wifi_power_save: false,
                    screen_off_mins: 10,
                    lock_mins: 10,
                    suspend_ac_mins: 60,
                    suspend_battery_mins: 0,
                },
                performance: ProfileTuning {
                    boost: true,
                    cpu_max_mhz: 0,
                    epp: "performance".to_owned(),
                    intel_max_perf_pct: 100,
                    rapl_pl1_w: 0,
                    rapl_pl2_w: 0,
                    disable_ccd1: false,
                    max_online_cores: 0,
                    nvidia_power_limit_w: 0,
                    nvidia_gpu_clock_mhz: 0,
                    nvidia_mem_clock_mhz: 0,
                    nvidia_persistence: false,
                    intel_igpu_max_mhz: 0,
                    display_hz: "159.962".to_owned(),
                    pcie_aspm: "default".to_owned(),
                    sata_alpm: "max_performance".to_owned(),
                    audio_power_save: false,
                    wifi_power_save: false,
                    screen_off_mins: 0,
                    lock_mins: 0,
                    suspend_ac_mins: 0,
                    suspend_battery_mins: 0,
                },
            }
        }
    }

    pub fn get(&self, kind: PowerProfileKind) -> &ProfileTuning {
        match kind {
            PowerProfileKind::PowerSaver => &self.power_saver,
            PowerProfileKind::Balanced => &self.balanced,
            PowerProfileKind::Performance => &self.performance,
        }
    }

    pub fn get_mut(&mut self, kind: PowerProfileKind) -> &mut ProfileTuning {
        match kind {
            PowerProfileKind::PowerSaver => &mut self.power_saver,
            PowerProfileKind::Balanced => &mut self.balanced,
            PowerProfileKind::Performance => &mut self.performance,
        }
    }

    pub fn for_profile_id(&self, id: &str) -> Option<&ProfileTuning> {
        PowerProfileKind::from_id(id).map(|k| self.get(k))
    }

    /// Path to `~/.config/cosmic-taskbar/power-profiles.conf`.
    pub fn config_path() -> PathBuf {
        xdg_config_dir().join("cosmic-taskbar/power-profiles.conf")
    }

    /// Loads `power-profiles.conf` layered over `defaults_for(is_intel, is_laptop)`.
    pub fn load(is_intel: bool, is_laptop: bool) -> Self {
        let base = Self::defaults_for(is_intel, is_laptop);
        let path = Self::config_path();
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::parse_conf(&text, base),
            Err(_) => base,
        }
    }

    /// Saves all profiles to `~/.config/cosmic-taskbar/power-profiles.conf`
    /// in simple shell-sourceable `KEY=VALUE` format.
    pub fn save(&self) -> io::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        write_atomic(&path, self.to_conf_string().as_bytes())
    }

    /// Formats the configuration as shell-sourceable `KEY=VALUE` lines.
    pub fn to_conf_string(&self) -> String {
        let mut out = String::with_capacity(2048);
        out.push_str("# Perfis de energia configuráveis do cosmic-taskbar / economia-energia\n");
        out.push_str("# Formato KEY=VALUE (compatível com POSIX sh).\n\n");
        for kind in PowerProfileKind::ALL {
            write_profile_section(&mut out, kind, self.get(kind));
            out.push('\n');
        }
        out
    }

    /// Parses shell-sourceable `KEY=VALUE` text onto `base`.
    pub fn parse_conf(text: &str, mut base: Self) -> Self {
        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((raw_key, raw_val)) = line.split_once('=') else {
                continue;
            };
            let key = raw_key.trim();
            let val = strip_quotes(raw_val.trim());

            let (tuning, suffix) = if let Some(s) = key.strip_prefix("POWER_SAVER_") {
                (&mut base.power_saver, s)
            } else if let Some(s) = key.strip_prefix("BALANCED_") {
                (&mut base.balanced, s)
            } else if let Some(s) = key.strip_prefix("PERFORMANCE_") {
                (&mut base.performance, s)
            } else {
                continue;
            };

            apply_kv(tuning, suffix, val);
        }
        base
    }
}

fn xdg_config_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        PathBuf::from(xdg)
    } else {
        PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config")
    }
}

/// Directory `~/.config/cosmic/com.system76.CosmicIdle/v1`.
pub fn cosmic_idle_dir() -> PathBuf {
    xdg_config_dir().join("cosmic/com.system76.CosmicIdle/v1")
}

/// Formats minutes into COSMIC Idle RON (`Some(<ms>)` or `None`).
pub fn format_cosmic_idle_mins(mins: u32) -> String {
    if mins == 0 {
        "None".to_owned()
    } else {
        let ms = u64::from(mins).saturating_mul(60_000);
        format!("Some({ms})")
    }
}

/// Writes `screen_off_time`, `lock_time`, `suspend_on_ac_time`, and
/// `suspend_on_battery_time` to `~/.config/cosmic/com.system76.CosmicIdle/v1/`.
pub fn apply_idle_timeouts(tuning: &ProfileTuning) -> io::Result<()> {
    apply_idle_timeouts_to_dir(&cosmic_idle_dir(), tuning)
}

/// Writes the COSMIC Idle RON files into a specific directory (used by tests and
/// `apply_idle_timeouts`).
pub fn apply_idle_timeouts_to_dir(dir: &Path, tuning: &ProfileTuning) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let entries = [
        ("screen_off_time", tuning.screen_off_mins),
        ("lock_time", tuning.lock_mins),
        ("suspend_on_ac_time", tuning.suspend_ac_mins),
        ("suspend_on_battery_time", tuning.suspend_battery_mins),
    ];
    for (file_name, mins) in entries {
        let ron = format_cosmic_idle_mins(mins);
        write_atomic(&dir.join(file_name), ron.as_bytes())?;
    }
    Ok(())
}

/// Applies idle timeouts immediately as the user and invokes `economia-energia`
/// via `pkexec` so hardware settings for `profile_id` take effect right away.
pub async fn apply_profile_now(profile_id: &str, tuning: &ProfileTuning) -> String {
    let Some(kind) = PowerProfileKind::from_id(profile_id) else {
        return format!("Plano de energia desconhecido: {profile_id}");
    };
    let idle_err = apply_idle_timeouts(tuning).err();

    // If `economia-energia.service` is already running as root, switching via
    // `power-profiles-daemon` (zero password!) lets the daemon apply all
    // hardware settings without ever prompting for `pkexec`/`sudo`.
    let daemon_active = Command::new("systemctl")
        .args(["is-active", "--quiet", "economia-energia.service"])
        .status()
        .await
        .is_ok_and(|s| s.success());
    if daemon_active {
        let bounce_to = if kind == PowerProfileKind::Balanced {
            "power-saver"
        } else {
            "balanced"
        };
        let _ = crate::central::services::set_power_profile(bounce_to.to_owned()).await;
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
        if crate::central::services::set_power_profile(kind.id().to_owned())
            .await
            .is_ok()
        {
            return match idle_err {
                Some(e) => format!(
                    "Plano {} aplicado via daemon (sem senha), mas houve erro ao salvar tempos de tela: {e}",
                    kind.label()
                ),
                None => format!("✓ Plano {} salvo e aplicado automaticamente (sem senha).", kind.label()),
            };
        }
    }

    let installed = PathBuf::from("/usr/local/bin/economia-energia");
    let (use_sh, script) = if installed.is_file() {
        (false, installed)
    } else if let Some(repo) = super::data::find_repo_path() {
        let local = repo.join("extras/power/economia-energia");
        if local.is_file() {
            (true, local)
        } else {
            return match idle_err {
                Some(e) => format!("Falha ao gravar CosmicIdle ({e}) e economia-energia não encontrado."),
                None => "Configuração salva e tempos de tela aplicados, mas economia-energia não foi encontrado.".to_owned(),
            };
        }
    } else {
        return match idle_err {
            Some(e) => format!("Falha ao gravar CosmicIdle ({e}) e /usr/local/bin/economia-energia não está instalado."),
            None => "Configuração salva e tempos de tela aplicados, mas /usr/local/bin/economia-energia não está instalado.".to_owned(),
        };
    };

    let mut cmd = Command::new("pkexec");
    if use_sh {
        cmd.arg("/bin/sh");
    }
    cmd.arg(&script).arg(kind.id());

    match cmd.output().await {
        Ok(out) if out.status.success() => match idle_err {
            Some(e) => format!(
                "Plano {} aplicado no hardware, mas houve erro ao salvar tempos de tela: {e}",
                kind.label()
            ),
            None => format!("Plano {} salvo e aplicado com sucesso.", kind.label()),
        },
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            let detail = if !stderr.is_empty() { stderr } else { stdout };
            if detail.is_empty() {
                format!(
                    "Configuração salva, mas economia-energia retornou código {} (autenticação cancelada ou erro).",
                    out.status.code().unwrap_or(-1)
                )
            } else {
                format!(
                    "Configuração salva, mas economia-energia falhou: {}",
                    detail.lines().last().unwrap_or(&detail)
                )
            }
        }
        Err(err) => format!("Configuração salva, mas falhou ao executar pkexec: {err}"),
    }
}

/// Probes `/sys/devices/system/cpu` to report physical cores, logical CPUs,
/// hardware max MHz, and whether the CPU is a dual-CCD AMD Ryzen.
pub fn detect_cpu_topology() -> CpuTopologyInfo {
    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let is_amd = cpuinfo
        .lines()
        .find_map(|l| l.strip_prefix("vendor_id"))
        .is_some_and(|rest| rest.contains("AuthenticAMD"));

    let mut logical_cpus = 0u32;
    let mut physical_set: BTreeSet<(String, String)> = BTreeSet::new();
    let mut l3_domains: BTreeSet<(String, String)> = BTreeSet::new();
    let mut hw_max_khz = 0u32;

    if let Ok(entries) = std::fs::read_dir("/sys/devices/system/cpu") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(s) = name.to_str() else { continue };
            let Some(num_str) = s.strip_prefix("cpu") else {
                continue;
            };
            if num_str.is_empty() || !num_str.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            logical_cpus += 1;
            let base = entry.path();
            let pkg = std::fs::read_to_string(base.join("topology/physical_package_id"))
                .unwrap_or_default()
                .trim()
                .to_owned();
            if let Ok(core_id) = std::fs::read_to_string(base.join("topology/core_id")) {
                let cid = core_id.trim().to_owned();
                if !cid.is_empty() {
                    physical_set.insert((pkg.clone(), cid));
                }
            }
            if let Ok(l3) = std::fs::read_to_string(base.join("cache/index3/id")) {
                let l3_id = l3.trim().to_owned();
                if !l3_id.is_empty() {
                    l3_domains.insert((pkg, l3_id));
                }
            }
            if let Ok(khz_str) = std::fs::read_to_string(base.join("cpufreq/cpuinfo_max_freq"))
                && let Ok(khz) = khz_str.trim().parse::<u32>()
            {
                hw_max_khz = hw_max_khz.max(khz);
            }
        }
    }

    let physical_cores = if physical_set.is_empty() {
        logical_cpus
    } else {
        physical_set.len() as u32
    };

    CpuTopologyInfo {
        is_amd_dual_ccd: is_amd && l3_domains.len() > 1,
        physical_cores,
        logical_cpus,
        hw_max_mhz: hw_max_khz / 1000,
    }
}

fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("config");
    let tmp_name = format!(".{file_name}.tmp.{}", std::process::id());
    let tmp_path = path.with_file_name(tmp_name);
    std::fs::write(&tmp_path, data)?;
    std::fs::rename(&tmp_path, path)
}

fn write_profile_section(out: &mut String, kind: PowerProfileKind, t: &ProfileTuning) {
    let p = kind.env_prefix();
    let b = |v: bool| if v { 1u8 } else { 0u8 };
    let epp = sanitize_epp(&t.epp);
    let hz = sanitize_hz(&t.display_hz);
    let aspm = sanitize_aspm(&t.pcie_aspm);
    let sata = sanitize_sata(&t.sata_alpm);

    out.push_str(&format!("# --- {} ({}) ---\n", kind.label(), kind.id()));
    out.push_str(&format!("{p}_BOOST={}\n", b(t.boost)));
    out.push_str(&format!("{p}_CPU_MAX_MHZ={}\n", t.cpu_max_mhz));
    out.push_str(&format!("{p}_EPP={epp}\n"));
    out.push_str(&format!(
        "{p}_INTEL_MAX_PERF_PCT={}\n",
        t.intel_max_perf_pct.clamp(10, 100)
    ));
    out.push_str(&format!("{p}_RAPL_PL1_W={}\n", t.rapl_pl1_w));
    out.push_str(&format!("{p}_RAPL_PL2_W={}\n", t.rapl_pl2_w));
    out.push_str(&format!("{p}_DISABLE_CCD1={}\n", b(t.disable_ccd1)));
    out.push_str(&format!("{p}_MAX_ONLINE_CORES={}\n", t.max_online_cores));
    out.push_str(&format!(
        "{p}_NVIDIA_POWER_LIMIT_W={}\n",
        t.nvidia_power_limit_w
    ));
    out.push_str(&format!(
        "{p}_NVIDIA_GPU_CLOCK_MHZ={}\n",
        t.nvidia_gpu_clock_mhz
    ));
    out.push_str(&format!(
        "{p}_NVIDIA_MEM_CLOCK_MHZ={}\n",
        t.nvidia_mem_clock_mhz
    ));
    out.push_str(&format!(
        "{p}_NVIDIA_PERSISTENCE={}\n",
        b(t.nvidia_persistence)
    ));
    out.push_str(&format!(
        "{p}_INTEL_IGPU_MAX_MHZ={}\n",
        t.intel_igpu_max_mhz
    ));
    out.push_str(&format!("{p}_DISPLAY_HZ={hz}\n"));
    out.push_str(&format!("{p}_PCIE_ASPM={aspm}\n"));
    out.push_str(&format!("{p}_SATA_ALPM={sata}\n"));
    out.push_str(&format!("{p}_AUDIO_POWER_SAVE={}\n", b(t.audio_power_save)));
    out.push_str(&format!("{p}_WIFI_POWER_SAVE={}\n", b(t.wifi_power_save)));
    out.push_str(&format!("{p}_SCREEN_OFF_MINS={}\n", t.screen_off_mins));
    out.push_str(&format!("{p}_LOCK_MINS={}\n", t.lock_mins));
    out.push_str(&format!("{p}_SUSPEND_AC_MINS={}\n", t.suspend_ac_mins));
    out.push_str(&format!(
        "{p}_SUSPEND_BATTERY_MINS={}\n",
        t.suspend_battery_mins
    ));
}

fn strip_quotes(val: &str) -> &str {
    if let Some(inner) = val.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        inner
    } else if let Some(inner) = val.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        inner
    } else {
        val
    }
}

fn parse_bool(val: &str) -> Option<bool> {
    match val.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn sanitize_epp(val: &str) -> &str {
    match val.trim() {
        "" => "",
        "power" => "power",
        "balance_power" => "balance_power",
        "balance_performance" => "balance_performance",
        "performance" => "performance",
        _ => "",
    }
}

fn sanitize_aspm(val: &str) -> &str {
    match val.trim() {
        "powersupersave" => "powersupersave",
        "powersave" => "powersave",
        "default" => "default",
        _ => "default",
    }
}

fn sanitize_sata(val: &str) -> &str {
    match val.trim() {
        "med_power_with_dipm" => "med_power_with_dipm",
        "max_performance" => "max_performance",
        "min_power" => "min_power",
        "medium_power" => "medium_power",
        _ => "max_performance",
    }
}

fn sanitize_hz(val: &str) -> String {
    val.trim()
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect()
}

fn apply_kv(t: &mut ProfileTuning, suffix: &str, val: &str) {
    match suffix {
        "BOOST" => {
            if let Some(v) = parse_bool(val) {
                t.boost = v;
            }
        }
        "CPU_MAX_MHZ" => {
            if let Ok(v) = val.parse::<u32>() {
                t.cpu_max_mhz = v;
            }
        }
        "EPP" => {
            let trimmed = val.trim();
            if matches!(
                trimmed,
                "" | "power" | "balance_power" | "balance_performance" | "performance"
            ) {
                t.epp = trimmed.to_owned();
            }
        }
        "INTEL_MAX_PERF_PCT" => {
            if let Ok(v) = val.parse::<u32>() {
                t.intel_max_perf_pct = v.clamp(10, 100);
            }
        }
        "RAPL_PL1_W" => {
            if let Ok(v) = val.parse::<u32>() {
                t.rapl_pl1_w = v;
            }
        }
        "RAPL_PL2_W" => {
            if let Ok(v) = val.parse::<u32>() {
                t.rapl_pl2_w = v;
            }
        }
        "DISABLE_CCD1" => {
            if let Some(v) = parse_bool(val) {
                t.disable_ccd1 = v;
            }
        }
        "MAX_ONLINE_CORES" => {
            if let Ok(v) = val.parse::<u32>() {
                t.max_online_cores = v;
            }
        }
        "NVIDIA_POWER_LIMIT_W" => {
            if let Ok(v) = val.parse::<u32>() {
                t.nvidia_power_limit_w = v;
            }
        }
        "NVIDIA_GPU_CLOCK_MHZ" => {
            if let Ok(v) = val.parse::<u32>() {
                t.nvidia_gpu_clock_mhz = v;
            }
        }
        "NVIDIA_MEM_CLOCK_MHZ" => {
            if let Ok(v) = val.parse::<u32>() {
                t.nvidia_mem_clock_mhz = v;
            }
        }
        "NVIDIA_PERSISTENCE" => {
            if let Some(v) = parse_bool(val) {
                t.nvidia_persistence = v;
            }
        }
        "INTEL_IGPU_MAX_MHZ" => {
            if let Ok(v) = val.parse::<u32>() {
                t.intel_igpu_max_mhz = v;
            }
        }
        "DISPLAY_HZ" => {
            t.display_hz = sanitize_hz(val);
        }
        "PCIE_ASPM" => {
            let trimmed = val.trim();
            if matches!(trimmed, "powersupersave" | "powersave" | "default") {
                t.pcie_aspm = trimmed.to_owned();
            }
        }
        "SATA_ALPM" => {
            let trimmed = val.trim();
            if matches!(
                trimmed,
                "med_power_with_dipm" | "max_performance" | "min_power" | "medium_power"
            ) {
                t.sata_alpm = trimmed.to_owned();
            }
        }
        "AUDIO_POWER_SAVE" => {
            if let Some(v) = parse_bool(val) {
                t.audio_power_save = v;
            }
        }
        "WIFI_POWER_SAVE" => {
            if let Some(v) = parse_bool(val) {
                t.wifi_power_save = v;
            }
        }
        "SCREEN_OFF_MINS" => {
            if let Ok(v) = val.parse::<u32>() {
                t.screen_off_mins = v;
            }
        }
        "LOCK_MINS" => {
            if let Ok(v) = val.parse::<u32>() {
                t.lock_mins = v;
            }
        }
        "SUSPEND_AC_MINS" => {
            if let Ok(v) = val.parse::<u32>() {
                t.suspend_ac_mins = v;
            }
        }
        "SUSPEND_BATTERY_MINS" => {
            if let Ok(v) = val.parse::<u32>() {
                t.suspend_battery_mins = v;
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_amd_desktop_and_custom_overrides() {
        let mut cfg = PowerProfilesConfig::defaults_for(false, false);
        assert!(!cfg.power_saver.boost);
        assert_eq!(cfg.power_saver.cpu_max_mhz, 2400);
        assert_eq!(cfg.power_saver.nvidia_gpu_clock_mhz, 1000);
        assert_eq!(cfg.power_saver.nvidia_mem_clock_mhz, 810);
        assert_eq!(cfg.power_saver.display_hz, "59.987");
        assert_eq!(cfg.balanced.cpu_max_mhz, 4400);
        assert_eq!(cfg.balanced.nvidia_power_limit_w, 200);
        assert_eq!(cfg.balanced.display_hz, "159.962");
        assert!(cfg.performance.boost);
        assert_eq!(cfg.performance.cpu_max_mhz, 0);

        // Customize AMD CCD1, core limit, clocks, and idle timeouts.
        cfg.power_saver.disable_ccd1 = true;
        cfg.power_saver.max_online_cores = 6;
        cfg.power_saver.screen_off_mins = 3;
        cfg.power_saver.lock_mins = 5;
        cfg.power_saver.suspend_ac_mins = 20;
        cfg.power_saver.suspend_battery_mins = 10;
        cfg.balanced.nvidia_gpu_clock_mhz = 1800;
        cfg.balanced.epp = "balance_performance".to_owned();
        cfg.performance.display_hz = String::new();

        let text = cfg.to_conf_string();
        // Verify every non-comment line is safe shell-sourceable KEY=VALUE.
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let (k, v) = trimmed.split_once('=').expect("KEY=VALUE format");
            assert!(
                k.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'),
                "unsafe key: {k}"
            );
            assert!(
                v.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')),
                "unsafe value for {k}: {v}"
            );
        }

        let parsed = PowerProfilesConfig::parse_conf(
            &text,
            PowerProfilesConfig::defaults_for(true, true),
        );
        assert_eq!(parsed, cfg);
    }

    #[test]
    fn round_trip_intel_laptop_defaults() {
        let cfg = PowerProfilesConfig::defaults_for(true, true);
        assert!(!cfg.power_saver.boost);
        assert_eq!(cfg.power_saver.intel_max_perf_pct, 55);
        assert_eq!(cfg.power_saver.rapl_pl1_w, 12);
        assert_eq!(cfg.power_saver.rapl_pl2_w, 18);
        assert_eq!(cfg.power_saver.intel_igpu_max_mhz, 800);
        assert_eq!(cfg.power_saver.pcie_aspm, "powersupersave");
        assert!(cfg.power_saver.wifi_power_save);

        assert_eq!(cfg.balanced.intel_max_perf_pct, 85);
        assert_eq!(cfg.balanced.rapl_pl1_w, 28);
        assert_eq!(cfg.balanced.rapl_pl2_w, 45);
        assert_eq!(cfg.balanced.intel_igpu_max_mhz, 1500);

        assert_eq!(cfg.performance.intel_max_perf_pct, 100);
        assert_eq!(cfg.performance.rapl_pl1_w, 45);
        assert_eq!(cfg.performance.rapl_pl2_w, 65);
        assert_eq!(cfg.performance.intel_igpu_max_mhz, 0);
        assert!(!cfg.performance.wifi_power_save);

        let conf = cfg.to_conf_string();
        let parsed = PowerProfilesConfig::parse_conf(
            &conf,
            PowerProfilesConfig::defaults_for(false, false),
        );
        assert_eq!(parsed, cfg);
    }

    #[test]
    fn parse_conf_clamps_and_tolerates_quotes() {
        let raw = r#"
# Comentário
POWER_SAVER_INTEL_MAX_PERF_PCT=3
POWER_SAVER_EPP="power"
POWER_SAVER_DISPLAY_HZ='60.0;rm -rf /'
BALANCED_BOOST=false
PERFORMANCE_PCIE_ASPM=invalid_policy
"#;
        let base = PowerProfilesConfig::defaults_for(true, true);
        let parsed = PowerProfilesConfig::parse_conf(raw, base.clone());
        assert_eq!(parsed.power_saver.intel_max_perf_pct, 10);
        assert_eq!(parsed.power_saver.epp, "power");
        assert_eq!(parsed.power_saver.display_hz, "60.0");
        assert!(!parsed.balanced.boost);
        // Invalid ASPM keeps base default.
        assert_eq!(parsed.performance.pcie_aspm, base.performance.pcie_aspm);
    }

    #[test]
    fn cosmic_idle_formatting_and_file_writes() {
        assert_eq!(format_cosmic_idle_mins(0), "None");
        assert_eq!(format_cosmic_idle_mins(3), "Some(180000)");
        assert_eq!(format_cosmic_idle_mins(60), "Some(3600000)");

        let mut tuning = PowerProfilesConfig::defaults_for(false, false).power_saver;
        tuning.screen_off_mins = 3;
        tuning.lock_mins = 5;
        tuning.suspend_ac_mins = 60;
        tuning.suspend_battery_mins = 0;

        let dir = std::env::temp_dir().join(format!(
            "cosmic-taskbar-idle-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        apply_idle_timeouts_to_dir(&dir, &tuning).expect("write CosmicIdle files");

        assert_eq!(
            std::fs::read_to_string(dir.join("screen_off_time")).unwrap(),
            "Some(180000)"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("lock_time")).unwrap(),
            "Some(300000)"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("suspend_on_ac_time")).unwrap(),
            "Some(3600000)"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("suspend_on_battery_time")).unwrap(),
            "None"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
