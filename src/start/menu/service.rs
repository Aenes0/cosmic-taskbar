use super::*;

const BUS_NAME: &str = "io.github.Aenes0.CosmicStart";
const BUS_PATH: &str = "/io/github/Aenes0/CosmicStart";

/// Asks the running menu to show or hide; false when none runs, and this
/// one should be it.
pub fn toggle_running() -> bool {
    call_running("Toggle")
}

/// The same for the tools' menu (Win+X).
pub fn tools_running() -> bool {
    call_running("Tools")
}

fn call_running(method: &'static str) -> bool {
    use std::sync::{Mutex, OnceLock};
    // Keep one lightweight runtime and D-Bus session connection alive across clicks
    // so left/right clicks on the Start button never pay a full runtime + D-Bus handshake.
    struct Client {
        runtime: tokio::runtime::Runtime,
        conn: Option<zbus::Connection>,
    }
    static CLIENT: OnceLock<Mutex<Client>> = OnceLock::new();
    let mutex = CLIENT.get_or_init(|| {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("start menu dbus runtime");
        Mutex::new(Client { runtime, conn: None })
    });
    let mut guard = mutex.lock().unwrap_or_else(|e| e.into_inner());
    let Client { runtime, conn } = &mut *guard;
    runtime.block_on(async {
        if conn.is_none() {
            *conn = zbus::Connection::session().await.ok();
        }
        let Some(c) = conn.as_ref() else {
            return false;
        };
        match c
            .call_method(Some(BUS_NAME), BUS_PATH, Some(BUS_NAME), method, &())
            .await
        {
            Ok(_) => true,
            Err(zbus::Error::InputOutput(_)) => {
                // Connection dropped: clear so the next call reconnects.
                *conn = None;
                false
            }
            Err(_) => false,
        }
    })
}

struct Service(mpsc::Sender<Message>);

#[zbus::interface(name = "io.github.Aenes0.CosmicStart")]
impl Service {
    async fn toggle(&self) {
        _ = self.0.clone().send(Message::Toggle).await;
    }

    async fn tools(&self) {
        _ = self.0.clone().send(Message::Tools).await;
    }

    async fn close(&self) {
        _ = self.0.clone().send(Message::Hide).await;
    }
}

pub(super) fn dbus_service() -> Subscription<Message> {
    struct Bus;
    Subscription::run_with(std::any::TypeId::of::<Bus>(), |_| {
        stream::channel(4, |output: mpsc::Sender<Message>| async move {
            let conn = async {
                zbus::connection::Builder::session()?
                    .name(BUS_NAME)?
                    .serve_at(BUS_PATH, Service(output))?
                    .build()
                    .await
            }
            .await;
            match conn {
                // Kept for as long as the menu runs.
                Ok(conn) => {
                    let _conn = conn;
                    futures::future::pending::<()>().await;
                }
                Err(err) => tracing::error!("cannot serve the start menu on D-Bus: {err}"),
            }
        })
    })
}
