//! Avisos pelo daemon de notificações: cada `Pill` vira uma chamada `Notify` com as dicas da
//! pílula (`x-estilo osd`, `x-tom`, `x-pontos`…); a bateria baixa vira cartão. O daemon desenha.
//!
//! O cliente D-Bus (zbus) roda numa thread própria com um runtime tokio `current_thread` (o
//! calloop dos dispositivos não tem executor). `Notifier::send` só põe a pílula num canal e volta
//! na hora: nunca bloqueia o laço dos dispositivos, nem com o barramento travado. A thread envia a
//! chamada sem esperar a resposta; as respostas chegam por um `MessageStream` e são descartadas
//! (erro vira uma linha de log por tipo até o próximo sucesso). Conexão caída é descartada; o
//! próximo envio reconecta.

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use futures::StreamExt;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use zbus::fdo::{DBusProxy, NameOwnerChanged, NameOwnerChangedStream};
use zbus::message::Type as MessageType;
use zbus::proxy::CacheProperties;
use zbus::zvariant::Value as DbusValue;
use zbus::{Connection, Message, MessageStream};

use crate::logi::message::{Icon, Pill, Tone, Value};

pub const APP_NAME: &str = "logi-osd";
/// Chave `synchronous`: o daemon troca a pílula anterior do logi-osd pela nova (nunca empilha).
pub const SYNC_KEY: &str = "logi-osd";

const DEST: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";
const IFACE: &str = "org.freedesktop.Notifications";
const ERR_SERVICE_UNKNOWN: &str = "org.freedesktop.DBus.Error.ServiceUnknown";
const ERR_NAME_HAS_NO_OWNER: &str = "org.freedesktop.DBus.Error.NameHasNoOwner";

/// Tipos de erro locais (os de resposta usam o nome do erro D-Bus).
const KIND_CONNECT: &str = "conexão";
const KIND_SEND: &str = "envio";

/// Prazo para abrir a conexão (barramento travado não prende a fila para sempre).
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
/// Chamadas sem resposta lembradas (daemon travado não faz a lista crescer sem fim).
const MAX_INFLIGHT: usize = 256;

#[derive(Debug, Clone, PartialEq)]
pub enum Hint {
    Bool(bool),
    Int(i32),
    Str(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct NotifyArgs {
    pub app_name: &'static str,
    pub app_icon: &'static str,
    pub summary: String,
    pub body: String,
    pub hints: Vec<(&'static str, Hint)>,
    pub expire_timeout: i32,
}

fn icon_name(icon: Icon) -> &'static str {
    match icon {
        Icon::CapsLock => "logi-caps-lock",
        Icon::NumLock => "logi-num-lock",
        Icon::Insert => "logi-insert",
        Icon::FnLock => "logi-fn-lock",
        Icon::KeyboardBacklight => "logi-backlight",
        Icon::Battery => "logi-battery",
        Icon::Charging => "logi-battery-charging",
        Icon::Switch => "logi-switch",
    }
}

fn tom(tone: Tone) -> &'static str {
    match tone {
        Tone::Accent => "normal",
        Tone::Dim => "apagado",
        Tone::Warning => "atencao",
        Tone::Critical => "critico",
    }
}

fn str_hint(key: &'static str, value: &str) -> (&'static str, Hint) {
    (key, Hint::Str(value.to_owned()))
}

/// Mapeamento puro da pílula para os argumentos do `Notify` (tabela do brief da integração T3).
pub fn to_notify(p: &Pill) -> NotifyArgs {
    let (body, value_tom) = match &p.value {
        Value::Text(text, tone) => (text.clone(), Some(tom(*tone))),
        Value::None => (String::new(), Some(tom(Tone::Dim))),
        Value::Dots { .. } => (String::new(), None),
    };

    // Bateria baixa/crítica: cartão (sem `x-estilo`, sem `transient`), fica no histórico.
    if p.icon == Icon::Battery {
        return NotifyArgs {
            app_name: APP_NAME,
            app_icon: icon_name(p.icon),
            summary: p.label.clone(),
            body,
            hints: vec![
                str_hint("x-tom", tom(p.icon_tone)),
                str_hint("category", "device"),
                str_hint("sound-name", "battery-low"),
            ],
            expire_timeout: -1,
        };
    }

    let mut hints = vec![
        str_hint("x-estilo", "osd"),
        ("transient", Hint::Bool(true)),
        str_hint("synchronous", SYNC_KEY),
    ];
    if let Some(t) = value_tom {
        hints.push(str_hint("x-tom", t));
    }
    if let Value::Dots { on, total } = p.value {
        hints.push(("x-pontos", Hint::Int(i32::from(on))));
        hints.push(("x-pontos-total", Hint::Int(i32::from(total))));
    }
    NotifyArgs {
        app_name: APP_NAME,
        app_icon: icon_name(p.icon),
        summary: p.label.clone(),
        body,
        hints,
        expire_timeout: i32::try_from(p.duration.as_millis()).unwrap_or(i32::MAX),
    }
}

/// Chamada `Notify` `(s u s s s as a{sv} i)`, com resposta esperada (a Central casa chamada e resposta).
fn notify_message(a: &NotifyArgs) -> Result<Message, String> {
    let hints: HashMap<&str, DbusValue<'_>> = a
        .hints
        .iter()
        .map(|(key, hint)| {
            let value = match hint {
                Hint::Bool(b) => DbusValue::from(*b),
                Hint::Int(i) => DbusValue::from(*i),
                Hint::Str(s) => DbusValue::from(s.as_str()),
            };
            (*key, value)
        })
        .collect();
    let body = (a.app_name, 0u32, a.app_icon, a.summary.as_str(), a.body.as_str(), Vec::<&str>::new(), hints, a.expire_timeout);
    Message::method_call(PATH, "Notify")
        .and_then(|b| b.destination(DEST))
        .and_then(|b| b.interface(IFACE))
        .and_then(|b| b.build(&body))
        .map_err(|e| e.to_string())
}

/// Uma linha de log por tipo de erro até o próximo sucesso (sem inundar o journal a cada tecla).
#[derive(Debug, Default)]
struct ErrorLog {
    seen: Vec<String>,
}

impl ErrorLog {
    /// Loga se `kind` ainda não foi logado desde o último sucesso; devolve se logou.
    fn report(&mut self, kind: &str, detail: &str) -> bool {
        if self.seen.iter().any(|k| k == kind) {
            return false;
        }
        self.seen.push(kind.to_owned());
        eprintln!("logi-osd: {detail}");
        true
    }

    fn success(&mut self) {
        self.seen.clear();
    }
}

/// Só a bateria baixa/crítica vira cartão (fica no histórico), então só ela é reenviada.
fn is_card(p: &Pill) -> bool {
    p.icon == Icon::Battery
}

/// Estado visto pelos dois lados (o `Notifier` no laço e a thread do cliente).
#[derive(Debug, Default)]
struct Shared {
    log: ErrorLog,
    /// Último cartão de bateria não entregue, com o número do pedido (reenvio na volta do daemon).
    pending: Option<(u64, Pill)>,
    /// Número do último cartão pedido: só ele pode virar o pendente (o mais novo substitui o antigo).
    last_card: u64,
    connected: bool,
}

type SharedRef = Arc<Mutex<Shared>>;

fn lock(shared: &SharedRef) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

fn report(shared: &SharedRef, kind: &str, detail: &str) {
    lock(shared).log.report(kind, detail);
}

/// Uma pílula a entregar; `card` = número do pedido se for cartão.
#[derive(Debug)]
struct Job {
    card: Option<u64>,
    pill: Pill,
}

/// Guarda o cartão que não saiu, se ainda for o último pedido (pílula não se guarda: perdeu a hora).
fn hold(shared: &SharedRef, job: Job) {
    if let Some(seq) = job.card {
        let mut s = lock(shared);
        if s.last_card == seq {
            s.pending = Some((seq, job.pill));
        }
    }
}

/// `NameOwnerChanged` do nome do daemon com dono novo (o daemon subiu).
fn is_owner_gained(signal: &NameOwnerChanged) -> bool {
    signal.args().is_ok_and(|a| a.name().as_str() == DEST && a.new_owner().is_some())
}

/// Conexão viva, só na thread do cliente.
struct Live {
    conn: Connection,
    /// Todas as mensagens recebidas; só as respostas aos nossos `Notify` interessam.
    replies: MessageStream,
    /// Troca de dono do nome do daemon; `None` se o `AddMatch` falhou (fica o reenvio no próximo envio).
    owners: Option<NameOwnerChangedStream>,
    /// `Notify` enviados e ainda sem resposta, por serial, na ordem de envio (com o cartão, se for).
    inflight: Vec<(NonZeroU32, Option<Job>)>,
}

enum Event {
    Job(Job),
    Reply(Option<zbus::Result<Message>>),
    Owner(Option<NameOwnerChanged>),
    Quit,
}

async fn next_owner(owners: &mut Option<NameOwnerChangedStream>) -> Option<NameOwnerChanged> {
    match owners {
        Some(stream) => stream.next().await,
        None => std::future::pending().await,
    }
}

async fn watch_owner(conn: &Connection) -> zbus::Result<NameOwnerChangedStream> {
    let proxy = DBusProxy::builder(conn).cache_properties(CacheProperties::No).build().await?;
    proxy.receive_name_owner_changed_with_args(&[(0, DEST)]).await
}

async fn open(address: Option<&str>, shared: &SharedRef) -> zbus::Result<Live> {
    let builder = match address {
        None => zbus::connection::Builder::session()?,
        Some(address) => zbus::connection::Builder::address(address)?,
    };
    let conn = builder.build().await?;
    // Antes de qualquer `Notify`: o stream só vê o que chega depois de criado.
    let replies = MessageStream::from(&conn);
    // Falha aqui só desliga o reenvio na volta do daemon; o reenvio no próximo envio fica.
    let owners = match watch_owner(&conn).await {
        Ok(stream) => Some(stream),
        Err(e) => {
            report(shared, "match", &format!("sem aviso de troca de dono do daemon de notificações: {e}"));
            None
        }
    };
    Ok(Live { conn, replies, owners, inflight: Vec::new() })
}

/// O lado da thread: conexão, envios, respostas e reenvio do pendente.
struct Client {
    /// `None` = barramento de sessão do ambiente.
    address: Option<String>,
    shared: SharedRef,
    live: Option<Live>,
}

impl Client {
    async fn run(mut self, mut rx: UnboundedReceiver<Job>) {
        loop {
            let event = match self.live.as_mut() {
                None => rx.recv().await.map_or(Event::Quit, Event::Job),
                Some(Live { replies, owners, .. }) => tokio::select! {
                    job = rx.recv() => job.map_or(Event::Quit, Event::Job),
                    reply = replies.next() => Event::Reply(reply),
                    owner = next_owner(owners) => Event::Owner(owner),
                },
            };
            match event {
                Event::Quit => return,
                Event::Job(job) => self.deliver(job).await,
                Event::Reply(Some(Ok(msg))) => self.on_reply(&msg).await,
                // Erro de leitura ou fim do stream: a conexão caiu; o próximo envio reconecta.
                Event::Reply(_) => self.drop_connection(),
                Event::Owner(Some(signal)) => {
                    if is_owner_gained(&signal) {
                        self.resend_pending().await;
                    }
                }
                Event::Owner(None) => {
                    if let Some(live) = self.live.as_mut() {
                        live.owners = None;
                    }
                }
            }
        }
    }

    async fn connect(&mut self) {
        match tokio::time::timeout(CONNECT_TIMEOUT, open(self.address.as_deref(), &self.shared)).await {
            Ok(Ok(live)) => {
                self.live = Some(live);
                lock(&self.shared).connected = true;
            }
            Ok(Err(e)) => report(&self.shared, KIND_CONNECT, &format!("sem barramento de sessão: {e}")),
            Err(_) => report(&self.shared, KIND_CONNECT, "sem barramento de sessão: tempo esgotado ao conectar"),
        }
    }

    fn drop_connection(&mut self) {
        self.live = None;
        lock(&self.shared).connected = false;
    }

    /// Envia o `Notify` sem esperar a resposta (reconecta antes, se preciso). Cartão: fica em voo
    /// até a resposta; se não sair, vira o pendente.
    async fn deliver(&mut self, job: Job) {
        if self.live.is_none() {
            self.connect().await;
        }
        let shared = &self.shared;
        let Some(live) = self.live.as_mut() else {
            hold(shared, job);
            return;
        };
        let msg = match notify_message(&to_notify(&job.pill)) {
            Ok(msg) => msg,
            Err(e) => {
                report(shared, KIND_SEND, &format!("chamada Notify não montada: {e}"));
                hold(shared, job);
                return;
            }
        };
        let serial = msg.primary_header().serial_num();
        if live.conn.send(&msg).await.is_err() {
            report(shared, KIND_SEND, "a conexão com o barramento de sessão caiu; o próximo aviso reconecta");
            hold(shared, job);
            self.drop_connection();
            return;
        }
        if live.inflight.len() >= MAX_INFLIGHT {
            live.inflight.remove(0);
        }
        live.inflight.push((serial, job.card.is_some().then_some(job)));
    }

    /// Reenvia o cartão pendente (uma vez: sai do pendente ao ser enviado).
    async fn resend_pending(&mut self) {
        let card = lock(&self.shared).pending.take();
        if let Some((seq, pill)) = card {
            self.deliver(Job { card: Some(seq), pill }).await;
        }
    }

    /// Resposta a um `Notify` nosso: sucesso limpa o log e reenvia o pendente; erro loga uma vez
    /// por tipo e, se for um cartão sem dono no nome, o cartão vira o pendente.
    async fn on_reply(&mut self, msg: &Message) {
        let header = msg.header();
        let Some(serial) = header.reply_serial() else { return };
        let Some(live) = self.live.as_mut() else { return };
        let Some(idx) = live.inflight.iter().position(|(s, _)| *s == serial) else { return };
        let (_, card) = live.inflight.remove(idx);
        match msg.message_type() {
            MessageType::MethodReturn => {
                lock(&self.shared).log.success();
                // O daemon respondeu: é a hora de reenviar o cartão que ficou para trás.
                self.resend_pending().await;
            }
            MessageType::Error => {
                let name = header.error_name().map_or_else(|| "erro D-Bus".to_owned(), ToString::to_string);
                if let Some(card) = card
                    && (name == ERR_SERVICE_UNKNOWN || name == ERR_NAME_HAS_NO_OWNER)
                {
                    hold(&self.shared, card);
                }
                let text = msg.body().deserialize::<String>().unwrap_or_default();
                let detail = format!("aviso não entregue ao daemon de notificações ({name}): {text}");
                report(&self.shared, &name, &detail);
            }
            _ => {}
        }
    }
}

pub struct Notifier {
    tx: UnboundedSender<Job>,
    shared: SharedRef,
}

impl Notifier {
    /// `address`: None = barramento de sessão do ambiente; Some = endereço explícito (testes).
    /// Sobe a thread do cliente; conecta no 1º envio.
    pub fn new(address: Option<String>) -> Notifier {
        let shared = SharedRef::default();
        let (tx, rx) = unbounded_channel();
        let client = Client { address, shared: shared.clone(), live: None };
        let spawned = std::thread::Builder::new().name("logi-dbus".into()).spawn(move || {
            match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(rt) => rt.block_on(client.run(rx)),
                Err(e) => report(&client.shared, KIND_CONNECT, &format!("runtime do cliente D-Bus: {e}")),
            }
        });
        if let Err(e) = spawned {
            report(&shared, KIND_CONNECT, &format!("thread do cliente D-Bus: {e}"));
        }
        Notifier { tx, shared }
    }

    /// Não bloqueia: põe a pílula no canal da thread do cliente e volta. Lá, a chamada sai com
    /// resposta esperada; respostas e erros são descartados (erro: uma linha de log por tipo até
    /// o próximo sucesso). Conexão caída é descartada; o próximo envio reconecta.
    pub fn send(&self, p: &Pill) {
        let card = is_card(p).then(|| {
            // Um cartão novo substitui o pendente (que não é mais o último).
            let mut s = lock(&self.shared);
            s.pending = None;
            s.last_card += 1;
            s.last_card
        });
        if let Err(e) = self.tx.send(Job { card, pill: p.clone() }) {
            report(&self.shared, KIND_SEND, "o cliente D-Bus parou; aviso não entregue");
            hold(&self.shared, e.0);
        }
    }

    #[cfg(test)]
    fn connected(&self) -> bool {
        lock(&self.shared).connected
    }

    #[cfg(test)]
    fn pending(&self) -> Option<Pill> {
        lock(&self.shared).pending.as_ref().map(|(_, p)| p.clone())
    }

    #[cfg(test)]
    fn logged(&self) -> Vec<String> {
        lock(&self.shared).log.seen.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::logi::battery::BatteryAlert;
    use crate::logi::message::{self, Durations, LockKey};

    const fn ms(n: u64) -> Duration { Duration::from_millis(n) }
    const D: Durations = Durations { normal: ms(1500), critical: ms(4000) };
    const VOCAB: [&str; 8] = [
        "logi-caps-lock",
        "logi-num-lock",
        "logi-insert",
        "logi-fn-lock",
        "logi-backlight",
        "logi-battery",
        "logi-battery-charging",
        "logi-switch",
    ];

    fn hints(a: &NotifyArgs) -> BTreeMap<&'static str, Hint> {
        a.hints.iter().cloned().collect()
    }

    fn s(v: &str) -> Hint { Hint::Str(v.into()) }

    fn osd(tom: Option<&str>) -> BTreeMap<&'static str, Hint> {
        let mut m = BTreeMap::from([("x-estilo", s("osd")), ("transient", Hint::Bool(true)), ("synchronous", s("logi-osd"))]);
        if let Some(t) = tom {
            m.insert("x-tom", s(t));
        }
        m
    }

    fn card(tom: &str) -> BTreeMap<&'static str, Hint> {
        BTreeMap::from([("x-tom", s(tom)), ("category", s("device")), ("sound-name", s("battery-low"))])
    }

    #[test]
    fn caps_on_maps_to_osd_pill() {
        let a = to_notify(&message::lock(LockKey::Caps, true, &D));
        assert_eq!((a.app_name, a.app_icon, a.summary.as_str(), a.body.as_str()), ("logi-osd", "logi-caps-lock", "Caps Lock", "Ativado"));
        assert_eq!(hints(&a), osd(Some("normal")));
        assert_eq!(a.expire_timeout, 1500);
    }

    #[test]
    fn caps_off_is_apagado() {
        let a = to_notify(&message::lock(LockKey::Caps, false, &D));
        assert_eq!((a.app_icon, a.body.as_str()), ("logi-caps-lock", "Desativado"));
        assert_eq!(hints(&a), osd(Some("apagado")));
        let num = to_notify(&message::lock(LockKey::Num, true, &D));
        assert_eq!((num.app_icon, num.summary.as_str()), ("logi-num-lock", "Num Lock"));
        let ins = to_notify(&message::lock(LockKey::Insert, false, &D));
        assert_eq!((ins.app_icon, ins.summary.as_str()), ("logi-insert", "Insert"));
    }

    #[test]
    fn fn_lock_texts_and_icon() {
        let f = to_notify(&message::fn_lock(false, &D));
        assert_eq!((f.app_icon, f.summary.as_str(), f.body.as_str()), ("logi-fn-lock", "Fn", "Teclas F"));
        assert_eq!(hints(&f), osd(Some("apagado")));
        assert_eq!(to_notify(&message::fn_lock(true, &D)).body, "Teclas de mídia");
        assert_eq!(f.expire_timeout, 1500);
    }

    #[test]
    fn backlight_maps_dots() {
        let a = to_notify(&message::backlight(3, 9, &D));
        assert_eq!((a.app_icon, a.summary.as_str(), a.body.as_str()), ("logi-backlight", "Teclado", ""));
        let mut want = osd(None);
        want.insert("x-pontos", Hint::Int(3));
        want.insert("x-pontos-total", Hint::Int(8));
        assert_eq!(hints(&a), want);
        assert_eq!(a.expire_timeout, 1500);
    }

    #[test]
    fn battery_low_is_atencao_card() {
        let a = to_notify(&message::battery(BatteryAlert::Low(15), "MX Keys Mini", &D));
        assert_eq!((a.app_icon, a.summary.as_str(), a.body.as_str()), ("logi-battery", "MX Keys Mini: bateria baixa", "15%"));
        assert_eq!(hints(&a), card("atencao"));
        assert_eq!(a.expire_timeout, -1);
    }

    #[test]
    fn battery_critical_is_critico_card() {
        let a = to_notify(&message::battery(BatteryAlert::Critical(5), "MX Keys Mini", &D));
        assert_eq!((a.app_icon, a.summary.as_str(), a.body.as_str()), ("logi-battery", "MX Keys Mini: bateria baixa", "5%"));
        assert_eq!(hints(&a), card("critico"));
        assert_eq!(a.expire_timeout, -1);
    }

    #[test]
    fn charging_and_charged_are_pills() {
        let c = to_notify(&message::battery(BatteryAlert::Charging(42), "G602", &D));
        assert_eq!((c.app_icon, c.summary.as_str(), c.body.as_str()), ("logi-battery-charging", "G602 carregando", "42%"));
        assert_eq!(hints(&c), osd(Some("apagado")));
        let d = to_notify(&message::battery(BatteryAlert::Charged, "G602", &D));
        assert_eq!((d.app_icon, d.summary.as_str(), d.body.as_str()), ("logi-battery-charging", "G602 carregado", ""));
        assert_eq!(hints(&d), osd(Some("apagado")));
        assert_eq!((c.expire_timeout, d.expire_timeout), (1500, 1500));
    }

    #[test]
    fn connected_with_and_without_channel() {
        let a = to_notify(&message::connected("MX Keys Mini", Some((3, 0)), &D));
        assert_eq!((a.app_icon, a.summary.as_str(), a.body.as_str()), ("logi-switch", "MX Keys Mini conectado", "Canal 1"));
        assert_eq!(hints(&a), osd(Some("apagado")));
        let b = to_notify(&message::connected("G602", None, &D));
        assert_eq!((b.summary.as_str(), b.body.as_str()), ("G602 conectado", ""));
        assert_eq!(hints(&b), osd(Some("apagado")));
    }

    #[test]
    fn every_demo_pill_maps() {
        let pills = crate::logi::demo_pills(&D);
        assert_eq!(pills.len(), 10);
        for p in &pills {
            let a = to_notify(p);
            assert!(VOCAB.contains(&a.app_icon), "ícone fora do vocabulário: {}", a.app_icon);
            assert_eq!(a.app_name, "logi-osd");
            assert!(!a.summary.is_empty());
            assert!(notify_message(&a).is_ok());
        }
    }

    #[test]
    fn duration_in_ms_saturates() {
        let mut p = message::lock(LockKey::Caps, true, &D);
        p.duration = Duration::from_secs(u64::MAX / 4);
        assert_eq!(to_notify(&p).expire_timeout, i32::MAX);
    }

    #[test]
    fn error_log_once_per_kind_until_success() {
        let mut log = ErrorLog::default();
        assert!(log.report("a", "x"));
        assert!(!log.report("a", "x"));
        assert!(log.report("b", "y"));
        log.success();
        assert!(log.report("a", "x"));
    }

    mod bus_tests {
        use std::sync::mpsc::Receiver;
        use std::time::Instant;

        use super::super::*;
        use super::D;
        use crate::logi::message::{self, LockKey};
        use crate::logi::test_bus::{PrivateBus, Recorded, Recorder};

        const WAIT: Duration = Duration::from_secs(2);
        const FAST: Duration = Duration::from_millis(50);

        /// Espera a próxima chamada gravada (≤ 2 s).
        fn next_call(calls: &Receiver<Recorded>) -> Option<Recorded> {
            calls.recv_timeout(WAIT).ok()
        }

        /// Espera a thread do cliente chegar a `cond` (≤ 2 s); as asserções vêm depois.
        fn wait_until(mut cond: impl FnMut() -> bool) {
            let deadline = Instant::now() + WAIT;
            while !cond() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
        }

        /// Janela curta sem condição (para provar que nada mais acontece).
        fn settle(d: Duration) {
            std::thread::sleep(d);
        }

        #[test]
        fn notifier_sends_notify_to_bus() {
            let bus = PrivateBus::start();
            let rec = Recorder::start(&bus);
            let n = Notifier::new(Some(bus.address.clone()));
            let pill = message::lock(LockKey::Caps, true, &D);
            n.send(&pill);
            let got = next_call(&rec.calls).expect("o Notify não chegou ao servidor");
            let want = to_notify(&pill);
            assert_eq!(got.signature, "susssasa{sv}i");
            assert_eq!(got.app_name, want.app_name);
            assert_eq!(got.replaces_id, 0);
            assert_eq!(got.app_icon, want.app_icon);
            assert_eq!(got.summary, want.summary);
            assert_eq!(got.body, want.body);
            assert!(got.actions.is_empty());
            let want_hints: std::collections::BTreeMap<String, Hint> =
                want.hints.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
            assert_eq!(got.hints, want_hints);
            assert_eq!(got.expire_timeout, 1500);
            // A resposta chega à thread do cliente e é descartada (sucesso limpa o log de erros).
            settle(Duration::from_millis(100));
            assert!(n.connected());
            assert!(n.logged().is_empty());
        }

        #[test]
        fn send_without_daemon_does_not_block() {
            let bus = PrivateBus::start();
            let n = Notifier::new(Some(bus.address.clone()));
            let pill = message::lock(LockKey::Caps, true, &D);
            for _ in 0..3 {
                let t = Instant::now();
                n.send(&pill);
                assert!(t.elapsed() < FAST, "send levou {:?}", t.elapsed());
            }
            // O erro do barramento (nome sem dono) chega à thread, é logado uma vez e a conexão segue.
            wait_until(|| !n.logged().is_empty());
            settle(Duration::from_millis(100));
            assert!(n.connected());
            assert_eq!(n.logged(), vec!["org.freedesktop.DBus.Error.ServiceUnknown".to_string()]);
        }

        /// Daemon trocando de dono (crash, `dev.sh parar`, volta do oficial): o barramento segue de
        /// pé, então a mesma conexão serve; o aviso vai para quem for o dono na hora.
        #[test]
        fn follows_name_owner_change() {
            let bus = PrivateBus::start();
            let n = Notifier::new(Some(bus.address.clone()));
            {
                let rec = Recorder::start(&bus);
                n.send(&message::lock(LockKey::Caps, true, &D));
                assert!(next_call(&rec.calls).is_some(), "o primeiro Notify não chegou");
            }
            // Sem dono: erro logado, conexão mantida.
            bus.wait_name_released();
            n.send(&message::lock(LockKey::Caps, false, &D));
            wait_until(|| !n.logged().is_empty());
            assert!(n.connected());
            assert_eq!(n.logged(), vec!["org.freedesktop.DBus.Error.ServiceUnknown".to_string()]);

            let rec = Recorder::start(&bus);
            n.send(&message::fn_lock(true, &D));
            let got = next_call(&rec.calls).expect("o Notify para o dono novo não chegou");
            assert_eq!(got.body, "Teclas de mídia");
            wait_until(|| n.logged().is_empty());
            assert!(n.logged().is_empty());
        }

        fn low(pct: u8) -> crate::logi::message::Pill {
            message::battery(crate::logi::battery::BatteryAlert::Low(pct), "MX Keys Mini", &D)
        }

        fn drain_calls(calls: &Receiver<Recorded>, d: Duration) -> Vec<Recorded> {
            settle(d);
            calls.try_iter().collect()
        }

        #[test]
        fn battery_card_without_daemon_is_held_and_delivered_when_daemon_returns() {
            let bus = PrivateBus::start();
            let n = Notifier::new(Some(bus.address.clone()));
            let card = low(15);
            n.send(&card);
            wait_until(|| n.pending().is_some());
            assert_eq!(n.pending(), Some(card.clone()), "o cartão não ficou pendente");

            // O daemon volta; nenhum evento novo no logi-osd.
            let rec = Recorder::start(&bus);
            let got = next_call(&rec.calls).expect("o cartão pendente não chegou");
            assert_eq!((got.summary.as_str(), got.body.as_str()), ("MX Keys Mini: bateria baixa", "15%"));
            settle(Duration::from_millis(100));
            assert_eq!(n.pending(), None);
            // No máximo uma vez.
            assert!(drain_calls(&rec.calls, Duration::from_millis(200)).is_empty());
        }

        #[test]
        fn failed_pill_is_not_held() {
            let bus = PrivateBus::start();
            let n = Notifier::new(Some(bus.address.clone()));
            n.send(&message::lock(LockKey::Caps, true, &D));
            wait_until(|| !n.logged().is_empty());
            assert_eq!(n.pending(), None);
            let rec = Recorder::start(&bus);
            assert!(drain_calls(&rec.calls, Duration::from_millis(300)).is_empty(), "a pílula não deve ser reenviada");
        }

        #[test]
        fn only_the_last_failed_card_is_resent() {
            let bus = PrivateBus::start();
            let n = Notifier::new(Some(bus.address.clone()));
            n.send(&low(15));
            n.send(&low(5));
            wait_until(|| n.pending().is_some());
            settle(Duration::from_millis(50));
            assert_eq!(n.pending(), Some(low(5)));
            let rec = Recorder::start(&bus);
            let calls = drain_calls(&rec.calls, Duration::from_millis(500));
            let bodies: Vec<&str> = calls.iter().map(|c| c.body.as_str()).collect();
            assert_eq!(bodies, vec!["5%"]);
        }

        #[test]
        fn pending_card_is_resent_on_next_successful_send_and_new_card_replaces_it() {
            let bus = PrivateBus::start();
            let n = Notifier::new(Some(bus.address.clone()));
            n.send(&low(15));
            wait_until(|| n.pending().is_some());
            assert_eq!(n.pending(), Some(low(15)));
            // Um cartão novo substitui o pendente, e sai com o daemon de volta uma vez só.
            n.send(&low(5));
            assert_eq!(n.pending(), None, "o cartão novo está em voo; o antigo foi substituído");
            wait_until(|| n.pending().is_some());
            assert_eq!(n.pending(), Some(low(5)));
            let rec = Recorder::start(&bus);
            let calls = drain_calls(&rec.calls, Duration::from_millis(500));
            assert_eq!(calls.iter().map(|c| c.body.as_str()).collect::<Vec<_>>(), vec!["5%"]);
        }

        #[test]
        fn pending_card_is_resent_after_a_successful_send() {
            let bus = PrivateBus::start();
            let n = Notifier::new(Some(bus.address.clone()));
            n.send(&low(15));
            wait_until(|| n.pending().is_some());
            assert_eq!(n.pending(), Some(low(15)));
            // O dono do nome muda e a conexão do teste não vê a troca: simula pelo envio normal.
            let rec = Recorder::start(&bus);
            n.send(&message::lock(LockKey::Caps, true, &D));
            let calls = drain_calls(&rec.calls, Duration::from_millis(500));
            let mut bodies: Vec<&str> = calls.iter().map(|c| c.body.as_str()).collect();
            bodies.sort_unstable();
            assert_eq!(bodies, vec!["15%", "Ativado"]);
            assert_eq!(n.pending(), None);
        }

        #[test]
        fn reconnects_after_bus_error() {
            let socket = std::env::temp_dir().join(format!("logi-osd-teste-{}.sock", std::process::id()));
            let mut bus = PrivateBus::start_at(&socket);
            let n = Notifier::new(Some(bus.address.clone()));
            {
                let rec = Recorder::start(&bus);
                n.send(&message::lock(LockKey::Caps, true, &D));
                assert!(next_call(&rec.calls).is_some(), "o primeiro Notify não chegou");
            }

            bus.kill();
            let t = Instant::now();
            n.send(&message::lock(LockKey::Caps, false, &D));
            assert!(t.elapsed() < FAST, "send com o barramento morto levou {:?}", t.elapsed());
            wait_until(|| !n.connected());
            assert!(!n.connected(), "a conexão morta não foi descartada");
            // Sem barramento: a tentativa de reconectar falha rápido, loga e segue.
            let t = Instant::now();
            n.send(&message::lock(LockKey::Caps, false, &D));
            assert!(t.elapsed() < FAST, "reconectar sem barramento levou {:?}", t.elapsed());
            wait_until(|| n.logged().iter().any(|k| k == KIND_CONNECT));
            assert!(!n.logged().is_empty());

            let bus = PrivateBus::start_at(&socket);
            let rec = Recorder::start(&bus);
            n.send(&message::lock(LockKey::Num, true, &D));
            let got = next_call(&rec.calls).expect("o Notify depois da volta do barramento não chegou");
            assert_eq!(got.summary, "Num Lock");
            wait_until(|| n.connected() && n.logged().is_empty());
            assert!(n.connected());
            assert!(n.logged().is_empty(), "o sucesso não limpou o log: {:?}", n.logged());
            drop(bus);
            let _ = std::fs::remove_file(&socket);
        }
    }
}
