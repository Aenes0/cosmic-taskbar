// SPDX-License-Identifier: GPL-3.0-only

//! Saúde de unidades systemd e atraso do backup (somente leitura).

use std::time::{Duration, SystemTime};

use tokio::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnitHealth {
    pub loaded: bool,
    pub active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimerTimes {
    pub last: Option<SystemTime>,
    pub next: Option<SystemTime>,
}

/// Atraso a partir do qual o backup é considerado atrasado.
pub const BACKUP_LATE: Duration = Duration::from_secs(26 * 3600);

fn value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .find_map(|l| l.trim().strip_prefix(key)?.strip_prefix('='))
        .map(str::trim)
}

/// Interpreta a saída de `systemctl show -p LoadState,ActiveState`.
pub fn parse_show(text: &str) -> UnitHealth {
    UnitHealth {
        loaded: value(text, "LoadState") == Some("loaded"),
        active: value(text, "ActiveState") == Some("active"),
    }
}

/// Unidade existe mas não está ativa.
pub fn is_stopped(h: &UnitHealth) -> bool {
    h.loaded && !h.active
}

fn parse_unix(v: Option<&str>) -> Option<SystemTime> {
    let secs: u64 = v?.strip_prefix('@')?.parse().ok()?;
    if secs == 0 {
        return None;
    }
    Some(SystemTime::UNIX_EPOCH + Duration::from_secs(secs))
}

/// Interpreta `systemctl show -p LastTriggerUSec,NextElapseUSecRealtime --timestamp=unix`
/// (valores no formato `@<segundos>`; `n/a`, vazio ou 0 viram `None`).
pub fn parse_timer(text: &str) -> TimerTimes {
    TimerTimes {
        last: parse_unix(value(text, "LastTriggerUSec")),
        next: parse_unix(value(text, "NextElapseUSecRealtime")),
    }
}

/// Backup atrasado: o último disparo foi há mais de `BACKUP_LATE`.
/// Nunca ter rodado (`last == None`) não é atraso.
pub fn backup_late(t: &TimerTimes, now: SystemTime) -> bool {
    t.last
        .and_then(|last| now.duration_since(last).ok())
        .is_some_and(|d| d > BACKUP_LATE)
}

async fn systemctl_show(unit: &str, system: bool, props: &str, extra: &[&str]) -> Option<String> {
    let mut cmd = Command::new("systemctl");
    if !system {
        cmd.arg("--user");
    }
    let out = cmd
        .args(["show", unit, "-p", props])
        .args(extra)
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Estado de uma unidade (somente leitura). Falha ao rodar `systemctl` → não carregada.
pub async fn unit_health(unit: &str, system: bool) -> UnitHealth {
    match systemctl_show(unit, system, "LoadState,ActiveState", &[]).await {
        Some(text) => parse_show(&text),
        None => UnitHealth::default(),
    }
}

/// Horários do timer de usuário `backup-restic.timer`. Falha → vazio.
pub async fn backup_timer() -> TimerTimes {
    match systemctl_show(
        "backup-restic.timer",
        false,
        "LastTriggerUSec,NextElapseUSecRealtime",
        &["--timestamp=unix"],
    )
    .await
    {
        Some(text) => parse_timer(&text),
        None => TimerTimes::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_unit_states() {
        assert!(is_stopped(&parse_show("LoadState=loaded\nActiveState=inactive\n")));
        assert!(!is_stopped(&parse_show("LoadState=not-found\nActiveState=inactive\n")));
        assert!(!is_stopped(&parse_show("LoadState=loaded\nActiveState=active\n")));
        assert!(!is_stopped(&parse_show("")));
    }

    #[test]
    fn backup_lateness() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(2_000_000);
        let t = parse_timer("LastTriggerUSec=@1900000\nNextElapseUSecRealtime=@2040000\n");
        assert!(backup_late(&t, now)); // 100 000 s > 26 h
        assert!(!backup_late(&TimerTimes { last: None, next: None }, now));
        assert!(!backup_late(&parse_timer("LastTriggerUSec=n/a\n"), now));
    }
}
