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
pub struct SystemSnapshot {
    pub hardware: HardwareInfo,
    pub units: Vec<UnitStatus>,
    pub repo: RepoStatus,
    pub optional: OptionalStatus,
    pub mac_rotativo_active: bool,
    pub economia_energia_active: bool,
    pub wallpapers_watcher_active: bool,
    pub pwa_watcher_active: bool,
    pub update_timer_active: bool,
}

/// User preferences for the isolated login screen (`~/.config/cosmic-taskbar/greeter.toml`).
#[derive(Clone, Debug, PartialEq)]
pub struct GreeterConfig {
    pub wallpaper: String,
    pub theme: String,
    pub clock_24h: bool,
    pub show_seconds: bool,
    pub blur_radius: u32,
    pub dim_opacity: f32,
    pub default_user: String,
    pub session: String,
    pub isolated_binary: bool,
}

impl Default for GreeterConfig {
    fn default() -> Self {
        Self {
            wallpaper: String::new(),
            theme: "dark".to_owned(),
            clock_24h: true,
            show_seconds: false,
            blur_radius: 0,
            dim_opacity: 0.0,
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
            "wallpaper" => cfg.wallpaper = parse_toml_string(val),
            "theme" => {
                let parsed = parse_toml_string(val).to_lowercase();
                if matches!(parsed.as_str(), "dark" | "light") {
                    cfg.theme = parsed;
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
        "# Configuração da Tela de Login isolada (aenes-greeter)\n\
         # Editado pelo Painel de Controle do cosmic-taskbar\n\
         \n\
         wallpaper = {}\n\
         theme = {}\n\
         clock_24h = {}\n\
         show_seconds = {}\n\
         blur_radius = {}\n\
         dim_opacity = {:.2}\n\
         default_user = {}\n\
         session = {}\n\
         isolated_binary = {}\n",
        escape_toml_string(&cfg.wallpaper),
        escape_toml_string(&cfg.theme),
        cfg.clock_24h,
        cfg.show_seconds,
        cfg.blur_radius.min(60),
        cfg.dim_opacity.clamp(0.0, 0.85),
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

pub async fn load_snapshot() -> SystemSnapshot {
    let hardware = read_hardware();
    let repo = read_repo_status().await;

    let tracked: &[(&'static str, &'static str, bool)] = &[
        ("central-panels.service", "Host de painéis Vulkan (Central / Monitor)", false),
        ("central-clipboard.service", "Histórico da área de transferência", false),
        ("central-notifications.service", "Histórico de notificações", false),
        ("cosmic-switcher.service", "Alternador Alt+Tab / Win+Tab isolado", false),
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

    SystemSnapshot {
        hardware,
        units,
        repo,
        optional,
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
        Ok(_) => format!("{label} iniciado."),
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
            wallpaper: "/home/user/Imagens/wall #1.jpg".to_owned(),
            theme: "light".to_owned(),
            clock_24h: false,
            show_seconds: true,
            blur_radius: 18,
            dim_opacity: 0.45,
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
