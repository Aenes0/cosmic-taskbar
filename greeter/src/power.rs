// SPDX-License-Identifier: GPL-3.0-only

use std::ffi::OsStr;

use crate::fl;

/// Quando `1`, nenhuma ação de energia chama o logind (só registra no log).
pub const NESTED_ENV: &str = "COSMIC_GREETER_NESTED";

pub fn nested_from_value(value: Option<&OsStr>) -> bool {
    value == Some(OsStr::new("1"))
}

pub fn nested_from_env() -> bool {
    nested_from_value(std::env::var_os(NESTED_ENV).as_deref())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerAction {
    Suspend,
    Restart,
    Shutdown,
}

impl PowerAction {
    pub const ALL: [PowerAction; 3] = [
        PowerAction::Suspend,
        PowerAction::Restart,
        PowerAction::Shutdown,
    ];

    pub fn icon_name(self) -> &'static str {
        match self {
            PowerAction::Suspend => "system-suspend-symbolic",
            PowerAction::Restart => "system-reboot-symbolic",
            PowerAction::Shutdown => "system-shutdown-symbolic",
        }
    }

    pub fn label(self) -> String {
        match self {
            PowerAction::Suspend => fl!("suspend"),
            PowerAction::Restart => fl!("restart"),
            PowerAction::Shutdown => fl!("shutdown"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PowerCall {
    DryRun(PowerAction),
    Logind(PowerAction),
}

pub fn plan(action: PowerAction, nested: bool) -> PowerCall {
    if nested {
        PowerCall::DryRun(action)
    } else {
        PowerCall::Logind(action)
    }
}

pub fn menu_visible(logind_available: bool, nested: bool) -> bool {
    logind_available || nested
}

/// Executa a ação. Nunca é chamada por testes: `perform(_, false)` reiniciaria a máquina.
pub async fn perform(action: PowerAction, nested: bool) -> Result<(), String> {
    match plan(action, nested) {
        PowerCall::DryRun(action) => {
            tracing::warn!("{NESTED_ENV}: {action:?} ignorado");
            Ok(())
        }
        PowerCall::Logind(action) => perform_logind(action).await,
    }
}

#[cfg(feature = "logind")]
async fn perform_logind(action: PowerAction) -> Result<(), String> {
    let result = match action {
        PowerAction::Suspend => crate::logind::suspend().await,
        PowerAction::Restart => crate::logind::reboot().await,
        PowerAction::Shutdown => crate::logind::power_off().await,
    };
    result.map_err(|err| err.to_string())
}

#[cfg(not(feature = "logind"))]
async fn perform_logind(_action: PowerAction) -> Result<(), String> {
    Err("logind desativado".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn menu_lists_suspend_restart_shutdown_in_order() {
        assert_eq!(PowerAction::ALL, [PowerAction::Suspend, PowerAction::Restart, PowerAction::Shutdown]);
    }

    #[test]
    fn icons_match_cosmic_icon_theme() {
        assert_eq!(PowerAction::Suspend.icon_name(), "system-suspend-symbolic");
        assert_eq!(PowerAction::Restart.icon_name(), "system-reboot-symbolic");
        assert_eq!(PowerAction::Shutdown.icon_name(), "system-shutdown-symbolic");
    }

    #[test]
    fn nested_mode_never_calls_logind() {
        for a in PowerAction::ALL {
            assert_eq!(plan(a, true), PowerCall::DryRun(a));
        }
    }

    #[test]
    fn real_mode_calls_logind_with_one_click() {
        for a in PowerAction::ALL {
            assert_eq!(plan(a, false), PowerCall::Logind(a));
        }
    }

    #[test]
    fn menu_visible_with_logind_or_nested() {
        assert!(menu_visible(true, false));
        assert!(menu_visible(false, true));
        assert!(!menu_visible(false, false));
    }

    #[test]
    fn nested_flag_only_for_one() {
        assert!(nested_from_value(Some(OsStr::new("1"))));
        assert!(!nested_from_value(None));
        assert!(!nested_from_value(Some(OsStr::new("0"))));
        assert!(!nested_from_value(Some(OsStr::new(""))));
    }
}
