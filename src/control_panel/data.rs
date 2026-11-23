// SPDX-License-Identifier: GPL-3.0-only

//! System, hardware, systemd, and repository probes for the Control Panel.

use std::path::PathBuf;

use cosmic::cosmic_config::{self, CosmicConfigEntry};
use tokio::process::Command;

use crate::central::tray::config::{CONFIG_ID as TRAY_CONFIG_ID, TrayConfig};

#[derive(Clone, Debug, Default)]
pub struct HardwareInfo {
    pub cpu_model: String,
    pub is_intel: bool,
    pub is_laptop: bool,
    pub has_nvidia: bool,
    pub has_coolercontrol: bool,
    pub host_name: String,
}

#[derive(Clone, Debug)]
pub struct UnitStatus {
    pub unit: &'static str,
    pub label: &'static str,
    pub active: bool,
    pub is_system: bool,
}

#[derive(Clone, Debug, Default)]
pub struct RepoStatus {
    pub path: Option<PathBuf>,
    pub commit_short: String,
    pub commit_subject: String,
    pub dirty: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SystemSnapshot {
    pub hardware: HardwareInfo,
    pub units: Vec<UnitStatus>,
    pub repo: RepoStatus,
    pub mac_rotativo_active: bool,
    pub economia_energia_active: bool,
    pub wallpapers_watcher_active: bool,
    pub pwa_watcher_active: bool,
    pub update_timer_active: bool,
}

pub fn load_tray_config() -> TrayConfig {
    cosmic_config::Config::new(TRAY_CONFIG_ID, TrayConfig::VERSION)
        .ok()
        .map(|handle| TrayConfig::get_entry(&handle).unwrap_or_else(|(_, cfg)| cfg))
        .unwrap_or_default()
}

pub fn save_tray_hidden(hidden: Vec<String>) {
    if let Ok(handle) = cosmic_config::Config::new(TRAY_CONFIG_ID, TrayConfig::VERSION) {
        let mut cfg = TrayConfig::get_entry(&handle).unwrap_or_else(|(_, cfg)| cfg);
        if let Err(err) = cfg.set_hidden(&handle, hidden) {
            tracing::warn!("failed to save tray hidden list: {err}");
        }
    }
}

pub fn read_hardware() -> HardwareInfo {
    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let cpu_model = cpuinfo
        .lines()
        .find_map(|line| line.strip_prefix("model name"))
        .and_then(|rest| rest.split_once(':'))
        .map(|(_, val)| val.trim().to_owned())
        .unwrap_or_else(|| "CPU desconhecida".to_owned());
    let is_intel = cpuinfo
        .lines()
        .find_map(|line| line.strip_prefix("vendor_id"))
        .is_some_and(|rest| rest.contains("GenuineIntel"));
    let is_laptop = std::fs::read_dir("/sys/class/power_supply")
        .ok()
        .is_some_and(|entries| {
            entries.flatten().any(|e| {
                e.file_name()
                    .to_str()
                    .is_some_and(|n| n.starts_with("BAT"))
            })
        });
    let has_nvidia = std::path::Path::new("/usr/bin/nvidia-smi").exists();
    let has_coolercontrol = std::path::Path::new("/etc/coolercontrol/config.toml").exists();
    let host_name = std::fs::read_to_string("/etc/hostname")
        .unwrap_or_default()
        .trim()
        .to_owned();
    HardwareInfo {
        cpu_model,
        is_intel,
        is_laptop,
        has_nvidia,
        has_coolercontrol,
        host_name,
    }
}

pub fn find_repo_path() -> Option<PathBuf> {
    if let Ok(env_path) = std::env::var("COSMIC_TASKBAR_REPO") {
        let p = PathBuf::from(env_path);
        if p.join(".git").is_dir() {
            return Some(p);
        }
    }
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let cfg_file = home.join(".config/cosmic-taskbar/repo_path");
    if let Ok(saved) = std::fs::read_to_string(cfg_file) {
        let p = PathBuf::from(saved.trim());
        if p.join(".git").is_dir() {
            return Some(p);
        }
    }
    for rel in ["Projetos/cosmic-taskbar", "cosmic-taskbar", "Repos/cosmic-taskbar"] {
        let p = home.join(rel);
        if p.join(".git").is_dir() {
            return Some(p);
        }
    }
    None
}

async fn is_unit_active(unit: &str, system: bool) -> bool {
    let mut cmd = Command::new("systemctl");
    if !system {
        cmd.arg("--user");
    }
    cmd.args(["is-active", "--quiet", unit])
        .status()
        .await
        .is_ok_and(|s| s.success())
}

async fn read_repo_status() -> RepoStatus {
    let Some(path) = find_repo_path() else {
        return RepoStatus::default();
    };
    let short = Command::new("git")
        .args(["-C", &path.to_string_lossy(), "rev-parse", "--short", "HEAD"])
        .output()
        .await
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default();
    let subject = Command::new("git")
        .args(["-C", &path.to_string_lossy(), "log", "-1", "--pretty=format:%s"])
        .output()
        .await
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default();
    let dirty = Command::new("git")
        .args(["-C", &path.to_string_lossy(), "status", "--porcelain"])
        .output()
        .await
        .ok()
        .is_some_and(|o| !o.stdout.is_empty());
    RepoStatus {
        path: Some(path),
        commit_short: short,
        commit_subject: subject,
        dirty,
    }
}

pub async fn load_snapshot() -> SystemSnapshot {
    let hardware = read_hardware();
    let repo = read_repo_status().await;

    let tracked: &[(&'static str, &'static str, bool)] = &[
        ("central-panels.service", "Host de painéis Vulkan (Central / Monitor)", false),
        ("central-clipboard.service", "Histórico da área de transferência", false),
        ("central-notifications.service", "Histórico de notificações", false),
        ("logi-osd.service", "OSD de dispositivos Logitech", false),
        ("cosmic-taskbar-update.timer", "Atualizador automático por boot", false),
        ("economia-energia.service", "Daemon de economia de energia", true),
        ("power-profiles-daemon.service", "Power Profiles Daemon", true),
    ];

    let mut units = Vec::with_capacity(tracked.len());
    for &(unit, label, is_system) in tracked {
        let active = is_unit_active(unit, is_system).await;
        units.push(UnitStatus {
            unit,
            label,
            active,
            is_system,
        });
    }

    let mac_rotativo_active = is_unit_active("mac-rotativo.timer", true).await;
    let economia_energia_active = units
        .iter()
        .find(|u| u.unit == "economia-energia.service")
        .is_some_and(|u| u.active);
    let wallpapers_watcher_active = is_unit_active("wallpapers-monitores.path", false).await;
    let pwa_watcher_active = is_unit_active("fix-brave-pwa-icons.path", false).await;
    let update_timer_active = units
        .iter()
        .find(|u| u.unit == "cosmic-taskbar-update.timer")
        .is_some_and(|u| u.active);

    SystemSnapshot {
        hardware,
        units,
        repo,
        mac_rotativo_active,
        economia_energia_active,
        wallpapers_watcher_active,
        pwa_watcher_active,
        update_timer_active,
    }
}

pub async fn restart_user_unit(unit: &'static str) -> String {
    let status = Command::new("systemctl")
        .args(["--user", "restart", unit])
        .status()
        .await;
    match status {
        Ok(s) if s.success() => format!("Serviço {unit} reiniciado."),
        Ok(s) => format!("Falha ao reiniciar {unit} (código {}).", s.code().unwrap_or(-1)),
        Err(err) => format!("Erro ao reiniciar {unit}: {err}"),
    }
}

pub async fn run_user_tool(program: &str, args: &[&str], ok_msg: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let local_bin = PathBuf::from(&home).join(".local/bin").join(program);
    let bin = if local_bin.is_file() {
        local_bin.to_string_lossy().into_owned()
    } else {
        program.to_owned()
    };
    let output = Command::new(&bin).args(args).output().await;
    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            if stdout.is_empty() {
                ok_msg.to_owned()
            } else {
                stdout.lines().last().unwrap_or(ok_msg).to_owned()
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            let detail = if !stderr.is_empty() { stderr } else { stdout };
            if detail.is_empty() {
                format!("{program} retornou código {}.", out.status.code().unwrap_or(-1))
            } else {
                detail.lines().last().unwrap_or(&detail).to_owned()
            }
        }
        Err(err) => format!("Não foi possível executar {program}: {err}"),
    }
}

pub async fn export_repo_config() -> String {
    let Some(repo) = find_repo_path() else {
        return "Repositório cosmic-taskbar não encontrado.".to_owned();
    };
    let script = repo.join("scripts/sync-config.sh");
    let output = Command::new(&script).arg("export").output().await;
    match output {
        Ok(out) if out.status.success() => {
            "Perfil visual exportado para config/cosmic e config/fish.".to_owned()
        }
        Ok(out) => format!(
            "Falha ao exportar: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ),
        Err(err) => format!("Erro ao executar sync-config.sh: {err}"),
    }
}
