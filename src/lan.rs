//! Steuerung vom Handy im selben WLAN (wie "Spotify Connect"). Gegenstück: Playtube für Android
//! ("PC steuern", PcRemote.kt).
//!
//! TCP (Standard-Port 47811), dasselbe Zeilenprotokoll wie remote.rs. Das Handy findet den PC per
//! UDP-Broadcast "PLAYTUBE_DISCOVER" an Port 47812 und bekommt
//! {"service": "playtube", "protocol": 1, "name": "...", "port": 47811} zurück.
//! Vor den eigentlichen Befehlen muss sich jede Verbindung ausweisen:
//!   -> {"id": 1, "cmd": "pair", "code": "123456", "device": "Pixel 8"}  <- {"ok": true, "token": "...", "name": "MEIN-PC"}
//!   -> {"id": 1, "cmd": "hello", "token": "..."}                        <- {"ok": true, "name": "MEIN-PC", "protocol": 1}
//! Schutz: standardmäßig aus; nur private/Link-Local-Adressen; Kopplung nur mit dem 6-stelligen Code
//! vom PC (2 Minuten gültig, höchstens 5 Fehlversuche); gespeichert wird nur ein SHA-256 des Tokens.

use crate::config::PairedDevice;
use crate::remote::{self, Dispatch, PROTOCOL_VERSION};
use rand::RngExt;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const DEFAULT_PORT: u16 = 47811;
pub const DISCOVERY_PORT: u16 = 47812;
const DISCOVERY_REQUEST: &[u8] = b"PLAYTUBE_DISCOVER";
pub const CODE_LIFETIME: Duration = Duration::from_secs(120);
const MAX_CODE_ATTEMPTS: u32 = 5;
const MAX_DEVICES: usize = 20;
const MAX_DEVICE_NAME: usize = 60;
const MAX_CLIENTS: usize = 8;

pub fn computer_name() -> String {
    std::env::var("COMPUTERNAME").ok().filter(|n| !n.is_empty()).unwrap_or_else(|| "Playtube".into())
}

pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// Nur Geräte im eigenen Netz dürfen verbinden (nicht über eine Port-Weiterleitung).
pub fn is_local_peer(ip: IpAddr) -> bool {
    let ip = match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(IpAddr::V6(v6)),
        other => other,
    };
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_loopback() || v4.is_link_local(),
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local(),
    }
}

/// IPv4-Adresse dieses PCs im lokalen Netz (zum Eintippen am Handy). Es wird nichts gesendet:
/// "connect" eines UDP-Sockets wählt nur die Netzwerkkarte aus.
pub fn local_addresses() -> Vec<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok();
    let addr = socket.and_then(|s| s.connect("192.0.2.1:9").ok().and_then(|_| s.local_addr().ok()));
    addr.map(|a| a.ip()).filter(|ip| is_local_peer(*ip) && !ip.is_loopback()).map(|ip| vec![ip.to_string()]).unwrap_or_default()
}

struct PairingState {
    devices: Vec<PairedDevice>,
    code: Option<String>,
    expires: Instant,
    attempts_left: u32,
}

/// Kopplungscode und gekoppelte Geräte, geteilt zwischen UI-Thread und Netzwerk-Tasks.
#[derive(Clone)]
pub struct Pairing {
    state: Arc<Mutex<PairingState>>,
    on_change: Arc<dyn Fn(Vec<PairedDevice>) + Send + Sync>,
}

impl Pairing {
    pub fn new(devices: Vec<PairedDevice>, on_change: impl Fn(Vec<PairedDevice>) + Send + Sync + 'static) -> Self {
        Self {
            state: Arc::new(Mutex::new(PairingState { devices, code: None, expires: Instant::now(), attempts_left: 0 })),
            on_change: Arc::new(on_change),
        }
    }

    pub fn new_code(&self) -> String {
        let code = format!("{:06}", rand::rng().random_range(0..1_000_000u32));
        let mut s = self.state.lock().unwrap();
        s.code = Some(code.clone());
        s.expires = Instant::now() + CODE_LIFETIME;
        s.attempts_left = MAX_CODE_ATTEMPTS;
        code
    }

    pub fn devices(&self) -> Vec<PairedDevice> {
        self.state.lock().unwrap().devices.clone()
    }

    /// Prüft den Code und koppelt das Gerät; liefert das neue Token.
    pub fn pair(&self, code: Option<&str>, device: Option<&str>) -> Result<String, String> {
        let devices = {
            let mut s = self.state.lock().unwrap();
            let active = s.code.is_some() && Instant::now() < s.expires && s.attempts_left > 0;
            if !active {
                return Err("Kein Kopplungscode aktiv - am PC in den Einstellungen \"Handy koppeln\" waehlen".into());
            }
            let expected = s.code.clone().unwrap_or_default();
            if !constant_eq(code.unwrap_or("").trim().as_bytes(), expected.as_bytes()) {
                s.attempts_left -= 1;
                if s.attempts_left == 0 {
                    s.code = None;
                }
                return Err("Falscher Code".into());
            }
            s.code = None; // Jeder Code koppelt genau ein Gerät.
            let name: String = device.unwrap_or("").split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_DEVICE_NAME).collect();
            let token = hex::encode(rand::rng().random::<[u8; 32]>());
            s.devices.push(PairedDevice { name: if name.is_empty() { "Handy".into() } else { name }, token_sha256: hash_token(&token) });
            let excess = s.devices.len().saturating_sub(MAX_DEVICES);
            s.devices.drain(..excess);
            (s.devices.clone(), token)
        };
        (self.on_change)(devices.0);
        Ok(devices.1)
    }

    pub fn device_for(&self, token: Option<&str>) -> Option<String> {
        let token = token.filter(|t| !t.is_empty())?;
        let digest = hash_token(token);
        let s = self.state.lock().unwrap();
        s.devices.iter().find(|d| constant_eq(d.token_sha256.as_bytes(), digest.as_bytes())).map(|d| d.name.clone())
    }

    pub fn forget_all(&self) {
        {
            let mut s = self.state.lock().unwrap();
            s.devices.clear();
            s.code = None;
        }
        (self.on_change)(Vec::new());
    }
}

fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Eine Verbindung: erst pair/hello, danach normale Fernsteuerungsbefehle.
pub struct Session {
    pairing: Pairing,
    dispatch: Dispatch,
    pub device: Option<String>,
}

impl Session {
    pub fn new(pairing: Pairing, dispatch: Dispatch) -> Self {
        Self { pairing, dispatch, device: None }
    }

    pub async fn handle_line(&mut self, line: &str) -> Vec<u8> {
        if self.device.is_some() {
            return remote::process_line(line, &self.dispatch).await;
        }
        let Some(data) = serde_json::from_str::<Value>(line).ok().filter(Value::is_object) else {
            return remote::error(&Value::Null, "ungueltiges JSON");
        };
        let id = data.get("id").filter(|v| v.is_number() || v.is_string()).cloned().unwrap_or(Value::Null);
        let hello = |extra: Option<(&str, String)>| {
            let mut map = Map::new();
            map.insert("ok".into(), Value::Bool(true));
            map.insert("name".into(), Value::String(computer_name()));
            map.insert("protocol".into(), json!(PROTOCOL_VERSION));
            if let Some((k, v)) = extra {
                map.insert(k.into(), Value::String(v));
            }
            remote::encode(&id, map)
        };
        match data.get("cmd").and_then(Value::as_str) {
            Some("pair") => match self.pairing.pair(data.get("code").and_then(Value::as_str), data.get("device").and_then(Value::as_str)) {
                Ok(token) => {
                    self.device = self.pairing.device_for(Some(&token));
                    hello(Some(("token", token)))
                }
                Err(e) => remote::error(&id, &e),
            },
            Some("hello") => {
                self.device = self.pairing.device_for(data.get("token").and_then(Value::as_str));
                if self.device.is_some() { hello(None) } else { remote::error(&id, "nicht gekoppelt") }
            }
            _ => remote::error(&id, "nicht gekoppelt"),
        }
    }
}

pub fn discovery_reply(port: u16) -> Vec<u8> {
    json!({"service": "playtube", "protocol": PROTOCOL_VERSION, "name": computer_name(), "port": port}).to_string().into_bytes()
}

/// Laufender Server (TCP + UDP-Suche); stoppt beim Drop.
pub struct LanServer {
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl Drop for LanServer {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

impl LanServer {
    /// Bindet die Ports sofort (Fehler = Port belegt) und startet die Tasks auf `runtime`.
    pub fn start(runtime: &tokio::runtime::Handle, port: u16, pairing: Pairing, dispatch: Dispatch) -> std::io::Result<Self> {
        let _guard = runtime.enter();
        let std_listener = std::net::TcpListener::bind(("0.0.0.0", port))?;
        std_listener.set_nonblocking(true)?;
        let listener = tokio::net::TcpListener::from_std(std_listener)?;
        let mut tasks = vec![runtime.spawn(accept_loop(listener, pairing, dispatch))];

        // Ohne Suche klappt die Verbindung trotzdem über die IP-Adresse von Hand.
        tasks.push(runtime.spawn(discovery_task(port, DISCOVERY_PORT)));
        Ok(Self { tasks })
    }
}

async fn accept_loop(listener: tokio::net::TcpListener, pairing: Pairing, dispatch: Dispatch) {
    let slots = Arc::new(tokio::sync::Semaphore::new(MAX_CLIENTS));
    loop {
        let Ok((stream, peer)) = listener.accept().await else { continue };
        if !is_local_peer(peer.ip()) {
            continue;
        }
        let Ok(permit) = slots.clone().try_acquire_owned() else { continue };
        let _ = stream.set_nodelay(true);
        let session = Arc::new(tokio::sync::Mutex::new(Session::new(pairing.clone(), dispatch.clone())));
        tokio::spawn(async move {
            remote::serve_lines(stream, |line| {
                let session = session.clone();
                async move { session.lock().await.handle_line(&line).await }
            })
            .await;
            drop(permit);
        });
    }
}

/// UDP-Socket für die Suche. Mit SO_REUSEADDR, damit ein noch nicht freigegebener Socket
/// (Neustart der Fernsteuerung, Update, zweite Instanz) das Binden nicht blockiert –
/// die Suche kommt per Broadcast und erreicht jeden gebundenen Socket.
fn bind_discovery(discovery_port: u16) -> std::io::Result<tokio::net::UdpSocket> {
    use socket2::{Domain, Protocol, Socket, Type};
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    socket.bind(&std::net::SocketAddr::from(([0, 0, 0, 0], discovery_port)).into())?;
    socket.set_nonblocking(true)?;
    tokio::net::UdpSocket::from_std(socket.into())
}

/// Bindet die Suche; ist der Port belegt, wird alle paar Sekunden erneut versucht (nur einmal geloggt).
async fn discovery_task(port: u16, discovery_port: u16) {
    let mut logged = false;
    loop {
        match bind_discovery(discovery_port) {
            Ok(udp) => {
                if logged {
                    crate::log::line("lan: Suche doch noch gestartet");
                }
                return discovery_loop(udp, port).await;
            }
            Err(e) => {
                if !logged {
                    crate::log::line(&format!("lan: Suche noch nicht verfuegbar, versuche es weiter: {e}"));
                    logged = true;
                }
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }
}

async fn discovery_loop(udp: tokio::net::UdpSocket, port: u16) {
    let mut buffer = [0u8; 512];
    loop {
        let Ok((len, from)) = udp.recv_from(&mut buffer).await else { continue };
        if buffer[..len].trim_ascii() == DISCOVERY_REQUEST && is_local_peer(from.ip()) {
            let _ = udp.send_to(&discovery_reply(port), from).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::Action;

    /// Port schon belegt (alter Socket, andere Instanz): die Suche bindet trotzdem und antwortet.
    #[test]
    fn discovery_works_while_port_is_taken() {
        let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            // Eigener Port, damit eine laufende Playtube den Test nicht stört.
            const TEST_PORT: u16 = 47_899;
            let _stale = bind_discovery(TEST_PORT).expect("alter Socket");
            let task = tokio::spawn(discovery_task(59_871, TEST_PORT));
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            let client = tokio::net::UdpSocket::bind("0.0.0.0:0").await.unwrap();
            client.set_broadcast(true).unwrap();
            let mut buffer = [0u8; 512];
            let mut found = false;
            for _ in 0..10 {
                client.send_to(DISCOVERY_REQUEST, ("255.255.255.255", TEST_PORT)).await.unwrap();
                let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(300);
                while let Ok(Ok((len, _))) = tokio::time::timeout_at(deadline, client.recv_from(&mut buffer)).await {
                    if String::from_utf8_lossy(&buffer[..len]).contains("59871") {
                        found = true;
                        break;
                    }
                }
                if found {
                    break;
                }
            }
            task.abort();
            assert!(found, "keine Antwort der Suche");
        });
    }

    fn pairing() -> (Pairing, Arc<Mutex<Vec<Vec<PairedDevice>>>>) {
        let changes = Arc::new(Mutex::new(Vec::new()));
        let log = changes.clone();
        (Pairing::new(Vec::new(), move |d| log.lock().unwrap().push(d)), changes)
    }

    fn dispatch(log: Arc<Mutex<Vec<Action>>>) -> Dispatch {
        Arc::new(move |action, reply| {
            log.lock().unwrap().push(action);
            reply(Value::Null)
        })
    }

    #[test]
    fn code_is_six_digits_and_couples_once() {
        let (p, changes) = pairing();
        let code = p.new_code();
        assert!(code.len() == 6 && code.bytes().all(|b| b.is_ascii_digit()));
        let token = p.pair(Some(&code), Some("  Pixel   8 ")).unwrap();
        let devices = p.devices();
        assert_eq!(devices[0].name, "Pixel 8");
        assert_eq!(devices[0].token_sha256, hash_token(&token));
        assert!(!serde_json::to_string(&devices).unwrap().contains(&token));
        assert_eq!(p.device_for(Some(&token)).as_deref(), Some("Pixel 8"));
        assert!(p.pair(Some(&code), Some("Zweites")).is_err());
        assert_eq!(changes.lock().unwrap().len(), 1);
    }

    #[test]
    fn no_pairing_without_code_and_too_many_guesses_drop_it() {
        let (p, _) = pairing();
        assert!(p.pair(Some("000000"), None).is_err());
        let code = p.new_code();
        let wrong = if code == "000000" { "111111" } else { "000000" };
        for _ in 0..MAX_CODE_ATTEMPTS {
            assert!(p.pair(Some(wrong), None).is_err());
        }
        assert!(p.pair(Some(&code), None).is_err());
        assert!(p.devices().is_empty());
    }

    #[test]
    fn unknown_tokens_and_forget_all() {
        let (p, changes) = pairing();
        p.pair(Some(&p.new_code()), Some("Pixel")).unwrap();
        for token in [None, Some(""), Some("falsch")] {
            assert!(p.device_for(token).is_none());
        }
        p.forget_all();
        assert!(p.devices().is_empty());
        assert_eq!(changes.lock().unwrap().len(), 2);
    }

    #[test]
    fn accepts_only_local_networks() {
        for a in ["192.168.1.20", "10.0.0.5", "172.16.3.4", "127.0.0.1", "fe80::1", "::ffff:192.168.1.20", "169.254.1.1", "fd00::1"] {
            assert!(is_local_peer(a.parse().unwrap()), "{a}");
        }
        for a in ["8.8.8.8", "2001:4860:4860::8888", "::ffff:8.8.8.8"] {
            assert!(!is_local_peer(a.parse().unwrap()), "{a}");
        }
    }

    #[tokio::test]
    async fn session_needs_pairing_first() {
        let (p, _) = pairing();
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut s = Session::new(p.clone(), dispatch(log.clone()));
        let r: Value = serde_json::from_slice(&s.handle_line(r#"{"id": 1, "cmd": "next"}"#).await).unwrap();
        assert_eq!(r["error"], "nicht gekoppelt");
        assert!(log.lock().unwrap().is_empty());

        let code = p.new_code();
        let r: Value = serde_json::from_slice(&s.handle_line(&json!({"id": 2, "cmd": "pair", "code": code, "device": "Pixel"}).to_string()).await).unwrap();
        assert_eq!(r["ok"], true);
        let token = r["token"].as_str().unwrap().to_string();
        s.handle_line(r#"{"id": 3, "cmd": "next"}"#).await;
        assert_eq!(log.lock().unwrap().len(), 1);

        let mut again = Session::new(p, dispatch(log));
        let r: Value = serde_json::from_slice(&again.handle_line(&json!({"cmd": "hello", "token": token}).to_string()).await).unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(again.device.as_deref(), Some("Pixel"));
    }

    #[tokio::test]
    async fn pair_and_command_over_tcp() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let (p, _) = pairing();
        let log = Arc::new(Mutex::new(Vec::new()));
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let server = LanServer::start(&tokio::runtime::Handle::current(), port, p.clone(), dispatch(log.clone())).unwrap();
        let code = p.new_code();
        let stream = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let (read, mut write) = stream.into_split();
        write.write_all(format!("{}\n{{\"id\": 2, \"cmd\": \"next\"}}\n", json!({"id": 1, "cmd": "pair", "code": code, "device": "Test"})).as_bytes()).await.unwrap();
        let mut lines = BufReader::new(read).lines();
        let first: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        let second: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(first["ok"], true);
        assert_eq!(second, json!({"id": 2, "ok": true}));
        assert_eq!(p.devices().len(), 1);
        drop(server);
    }
}
