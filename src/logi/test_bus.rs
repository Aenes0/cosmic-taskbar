//! Apoio dos testes de barramento (só `cfg(test)`): um `dbus-daemon` de sessão privado e um
//! servidor `org.freedesktop.Notifications` falso que grava as chamadas `Notify`.
//! Mesmo modelo do `PrivateBus` do daemon de notificações (`tests/common/mod.rs`).

use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use zbus::message::Header;
use zbus::zvariant::{OwnedValue, Value};

use crate::logi::notify::Hint;

pub const NAME: &str = "org.freedesktop.Notifications";

/// `dbus-daemon` de sessão só deste teste; morre no `Drop`.
pub struct PrivateBus {
    pub address: String,
    child: Child,
}

impl PrivateBus {
    /// Endereço escolhido pelo `dbus-daemon`.
    pub fn start() -> PrivateBus {
        PrivateBus::spawn(None)
    }

    /// Endereço fixo `unix:path=<socket>` (para derrubar e subir de novo no mesmo endereço).
    pub fn start_at(socket: &Path) -> PrivateBus {
        let _ = std::fs::remove_file(socket);
        PrivateBus::spawn(Some(format!("unix:path={}", socket.display())))
    }

    fn spawn(address: Option<String>) -> PrivateBus {
        let mut cmd = Command::new("dbus-daemon");
        cmd.args(["--session", "--nofork", "--print-address=1"]);
        if let Some(address) = &address {
            cmd.arg(format!("--address={address}"));
        }
        let mut child = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("dbus-daemon não iniciou");
        let stdout = child.stdout.take().expect("sem stdout do dbus-daemon");
        let mut line = String::new();
        BufReader::new(stdout).read_line(&mut line).expect("dbus-daemon não imprimiu o endereço");
        let printed = line.trim().to_string();
        assert!(!printed.is_empty(), "endereço vazio do dbus-daemon");
        // Com endereço fixo, devolve o pedido (o impresso traz `,guid=`, que muda a cada subida).
        PrivateBus { address: address.unwrap_or(printed), child }
    }

    /// Derruba o barramento (o `Drop` continua seguro).
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// Espera o nome ficar sem dono (≤ 2 s): a saída do dono é processada pelo barramento depois.
    pub fn wait_name_released(&self) {
        let rt = runtime();
        let released = rt.block_on(async {
            let conn = zbus::connection::Builder::address(self.address.as_str())?.build().await?;
            let proxy = zbus::fdo::DBusProxy::new(&conn).await?;
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            while std::time::Instant::now() < deadline {
                if !proxy.name_has_owner(NAME.try_into()?).await? {
                    return Ok::<bool, zbus::Error>(true);
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            Ok(false)
        });
        assert!(released.expect("cliente do barramento privado"), "{NAME} ainda tem dono");
    }
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime tokio do teste")
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        self.kill();
    }
}

/// Uma chamada `Notify` como chegou ao servidor.
#[derive(Debug, Clone, PartialEq)]
pub struct Recorded {
    pub signature: String,
    pub app_name: String,
    pub replaces_id: u32,
    pub app_icon: String,
    pub summary: String,
    pub body: String,
    pub actions: Vec<String>,
    pub hints: BTreeMap<String, Hint>,
    pub expire_timeout: i32,
}

/// Servidor falso numa thread de teste: toma o nome, responde a `Notify` com um id e grava a
/// chamada. Para no `Drop`.
pub struct Recorder {
    pub calls: Receiver<Recorded>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Recorder {
    pub fn start(bus: &PrivateBus) -> Recorder {
        let address = bus.address.clone();
        let (tx, calls) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let thread = std::thread::spawn(move || {
            runtime().block_on(async move {
                let server = FakeServer { tx, next_id: std::sync::atomic::AtomicU32::new(1) };
                let built = async {
                    zbus::connection::Builder::address(address.as_str())?
                        .serve_at("/org/freedesktop/Notifications", server)?
                        .name(NAME)?
                        .build()
                        .await
                }
                .await;
                match built {
                    Ok(conn) => {
                        let _ = ready_tx.send(Ok(()));
                        let _ = stopped.await;
                        drop(conn);
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e.to_string()));
                    }
                }
            });
        });
        ready_rx.recv().expect("servidor falso não subiu").expect("RequestName do servidor falso");
        Recorder { calls, stop: Some(stop), thread: Some(thread) }
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

struct FakeServer {
    tx: Sender<Recorded>,
    next_id: std::sync::atomic::AtomicU32,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl FakeServer {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        #[zbus(header)] header: Header<'_>,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> u32 {
        let signature = header.signature().to_string_no_parens();
        let hints = hints.into_iter().filter_map(|(k, v)| hint(&v).map(|h| (k, h))).collect();
        let _ = self.tx.send(Recorded { signature, app_name, replaces_id, app_icon, summary, body, actions, hints, expire_timeout });
        self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
}

fn hint(v: &Value<'_>) -> Option<Hint> {
    match v {
        Value::Bool(b) => Some(Hint::Bool(*b)),
        Value::I32(i) => Some(Hint::Int(*i)),
        Value::Str(s) => Some(Hint::Str(s.to_string())),
        _ => None,
    }
}
