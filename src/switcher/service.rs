// SPDX-License-Identifier: GPL-3.0-only

//! Isolated D-Bus service (`io.github.Aenes0.CosmicSwitcher`) and systemd
//! sd_notify / watchdog support for the window switcher.

use std::os::unix::net::UnixDatagram;
use std::time::Duration;

use cosmic::iced::{
    Subscription,
    futures::{SinkExt, StreamExt, channel::mpsc},
    stream,
};

pub const BUS_NAME: &str = "io.github.Aenes0.CosmicSwitcher";
pub const BUS_PATH: &str = "/io/github/Aenes0/CosmicSwitcher";

const CLI_TIMEOUT: Duration = Duration::from_secs(3);

/// Events arriving from the session D-Bus service.
#[derive(Clone, Debug)]
pub enum DbusEvent {
    ShowSwitcher { reverse: bool },
    ShowSwitcherOn { reverse: bool, output: String },
    ShowOverviewOn { output: String },
    Cycle { delta: i32 },
    Release,
    Hide,
    /// Bus name acquired; ready for systemd `Type=notify`.
    Ready,
    /// Another authoritative instance (e.g. systemd unit) took over the bus.
    Quit,
}

struct Service(mpsc::Sender<DbusEvent>);

#[zbus::interface(name = "io.github.Aenes0.CosmicSwitcher")]
impl Service {
    /// Opens the switcher on the active/focused output, or advances selection.
    async fn show_switcher(&self, reverse: bool) {
        let _ = self
            .0
            .clone()
            .send(DbusEvent::ShowSwitcher { reverse })
            .await;
    }

    /// Opens the switcher on `output` (empty = focused output), or advances.
    async fn show_switcher_on(&self, reverse: bool, output: String) {
        let _ = self
            .0
            .clone()
            .send(DbusEvent::ShowSwitcherOn { reverse, output })
            .await;
    }

    /// Opens (or toggles closed) the sticky window overview on `output`.
    async fn show_overview_on(&self, output: String) {
        let _ = self
            .0
            .clone()
            .send(DbusEvent::ShowOverviewOn { output })
            .await;
    }

    /// Moves the switcher selection by `delta`.
    async fn cycle(&self, delta: i32) {
        let _ = self.0.clone().send(DbusEvent::Cycle { delta }).await;
    }

    /// Confirms the current selection and closes.
    async fn release(&self) {
        let _ = self.0.clone().send(DbusEvent::Release).await;
    }

    /// Cancels and closes the switcher without activating a window.
    async fn hide(&self) {
        let _ = self.0.clone().send(DbusEvent::Hide).await;
    }
}

/// Bus-ownership policy: the systemd unit replaces a manually started process;
/// a manually started process allows the unit to replace it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NamePolicy {
    pub replace_existing: bool,
    pub allow_replacement: bool,
}

pub fn name_policy(under_unit: bool) -> NamePolicy {
    NamePolicy {
        replace_existing: under_unit,
        allow_replacement: !under_unit,
    }
}

/// True when systemd launched this process as a `Type=notify` service.
pub fn under_unit() -> bool {
    std::env::var_os("NOTIFY_SOCKET").is_some()
}

/// Serves `io.github.Aenes0.CosmicSwitcher` on the session D-Bus.
pub fn subscription() -> Subscription<DbusEvent> {
    struct SwitcherBus;
    Subscription::run_with(std::any::TypeId::of::<SwitcherBus>(), |_| {
        stream::channel(16, |mut output: mpsc::Sender<DbusEvent>| async move {
            use zbus::fdo::{RequestNameFlags, RequestNameReply};
            let policy = name_policy(under_unit());
            let serving = async {
                let conn = zbus::connection::Builder::session()?
                    .serve_at(BUS_PATH, Service(output.clone()))?
                    .build()
                    .await?;
                let flags = if policy.replace_existing {
                    RequestNameFlags::DoNotQueue | RequestNameFlags::ReplaceExisting
                } else {
                    RequestNameFlags::DoNotQueue | RequestNameFlags::AllowReplacement
                };
                let reply = conn.request_name_with_flags(BUS_NAME, flags).await?;
                let lost = zbus::fdo::DBusProxy::new(&conn)
                    .await?
                    .receive_name_lost()
                    .await?;
                zbus::Result::Ok((conn, reply, lost))
            }
            .await;

            match serving {
                Ok((
                    conn,
                    RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner,
                    mut lost,
                )) => {
                    let _conn = conn;
                    let _ = output.send(DbusEvent::Ready).await;
                    while let Some(signal) = lost.next().await {
                        if signal.args().is_ok_and(|args| args.name() == BUS_NAME) {
                            tracing::info!("another instance took {BUS_NAME}: exiting");
                            break;
                        }
                    }
                    let _ = output.send(DbusEvent::Quit).await;
                }
                Ok((_, reply, _)) => {
                    tracing::info!("another instance already serves {BUS_NAME} ({reply:?}): exiting");
                    let _ = output.send(DbusEvent::Quit).await;
                }
                Err(err) => {
                    tracing::error!("cannot serve {BUS_NAME} on D-Bus: {err}");
                    let _ = output.send(DbusEvent::Quit).await;
                }
            }
        })
    })
}

/// Optional CLI action when invoking `cosmic-taskbar switcher --show ...`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CliAction {
    ShowSwitcher { reverse: bool, output: String },
    ShowOverview { output: String },
    Hide,
}

/// Parses optional one-shot CLI flags after `switcher` / `--switcher`.
/// Returns `None` when no action flag is given (start daemon mode).
pub fn parse_cli_action(args: &[String]) -> Option<CliAction> {
    let mut show = false;
    let mut overview = false;
    let mut hide = false;
    let mut reverse = false;
    let mut output = std::env::var("COSMIC_ACTIVE_OUTPUT").unwrap_or_default();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--show" => show = true,
            "--reverse" => {
                show = true;
                reverse = true;
            }
            "--overview" => overview = true,
            "--hide" => hide = true,
            "--output" => {
                if let Some(val) = args.get(i + 1) {
                    output = val.clone();
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    if hide {
        Some(CliAction::Hide)
    } else if overview {
        Some(CliAction::ShowOverview { output })
    } else if show {
        Some(CliAction::ShowSwitcher { reverse, output })
    } else {
        None
    }
}

/// Calls the D-Bus service (using D-Bus activation if installed) for a CLI action.
/// Returns `true` if a CLI action was handled and the process should exit.
pub fn forward_cli(args: &[String]) -> bool {
    let Some(action) = parse_cli_action(args) else {
        return false;
    };
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return true;
    };
    runtime.block_on(async move {
        let conn = match zbus::connection::Builder::session()
            .map(|b| b.method_timeout(CLI_TIMEOUT))
        {
            Ok(builder) => match builder.build().await {
                Ok(c) => c,
                Err(err) => {
                    tracing::error!("cannot connect to session D-Bus: {err}");
                    return;
                }
            },
            Err(err) => {
                tracing::error!("cannot build session D-Bus client: {err}");
                return;
            }
        };
        let res = match action {
            CliAction::ShowSwitcher { reverse, output } => {
                conn.call_method(
                    Some(BUS_NAME),
                    BUS_PATH,
                    Some(BUS_NAME),
                    "ShowSwitcherOn",
                    &(reverse, output),
                )
                .await
            }
            CliAction::ShowOverview { output } => {
                conn.call_method(
                    Some(BUS_NAME),
                    BUS_PATH,
                    Some(BUS_NAME),
                    "ShowOverviewOn",
                    &(output,),
                )
                .await
            }
            CliAction::Hide => {
                conn.call_method(Some(BUS_NAME), BUS_PATH, Some(BUS_NAME), "Hide", &())
                    .await
            }
        };
        if let Err(err) = res {
            tracing::warn!("failed to call {BUS_NAME}: {err}");
        }
    });
    true
}

/// Notify systemd that the switcher daemon is ready on D-Bus.
pub fn sd_ready() {
    sd_notify("READY=1");
}

/// Notify systemd watchdog that the UI loop is alive.
pub fn sd_beat() {
    sd_notify("WATCHDOG=1");
}

/// Half the configured `WATCHDOG_USEC`, when systemd monitors this PID.
pub fn watchdog_period() -> Option<Duration> {
    let usec = std::env::var("WATCHDOG_USEC").ok();
    let pid = std::env::var("WATCHDOG_PID").ok();
    watchdog_period_of(usec.as_deref(), pid.as_deref(), std::process::id())
}

fn watchdog_period_of(usec: Option<&str>, pid: Option<&str>, own: u32) -> Option<Duration> {
    let usec: u64 = usec?.trim().parse().ok().filter(|u| *u > 0)?;
    if let Some(pid) = pid
        && pid.trim().parse::<u32>().ok() != Some(own)
    {
        return None;
    }
    Some(Duration::from_micros(usec / 2))
}

fn sd_notify(state: &str) {
    let Some(path) = std::env::var_os("NOTIFY_SOCKET") else {
        return;
    };
    let sent = UnixDatagram::unbound().and_then(|socket| {
        let bytes = path.as_encoded_bytes();
        let address = match bytes.strip_prefix(b"@") {
            Some(name) => {
                use std::os::linux::net::SocketAddrExt;
                std::os::unix::net::SocketAddr::from_abstract_name(name)?
            }
            None => std::os::unix::net::SocketAddr::from_pathname(&path)?,
        };
        socket.send_to_addr(state.as_bytes(), &address)
    });
    if let Err(err) = sent {
        tracing::warn!("sd_notify({state}) failed: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_replaces_manual_and_manual_allows_replacement() {
        assert_eq!(
            name_policy(true),
            NamePolicy {
                replace_existing: true,
                allow_replacement: false,
            }
        );
        assert_eq!(
            name_policy(false),
            NamePolicy {
                replace_existing: false,
                allow_replacement: true,
            }
        );
    }

    #[test]
    fn watchdog_period_parses_half_interval() {
        assert_eq!(
            watchdog_period_of(Some("20000000"), None, 42),
            Some(Duration::from_secs(10))
        );
        assert_eq!(
            watchdog_period_of(Some("20000000"), Some("42"), 42),
            Some(Duration::from_secs(10))
        );
        assert_eq!(watchdog_period_of(Some("20000000"), Some("99"), 42), None);
        assert_eq!(watchdog_period_of(Some("0"), None, 42), None);
        assert_eq!(watchdog_period_of(None, None, 42), None);
    }

    #[test]
    fn cli_action_parses_show_reverse_overview_and_output() {
        let s = |v: &[&str]| v.iter().map(|x| (*x).to_string()).collect::<Vec<_>>();
        assert_eq!(parse_cli_action(&s(&[])), None);
        assert_eq!(
            parse_cli_action(&s(&["--show", "--output", "DP-1"])),
            Some(CliAction::ShowSwitcher {
                reverse: false,
                output: "DP-1".into(),
            })
        );
        assert_eq!(
            parse_cli_action(&s(&["--reverse", "--output", "HDMI-A-1"])),
            Some(CliAction::ShowSwitcher {
                reverse: true,
                output: "HDMI-A-1".into(),
            })
        );
        assert_eq!(
            parse_cli_action(&s(&["--overview", "--output", "DP-2"])),
            Some(CliAction::ShowOverview {
                output: "DP-2".into(),
            })
        );
        assert_eq!(parse_cli_action(&s(&["--hide"])), Some(CliAction::Hide));
    }
}
