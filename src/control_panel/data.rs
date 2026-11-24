// SPDX-License-Identifier: GPL-3.0-only

//! System, hardware, systemd, repository, optional packages, and greeter probes
//! for the native Control Panel.

use std::path::{Path, PathBuf};

use cosmic::cosmic_config::{self, CosmicConfigEntry};
use tokio::process::Command;

use crate::central::{
    services,
    tiles::{Fit, Kind, Placed, Tile},
    tray::config::{CONFIG_ID as TRAY_CONFIG_ID, TrayConfig},
};

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
pub struct OptionalStatus {
    pub iriun_installed: bool,
    pub coolercontrol_installed: bool,
    pub coolercontrol_active: bool,
    pub mission_center_installed: bool,
    pub mission_center_flatpak: bool,
    pub brave_installed: bool,
    pub mac_rotativo_installed: bool,
    pub mac_rotativo_active: bool,
    pub switcher_installed: bool,
    pub switcher_active: bool,
    pub greeter_isolated_installed: bool,
    pub greeter_service_enabled: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SecuritySnapshot {
    pub ufw_installed: bool,
    pub ufw_active: bool,
    pub resolved_active: bool,
    pub dns_over_tls: bool,
    pub dns_server: String,
    pub llmnr_disabled: bool,
    pub mdns_disabled: bool,
    pub nm_privacy_conf: bool,
    pub kernel_hardening_conf: bool,
    pub kptr_restrict: i32,
    pub dmesg_restrict: i32,
    pub ptrace_scope: i32,
    pub unpriv_bpf_disabled: i32,
    pub tcp_syncookies: bool,
    pub icmp_stealth: bool,
    pub ipv6_privacy: i32,
    pub lsm_list: String,
    // Deep kernel armor & core isolation
    pub kernel_release: String,
    pub lockdown_mode: String,
    pub kernel_armor_conf: bool,
    pub blacklist_modules_conf: bool,
    pub nocoredump_conf: bool,
    pub kexec_disabled: bool,
    pub sysrq_disabled: bool,
    pub aslr_full: bool,
    pub perf_paranoid: i32,
    pub io_uring_disabled: i32,
    pub suid_dumpable: i32,
    pub iommu_groups: usize,
    pub bwrap_installed: bool,
    pub usbguard_installed: bool,
    pub usbguard_active: bool,
    pub hosts_adblock_active: bool,
    pub networking_enabled: bool,
    pub vpn_iface: String,
    pub flatpak_installed: bool,
    pub sshd_active: bool,
    pub bluetooth_active: bool,
    pub avahi_active: bool,
    pub cups_active: bool,
    pub exposed_ports_count: usize,
    pub local_ports_count: usize,
    pub luks_active: bool,
    pub score: u8,
}

#[derive(Clone, Debug, Default)]
pub struct SystemSnapshot {
    pub hardware: HardwareInfo,
    pub units: Vec<UnitStatus>,
    pub repo: RepoStatus,
    pub optional: OptionalStatus,
    pub security: SecuritySnapshot,
    pub mac_rotativo_active: bool,
    pub economia_energia_active: bool,
    pub wallpapers_watcher_active: bool,
    pub pwa_watcher_active: bool,
    pub update_timer_active: bool,
    // Kernel / hardware tuning rules
    pub audio_modprobe_configured: bool,
    pub rapl_udev_configured: bool,
    pub i2c_dev_module_configured: bool,
    pub nct6775_module_configured: bool,
    pub logi_udev_configured: bool,
    // Backup, Snapshots & Claude sync
    pub backup_restic_installed: bool,
    pub backup_restic_timer_active: bool,
    pub backup_drive_mounted: bool,
    pub snapper_timer_active: bool,
    pub fstrim_timer_active: bool,
    pub claude_sync_installed: bool,
    pub claude_sync_active: bool,
}

/// User preferences for the isolated login & lock screen (`~/.config/cosmic-taskbar/greeter.toml`).
#[derive(Clone, Debug, PartialEq)]
pub struct GreeterConfig {
    /// Layout style: `"win11"`, `"android"`, `"macos"`, `"sidebar"`, `"editorial"`.
    pub style: String,
    pub wallpaper: String,
    /// Wallpaper mode: `"fixed"`, `"rotate_on_lock"`, `"rotate_interval"`.
    pub wallpaper_mode: String,
    /// Directory scanned for rotating wallpapers.
    pub wallpaper_dir: String,
    pub wallpaper_shuffle: bool,
    pub wallpaper_interval_mins: u32,
    pub theme: String,
    /// Font family: `"system"`, `"sans"`, `"mono"`, `"serif"`.
    pub font_family: String,
    /// Font weight: `"light"`, `"normal"`, `"semibold"`, `"bold"`.
    pub font_weight: String,
    /// Clock size: `"small"`, `"medium"`, `"large"`, `"huge"`.
    pub clock_size: String,
    pub clock_24h: bool,
    pub show_seconds: bool,
    pub show_date: bool,
    pub show_avatar: bool,
    pub show_username: bool,
    pub blur_radius: u32,
    pub dim_opacity: f32,
    /// Glass card opacity behind password block (`0.0..=0.95`).
    pub card_opacity: f32,
    /// Glass card corner radius in px (`4..=32`).
    pub card_radius: f32,
    /// Custom accent hex (`""` = system accent, or `"#RRGGBB"`).
    pub accent_color: String,
    pub default_user: String,
    pub session: String,
    pub isolated_binary: bool,
}

impl Default for GreeterConfig {
    fn default() -> Self {
        Self {
            style: "win11".to_owned(),
            wallpaper: String::new(),
            wallpaper_mode: "fixed".to_owned(),
            wallpaper_dir: String::new(),
            wallpaper_shuffle: true,
            wallpaper_interval_mins: 30,
            theme: "dark".to_owned(),
            font_family: "system".to_owned(),
            font_weight: "normal".to_owned(),
            clock_size: "medium".to_owned(),
            clock_24h: true,
            show_seconds: false,
            show_date: true,
            show_avatar: true,
            show_username: true,
            blur_radius: 0,
            dim_opacity: 0.0,
            card_opacity: 0.0,
            card_radius: 16.0,
            accent_color: String::new(),
            default_user: String::new(),
            session: "COSMIC".to_owned(),
            isolated_binary: true,
        }
    }
}

pub fn greeter_config_path() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        PathBuf::from(xdg).join("cosmic-taskbar/greeter.toml")
    } else {
        PathBuf::from(std::env::var("HOME").unwrap_or_default())
            .join(".config/cosmic-taskbar/greeter.toml")
    }
}

fn strip_toml_comment(line: &str) -> &str {
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    for (idx, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_double => escaped = true,
            '"' if !in_single => in_double = !in_double,
            '\'' if !in_double => in_single = !in_single,
            '#' if !in_single && !in_double => return &line[..idx],
            _ => {}
        }
    }
    line
}

fn parse_toml_string(raw: &str) -> String {
    let s = raw.trim();
    if let Some(inner) = s.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        let mut out = String::with_capacity(inner.len());
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some(other) => {
                        out.push('\\');
                        out.push(other);
                    }
                    None => out.push('\\'),
                }
            } else {
                out.push(c);
            }
        }
        out
    } else if let Some(inner) = s.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
        inner.to_owned()
    } else {
        s.to_owned()
    }
}

fn escape_toml_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

pub fn parse_greeter_toml(text: &str) -> GreeterConfig {
    let mut cfg = GreeterConfig::default();
    for raw_line in text.lines() {
        let line = strip_toml_comment(raw_line).trim();
        if line.is_empty() || line.starts_with('[') {
            continue;
        }
        let Some((key, val)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let val = val.trim();
        match key {
            "style" => {
                let parsed = parse_toml_string(val).to_lowercase();
                if matches!(parsed.as_str(), "win11" | "android" | "macos" | "sidebar" | "editorial") {
                    cfg.style = parsed;
                }
            }
            "wallpaper" => cfg.wallpaper = parse_toml_string(val),
            "wallpaper_mode" => {
                let parsed = parse_toml_string(val).to_lowercase();
                if matches!(parsed.as_str(), "fixed" | "rotate_on_lock" | "rotate_interval") {
                    cfg.wallpaper_mode = parsed;
                }
            }
            "wallpaper_dir" => cfg.wallpaper_dir = parse_toml_string(val),
            "wallpaper_shuffle" => {
                if let Ok(b) = val.parse::<bool>() {
                    cfg.wallpaper_shuffle = b;
                }
            }
            "wallpaper_interval_mins" => {
                if let Ok(n) = val.parse::<u32>() {
                    cfg.wallpaper_interval_mins = n.clamp(1, 1440);
                }
            }
            "theme" => {
                let parsed = parse_toml_string(val).to_lowercase();
                if matches!(parsed.as_str(), "dark" | "light") {
                    cfg.theme = parsed;
                }
            }
            "font_family" => {
                let parsed = parse_toml_string(val).to_lowercase();
                if matches!(parsed.as_str(), "system" | "sans" | "mono" | "serif") {
                    cfg.font_family = parsed;
                }
            }
            "font_weight" => {
                let parsed = parse_toml_string(val).to_lowercase();
                if matches!(parsed.as_str(), "light" | "normal" | "semibold" | "bold") {
                    cfg.font_weight = parsed;
                }
            }
            "clock_size" => {
                let parsed = parse_toml_string(val).to_lowercase();
                if matches!(parsed.as_str(), "small" | "medium" | "large" | "huge") {
                    cfg.clock_size = parsed;
                }
            }
            "clock_24h" => {
                if let Ok(b) = val.parse::<bool>() {
                    cfg.clock_24h = b;
                }
            }
            "show_seconds" => {
                if let Ok(b) = val.parse::<bool>() {
                    cfg.show_seconds = b;
                }
            }
            "show_date" => {
                if let Ok(b) = val.parse::<bool>() {
                    cfg.show_date = b;
                }
            }
            "show_avatar" => {
                if let Ok(b) = val.parse::<bool>() {
                    cfg.show_avatar = b;
                }
            }
            "show_username" => {
                if let Ok(b) = val.parse::<bool>() {
                    cfg.show_username = b;
                }
            }
            "blur_radius" => {
                if let Ok(n) = val.parse::<u32>() {
                    cfg.blur_radius = n.min(60);
                }
            }
            "dim_opacity" => {
                if let Ok(f) = val.parse::<f32>() {
                    cfg.dim_opacity = f.clamp(0.0, 0.85);
                }
            }
            "card_opacity" => {
                if let Ok(f) = val.parse::<f32>() {
                    cfg.card_opacity = f.clamp(0.0, 0.95);
                }
            }
            "card_radius" => {
                if let Ok(f) = val.parse::<f32>() {
                    cfg.card_radius = f.clamp(4.0, 32.0);
                }
            }
            "accent_color" => cfg.accent_color = parse_toml_string(val),
            "default_user" => cfg.default_user = parse_toml_string(val),
            "session" => {
                let parsed = parse_toml_string(val);
                if !parsed.is_empty() {
                    cfg.session = parsed;
                }
            }
            "isolated_binary" => {
                if let Ok(b) = val.parse::<bool>() {
                    cfg.isolated_binary = b;
                }
            }
            _ => {}
        }
    }
    cfg
}

pub fn format_greeter_toml(cfg: &GreeterConfig) -> String {
    format!(
        "# Configuração da Tela de Login e Bloqueio isolada (aenes-greeter)\n\
         # Editado pelo Painel de Controle do cosmic-taskbar\n\
         \n\
         style = {}\n\
         wallpaper = {}\n\
         wallpaper_mode = {}\n\
         wallpaper_dir = {}\n\
         wallpaper_shuffle = {}\n\
         wallpaper_interval_mins = {}\n\
         theme = {}\n\
         font_family = {}\n\
         font_weight = {}\n\
         clock_size = {}\n\
         clock_24h = {}\n\
         show_seconds = {}\n\
         show_date = {}\n\
         show_avatar = {}\n\
         show_username = {}\n\
         blur_radius = {}\n\
         dim_opacity = {:.2}\n\
         card_opacity = {:.2}\n\
         card_radius = {:.0}\n\
         accent_color = {}\n\
         default_user = {}\n\
         session = {}\n\
         isolated_binary = {}\n",
        escape_toml_string(&cfg.style),
        escape_toml_string(&cfg.wallpaper),
        escape_toml_string(&cfg.wallpaper_mode),
        escape_toml_string(&cfg.wallpaper_dir),
        cfg.wallpaper_shuffle,
        cfg.wallpaper_interval_mins.clamp(1, 1440),
        escape_toml_string(&cfg.theme),
        escape_toml_string(&cfg.font_family),
        escape_toml_string(&cfg.font_weight),
        escape_toml_string(&cfg.clock_size),
        cfg.clock_24h,
        cfg.show_seconds,
        cfg.show_date,
        cfg.show_avatar,
        cfg.show_username,
        cfg.blur_radius.min(60),
        cfg.dim_opacity.clamp(0.0, 0.85),
        cfg.card_opacity.clamp(0.0, 0.95),
        cfg.card_radius.clamp(4.0, 32.0),
        escape_toml_string(&cfg.accent_color),
        escape_toml_string(&cfg.default_user),
        escape_toml_string(&cfg.session),
        cfg.isolated_binary,
    )
}

pub fn load_greeter_config() -> GreeterConfig {
    let path = greeter_config_path();
    match std::fs::read_to_string(&path) {
        Ok(text) => parse_greeter_toml(&text),
        Err(_) => GreeterConfig::default(),
    }
}

pub fn save_greeter_config(cfg: &GreeterConfig) -> Result<PathBuf, String> {
    let path = greeter_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Falha ao criar {}: {e}", parent.display()))?;
    }
    std::fs::write(&path, format_greeter_toml(cfg))
        .map_err(|e| format!("Falha ao salvar {}: {e}", path.display()))?;
    Ok(path)
}

pub async fn apply_greeter(cfg: GreeterConfig) -> String {
    let path = match save_greeter_config(&cfg) {
        Ok(p) => p,
        Err(err) => return err,
    };
    let installed = PathBuf::from("/usr/local/bin/apply-greeter");
    let (use_bash, script) = if installed.is_file() {
        (false, installed)
    } else if let Some(repo) = find_repo_path() {
        let local = repo.join("extras/greeter/apply-greeter.sh");
        if local.is_file() {
            (true, local)
        } else {
            return format!(
                "Configuração salva em {}, mas apply-greeter.sh não foi encontrado.",
                path.display()
            );
        }
    } else {
        return format!(
            "Configuração salva em {}, mas /usr/local/bin/apply-greeter não está instalado.",
            path.display()
        );
    };
    let mut cmd = Command::new("pkexec");
    if use_bash {
        cmd.arg("/bin/bash");
    }
    cmd.arg(&script).arg("--config").arg(&path);
    if let Ok(user) = std::env::var("USER")
        && !user.is_empty()
    {
        cmd.arg("--user").arg(user);
    }
    let output = cmd.output().await;
    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            if stdout.is_empty() {
                "Preferências aplicadas na tela de login.".to_owned()
            } else {
                stdout
                    .lines()
                    .last()
                    .unwrap_or("Preferências aplicadas na tela de login.")
                    .to_owned()
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            let detail = if !stderr.is_empty() { stderr } else { stdout };
            if detail.is_empty() {
                format!(
                    "apply-greeter retornou código {} (autenticação cancelada ou erro).",
                    out.status.code().unwrap_or(-1)
                )
            } else {
                detail.lines().last().unwrap_or(&detail).to_owned()
            }
        }
        Err(err) => format!("Falha ao executar pkexec: {err}"),
    }
}

pub fn reset_central_tiles() {
    let at = |kind, fit| Placed { kind, fit };
    let defaults = [
        at(Kind::Brightness, Fit::Vertical),
        at(Kind::Tile(Tile::Wifi), Fit::Pill),
        at(Kind::Tile(Tile::DoNotDisturb), Fit::Icon),
        at(Kind::Tile(Tile::Bluetooth), Fit::Short),
        at(Kind::Tile(Tile::Power), Fit::Short),
        at(Kind::Tile(Tile::DarkMode), Fit::Icon),
        at(Kind::Tile(Tile::Airplane), Fit::Icon),
        at(Kind::Tile(Tile::KeepAwake), Fit::Icon),
        at(Kind::Tile(Tile::Screenshot), Fit::Icon),
        at(Kind::Tile(Tile::Microphone), Fit::Icon),
        at(Kind::Tile(Tile::Workspaces), Fit::Icon),
        at(Kind::Tile(Tile::Lock), Fit::Icon),
    ];
    let ids: Vec<String> = defaults.into_iter().map(Placed::id).collect();
    services::set_tiles(&ids);
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

fn command_exists(program: &str) -> bool {
    let home = std::env::var("HOME").unwrap_or_default();
    let path_env = std::env::var("PATH").unwrap_or_default();
    path_env
        .split(':')
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .chain([
            PathBuf::from(format!("{home}/.local/bin")),
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/usr/bin"),
        ])
        .any(|dir| dir.join(program).is_file())
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
    let has_nvidia = Path::new("/usr/bin/nvidia-smi").exists();
    let has_coolercontrol = Path::new("/etc/coolercontrol/config.toml").exists();
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

async fn probe_optional(mac_rotativo_active: bool, switcher_active: bool) -> OptionalStatus {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let iriun_installed =
        services::iriun_installed() || Path::new("/usr/bin/iriunwebcam").exists();
    let coolercontrol_installed = command_exists("coolercontrol")
        || command_exists("coolercontrold")
        || Path::new("/etc/coolercontrol/config.toml").exists();
    let coolercontrol_active = is_unit_active("coolercontrold.service", true).await;
    let mission_center_flatpak =
        Path::new("/var/lib/flatpak/app/io.missioncenter.MissionCenter").exists()
            || home
                .join(".local/share/flatpak/app/io.missioncenter.MissionCenter")
                .exists();
    let mission_center_installed = command_exists("missioncenter") || mission_center_flatpak;
    let brave_installed = command_exists("brave")
        || command_exists("brave-browser")
        || Path::new("/opt/brave-bin/brave").exists();
    let mac_rotativo_installed = Path::new("/etc/systemd/system/mac-rotativo.timer").exists()
        || Path::new("/usr/local/bin/mac-rotativo").exists();
    let switcher_installed = home
        .join(".config/systemd/user/cosmic-switcher.service")
        .exists()
        || home
            .join(".local/share/systemd/user/cosmic-switcher.service")
            .exists()
        || home
            .join(".local/share/dbus-1/services/io.github.Aenes0.CosmicSwitcher.service")
            .exists()
        || Path::new("/usr/share/systemd/user/cosmic-switcher.service").exists();
    let greeter_isolated_installed = Path::new("/usr/local/bin/aenes-greeter").exists()
        || Path::new("/etc/systemd/system/aenes-greeter.service").exists();
    let greeter_service_enabled = Command::new("systemctl")
        .args(["is-enabled", "--quiet", "aenes-greeter.service"])
        .status()
        .await
        .is_ok_and(|s| s.success());

    OptionalStatus {
        iriun_installed,
        coolercontrol_installed,
        coolercontrol_active,
        mission_center_installed,
        mission_center_flatpak,
        brave_installed,
        mac_rotativo_installed,
        mac_rotativo_active,
        switcher_installed,
        switcher_active,
        greeter_isolated_installed,
        greeter_service_enabled,
    }
}

fn read_sysctl_i32(proc_subpath: &str) -> i32 {
    std::fs::read_to_string(format!("/proc/sys/{proc_subpath}"))
        .ok()
        .and_then(|s| s.trim().parse::<i32>().ok())
        .unwrap_or(-1)
}

async fn probe_security(mac_rotativo_active: bool, greeter_isolated: bool) -> SecuritySnapshot {
    let ufw_installed = command_exists("ufw");
    let ufw_active = is_unit_active("ufw.service", true).await;
    let resolved_active = is_unit_active("systemd-resolved.service", true).await;
    let sshd_active = is_unit_active("sshd.service", true).await;
    let bluetooth_active = is_unit_active("bluetooth.service", true).await;
    let avahi_active = is_unit_active("avahi-daemon.service", true).await;
    let cups_active = is_unit_active("cups.service", true).await
        || is_unit_active("cups-browsed.service", true).await;

    let mut dns_over_tls = false;
    let mut llmnr_disabled = false;
    let mut mdns_disabled = false;
    let mut dns_server = String::new();
    if let Ok(out) = Command::new("resolvectl").arg("status").output().await {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines().take(18) {
                let trimmed = line.trim();
                if trimmed.starts_with("Protocols:") {
                    if trimmed.contains("+DNSOverTLS") {
                        dns_over_tls = true;
                    }
                    if trimmed.contains("-LLMNR") {
                        llmnr_disabled = true;
                    }
                    if trimmed.contains("-mDNS") {
                        mdns_disabled = true;
                    }
                } else if dns_server.is_empty() && trimmed.starts_with("Current DNS Server:") {
                    dns_server = trimmed
                        .trim_start_matches("Current DNS Server:")
                        .trim()
                        .to_owned();
                } else if dns_server.is_empty() && trimmed.starts_with("DNS Servers:") {
                    dns_server = trimmed
                        .trim_start_matches("DNS Servers:")
                        .trim()
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .to_owned();
                }
            }
        }
    }

    let mut exposed_ports_count = 0usize;
    let mut local_ports_count = 0usize;
    if let Ok(out) = Command::new("ss").args(["-tulnH"]).output().await {
        if out.status.success() {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                let cols: Vec<&str> = line.split_whitespace().collect();
                if cols.len() < 5 {
                    continue;
                }
                let state = cols[1];
                if state != "LISTEN" {
                    continue;
                }
                let local = cols[4];
                if local.starts_with("127.") || local.starts_with("[::1]") {
                    local_ports_count += 1;
                } else {
                    exposed_ports_count += 1;
                }
            }
        }
    }

    let kptr_restrict = read_sysctl_i32("kernel/kptr_restrict");
    let dmesg_restrict = read_sysctl_i32("kernel/dmesg_restrict");
    let ptrace_scope = read_sysctl_i32("kernel/yama/ptrace_scope");
    let unpriv_bpf_disabled = read_sysctl_i32("kernel/unprivileged_bpf_disabled");
    let tcp_syncookies = read_sysctl_i32("net/ipv4/tcp_syncookies") == 1;
    let icmp_stealth = read_sysctl_i32("net/ipv4/icmp_echo_ignore_all") == 1;
    let ipv6_privacy = read_sysctl_i32("net/ipv6/conf/all/use_tempaddr");
    let lsm_list = std::fs::read_to_string("/sys/kernel/security/lsm")
        .unwrap_or_default()
        .trim()
        .to_owned();
    let kernel_release = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .unwrap_or_default()
        .trim()
        .to_owned();
    let lockdown_raw = std::fs::read_to_string("/sys/kernel/security/lockdown").unwrap_or_default();
    let lockdown_mode = lockdown_raw
        .split_whitespace()
        .find(|tok| tok.starts_with('[') && tok.ends_with(']'))
        .map(|tok| tok.trim_matches(|c| c == '[' || c == ']').to_owned())
        .unwrap_or_else(|| "indisponível".to_owned());
    let kernel_armor_conf = Path::new("/etc/sysctl.d/99-aenes-kernel-armor.conf").exists();
    let blacklist_modules_conf = Path::new("/etc/modprobe.d/99-aenes-blacklist.conf").exists();
    let nocoredump_conf = Path::new("/etc/systemd/coredump.conf.d/99-aenes-nocoredump.conf").exists();
    let kexec_disabled = read_sysctl_i32("kernel/kexec_load_disabled") == 1;
    let sysrq_disabled = read_sysctl_i32("kernel/sysrq") == 0;
    let aslr_full = read_sysctl_i32("kernel/randomize_va_space") >= 2;
    let perf_paranoid = read_sysctl_i32("kernel/perf_event_paranoid");
    let io_uring_disabled = read_sysctl_i32("kernel/io_uring_disabled");
    let suid_dumpable = read_sysctl_i32("fs/suid_dumpable");
    let iommu_groups = std::fs::read_dir("/sys/kernel/iommu_groups")
        .ok()
        .map(|d| d.count())
        .unwrap_or(0);
    let bwrap_installed = command_exists("bwrap");
    let usbguard_installed = command_exists("usbguard");
    let usbguard_active = is_unit_active("usbguard.service", true).await;
    let hosts_adblock_active = std::fs::read_to_string("/etc/hosts")
        .is_ok_and(|s| s.contains("# aenes-hosts-adblock START"));
    let networking_enabled = Command::new("nmcli")
        .args(["networking"])
        .output()
        .await
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().eq_ignore_ascii_case("enabled"))
        .unwrap_or(true);
    let vpn_iface = std::fs::read_dir("/sys/class/net")
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .find(|n| {
            n.starts_with("wg")
                || n.starts_with("tun")
                || n.starts_with("tailscale")
                || n.starts_with("proton")
                || n.starts_with("mullvad")
        })
        .unwrap_or_default();
    let flatpak_installed = command_exists("flatpak");
    let nm_privacy_conf = Path::new("/etc/NetworkManager/conf.d/99-aenes-privacy.conf").exists();
    let kernel_hardening_conf = Path::new("/etc/sysctl.d/99-aenes-privacy.conf").exists();
    let luks_active = std::fs::read_dir("/dev/mapper")
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| {
            let name = e.file_name();
            let s = name.to_string_lossy();
            s != "control" && !s.is_empty()
        });

    let mut score: u8 = 0;
    if ufw_active {
        score += 20;
    }
    if dns_over_tls {
        score += 15;
    }
    if llmnr_disabled && mdns_disabled {
        score += 5;
    }
    if mac_rotativo_active || nm_privacy_conf {
        score += 10;
    }
    if kptr_restrict >= 2 && dmesg_restrict >= 1 {
        score += 10;
    }
    if ptrace_scope >= 1 && unpriv_bpf_disabled >= 1 {
        score += 10;
    }
    if tcp_syncookies && (ipv6_privacy >= 2 || icmp_stealth) {
        score += 10;
    }
    if !sshd_active && !avahi_active && !cups_active && exposed_ports_count == 0 {
        score += 10;
    }
    if greeter_isolated {
        score += 10;
    }

    SecuritySnapshot {
        ufw_installed,
        ufw_active,
        resolved_active,
        dns_over_tls,
        dns_server,
        llmnr_disabled,
        mdns_disabled,
        nm_privacy_conf,
        kernel_hardening_conf,
        kptr_restrict,
        dmesg_restrict,
        ptrace_scope,
        unpriv_bpf_disabled,
        tcp_syncookies,
        icmp_stealth,
        ipv6_privacy,
        lsm_list,
        kernel_release,
        lockdown_mode,
        kernel_armor_conf,
        blacklist_modules_conf,
        nocoredump_conf,
        kexec_disabled,
        sysrq_disabled,
        aslr_full,
        perf_paranoid,
        io_uring_disabled,
        suid_dumpable,
        iommu_groups,
        bwrap_installed,
        usbguard_installed,
        usbguard_active,
        hosts_adblock_active,
        networking_enabled,
        vpn_iface,
        flatpak_installed,
        sshd_active,
        bluetooth_active,
        avahi_active,
        cups_active,
        exposed_ports_count,
        local_ports_count,
        luks_active,
        score,
    }
}

pub async fn load_snapshot() -> SystemSnapshot {
    let hardware = read_hardware();
    let repo = read_repo_status().await;

    let tracked: &[(&'static str, &'static str, bool)] = &[
        ("central-panels.service", "Host de painéis Vulkan (Central / Monitor)", false),
        ("central-clipboard.service", "Histórico da área de transferência", false),
        ("central-notifications.service", "Histórico de notificações", false),
        ("cosmic-switcher.service", "Alternador Alt+Tab / Win+Tab isolado", false),
        ("logi-osd.service", "OSD de Dispositivos (entrada/saída e periféricos)", false),
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
    let switcher_active = units
        .iter()
        .find(|u| u.unit == "cosmic-switcher.service")
        .is_some_and(|u| u.active);
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
    let optional = probe_optional(mac_rotativo_active, switcher_active).await;
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let greeter_user_installed = home.join(".local/bin/aenes-greeter").exists();
    let security = probe_security(
        mac_rotativo_active,
        optional.greeter_service_enabled || greeter_user_installed,
    )
    .await;

    let audio_modprobe_configured = Path::new("/etc/modprobe.d/99-economia-energia.conf").exists();
    let rapl_udev_configured = Path::new("/etc/udev/rules.d/99-rapl.rules").exists()
        || Path::new("/etc/udev/rules.d/99-powercap.rules").exists();
    let i2c_dev_module_configured = Path::new("/etc/modules-load.d/i2c-dev.conf").exists();
    let nct6775_module_configured = Path::new("/etc/modules-load.d/nct6775.conf").exists();
    let logi_udev_configured = Path::new("/etc/udev/rules.d/70-logi-osd.rules").exists()
        || Path::new("/etc/udev/rules.d/70-cosmic-cosmetics-logi.rules").exists()
        || Path::new("/usr/lib/udev/rules.d/42-logitech-unify-permissions.rules").exists();

    let backup_restic_installed = home.join(".local/bin/backup-restic").exists();
    let backup_restic_timer_active = is_unit_active("backup-restic.timer", false).await;
    let backup_drive_mounted = Path::new("/mnt/devdrive/backup-restic").exists()
        || Path::new("/mnt/devdrive").read_dir().is_ok_and(|mut d| d.next().is_some());
    let snapper_timer_active = is_unit_active("snapper-timeline.timer", true).await;
    let fstrim_timer_active = is_unit_active("fstrim.timer", true).await;
    let claude_sync_installed = home.join("claude-migracao/sincronizar_contas.py").exists();
    let claude_sync_active = is_unit_active("claude-sync-contas.timer", false).await
        || is_unit_active("claude-sync-contas.path", false).await;

    SystemSnapshot {
        hardware,
        units,
        repo,
        optional,
        security,
        mac_rotativo_active,
        economia_energia_active,
        wallpapers_watcher_active,
        pwa_watcher_active,
        update_timer_active,
        audio_modprobe_configured,
        rapl_udev_configured,
        i2c_dev_module_configured,
        nct6775_module_configured,
        logi_udev_configured,
        backup_restic_installed,
        backup_restic_timer_active,
        backup_drive_mounted,
        snapper_timer_active,
        fstrim_timer_active,
        claude_sync_installed,
        claude_sync_active,
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

pub async fn enable_user_unit(unit: &'static str) -> String {
    let status = Command::new("systemctl")
        .args(["--user", "enable", "--now", unit])
        .status()
        .await;
    match status {
        Ok(s) if s.success() => format!("Serviço {unit} habilitado e iniciado."),
        Ok(s) => format!("Falha ao habilitar {unit} (código {}).", s.code().unwrap_or(-1)),
        Err(err) => format!("Erro ao habilitar {unit}: {err}"),
    }
}

pub async fn run_shell_action(label: &str, cmd: &str, privileged: bool) -> String {
    let output = if privileged {
        Command::new("pkexec").args(["sh", "-c", cmd]).output().await
    } else {
        Command::new("sh").args(["-c", cmd]).output().await
    };
    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let detail = stdout
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .map(str::trim)
                .unwrap_or("concluído");
            format!("✓ {label}: {detail}")
        }
        Ok(out) => {
            let code = out.status.code().unwrap_or(-1);
            if privileged && (code == 126 || code == 127) {
                return format!("Cancelado ({label}): autenticação não concluída.");
            }
            let stderr = String::from_utf8_lossy(&out.stderr);
            let stdout = String::from_utf8_lossy(&out.stdout);
            let detail = stderr
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .or_else(|| stdout.lines().rev().find(|l| !l.trim().is_empty()))
                .map(str::trim)
                .unwrap_or("falha ao executar");
            format!("✗ {label}: {detail} (código {code})")
        }
        Err(err) => format!("✗ {label}: erro ao executar ({err})"),
    }
}

pub fn open_in_terminal(title: &str, shell_cmd: &str) -> String {
    let wrapped = format!(
        "{shell_cmd}; echo; printf 'Pressione Enter para fechar... '; read -r _"
    );
    let candidates: &[(&str, Vec<String>)] = &[
        ("cosmic-term", vec!["-e".into(), "sh".into(), "-c".into(), wrapped.clone()]),
        ("kitty", vec!["-T".into(), title.into(), "sh".into(), "-c".into(), wrapped.clone()]),
        (
            "alacritty",
            vec!["-T".into(), title.into(), "-e".into(), "sh".into(), "-c".into(), wrapped.clone()],
        ),
        ("foot", vec!["-T".into(), title.into(), "sh".into(), "-c".into(), wrapped.clone()]),
        ("xterm", vec!["-T".into(), title.into(), "-e".into(), "sh".into(), "-c".into(), wrapped]),
    ];
    for (term, args) in candidates {
        if command_exists(term) {
            return match std::process::Command::new(term).args(args).spawn() {
                Ok(_) => format!("Terminal aberto ({title})."),
                Err(err) => format!("Falha ao abrir {term}: {err}"),
            };
        }
    }
    "Nenhum terminal encontrado (cosmic-term, kitty, alacritty, foot, xterm).".to_owned()
}

pub fn spawn_app(program: &str, args: &[&str], label: &str) -> String {
    match std::process::Command::new(program).args(args).spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || {
                _ = child.wait();
            });
            format!("{label} iniciado.")
        }
        Err(err) => format!("Não foi possível abrir {label}: {err}"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greeter_toml_round_trip() {
        let cfg = GreeterConfig {
            style: "android".to_owned(),
            wallpaper: "/home/user/Imagens/wall #1.jpg".to_owned(),
            wallpaper_mode: "rotate_interval".to_owned(),
            wallpaper_dir: "/home/user/Imagens/Wallpapers".to_owned(),
            wallpaper_shuffle: false,
            wallpaper_interval_mins: 45,
            theme: "light".to_owned(),
            font_family: "mono".to_owned(),
            font_weight: "bold".to_owned(),
            clock_size: "huge".to_owned(),
            clock_24h: false,
            show_seconds: true,
            show_date: false,
            show_avatar: false,
            show_username: true,
            blur_radius: 18,
            dim_opacity: 0.45,
            card_opacity: 0.72,
            card_radius: 24.0,
            accent_color: "#60A5FA".to_owned(),
            default_user: "alexandre".to_owned(),
            session: "COSMIC".to_owned(),
            isolated_binary: true,
        };
        let toml = format_greeter_toml(&cfg);
        let parsed = parse_greeter_toml(&toml);
        assert_eq!(parsed, cfg);
    }

    #[test]
    fn greeter_toml_tolerates_sections_and_comments() {
        let raw = r#"
# Comentário inicial
[greeter]
wallpaper = '/usr/share/backgrounds/bg.png' # inline comment
theme = "dark"
clock_24h = true
show_seconds = false
blur_radius = 40
dim_opacity = 0.25
default_user = "maria"
session = "COSMIC"
isolated_binary = false
"#;
        let parsed = parse_greeter_toml(raw);
        assert_eq!(parsed.wallpaper, "/usr/share/backgrounds/bg.png");
        assert_eq!(parsed.theme, "dark");
        assert!(parsed.clock_24h);
        assert!(!parsed.show_seconds);
        assert_eq!(parsed.blur_radius, 40);
        assert!((parsed.dim_opacity - 0.25).abs() < 0.001);
        assert_eq!(parsed.default_user, "maria");
        assert_eq!(parsed.session, "COSMIC");
        assert!(!parsed.isolated_binary);
    }
}
