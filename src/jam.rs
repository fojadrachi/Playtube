//! Playtube Jam: gemeinsam hören wie bei Spotify Jam. Ein eigener Dienst (Cloudflare Worker +
//! Durable Object, Ordner `playtube-jam`) hält pro Jam Warteschlange und Wiedergabestand; jedes
//! Gerät spielt selbst ab und gleicht sich an. Diese Datei enthält das Protokoll, die reine
//! Sync-Logik (getestet) und die WebSocket-Verbindung im tokio-Runtime.
//!
//! Dieselben Regeln gelten im Web-Client (`playtube-jam/public/sync.js`) und in der Android-App.

use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

pub const DEFAULT_SERVER: &str = "https://jam.fojadrachi.de";
/// Ab dieser Abweichung (Sekunden) wird gespult. Etwas großzügiger als im Browser, weil die
/// Position über ein Skript im WebView gelesen wird.
pub const TOLERANCE_S: f64 = 1.5;
const CODE_ALPHABET: &str = "ABCDEFGHJKMNPQRSTUVWXYZ23456789";

// ---------------------------------------------------------------------------- Protokoll

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    pub video_id: String,
    pub title: String,
    pub artist: String,
    pub thumb: String,
    pub dur: Option<f64>,
    pub by: String,
    pub by_name: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Member {
    pub id: String,
    pub name: String,
    pub client: String,
    pub online: bool,
    pub host: bool,
    pub cant_play: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Playback {
    pub paused: bool,
    pub position: f64,
    #[serde(rename = "ref")]
    pub reference: f64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Features {
    pub search: bool,
}

/// Zustand eines Jams aus Sicht dieses Teilnehmers (Nachricht `state` des Servers).
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct JamState {
    pub code: String,
    pub seq: u64,
    pub you: String,
    pub host_id: String,
    pub all_control: bool,
    pub can_control: bool,
    pub features: Features,
    pub members: Vec<Member>,
    pub current: Option<Item>,
    pub queue: Vec<Item>,
    pub history: Vec<Item>,
    pub playback: Playback,
    pub server_now: f64,
}

impl JamState {
    pub fn is_host(&self) -> bool {
        self.you == self.host_id
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Created {
    pub code: String,
    pub token: String,
    pub member_id: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct SearchResult {
    pub video_id: String,
    pub title: String,
    pub artist: String,
    pub thumb: String,
}

// ---------------------------------------------------------------------------- Codes und Links

/// „abc-234“, „ABC 234“ oder ein Einladungslink (`https://…/ABC234`, `playtube://jam/ABC234`)
/// → „ABC234“.
pub fn normalize_code(input: &str) -> Option<String> {
    let mut text = input.trim().split(['?', '#']).next().unwrap_or("").trim_end_matches('/');
    if let Some(pos) = text.rfind('/') {
        text = &text[pos + 1..];
    }
    let code: String = text.chars().filter(|c| !c.is_whitespace() && *c != '-').collect::<String>().to_uppercase();
    (code.chars().count() == 6 && code.chars().all(|c| CODE_ALPHABET.contains(c))).then_some(code)
}

/// Aufrufparameter → Jam-Code: `playtube://jam/ABC234` (URL-Protokoll) oder `--jam ABC234`.
pub fn code_from_args(args: &[String]) -> Option<String> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--jam" {
            return iter.next().and_then(|c| normalize_code(c));
        }
        let lower = arg.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("playtube://jam/").or_else(|| lower.strip_prefix("playtube:jam/")) {
            return normalize_code(rest);
        }
    }
    None
}

fn valid_video_id(id: &str) -> bool {
    id.len() == 11 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Video-ID aus einer ID oder einem YouTube-/YouTube-Music-Link.
pub fn parse_video_id(input: &str) -> Option<String> {
    let text = input.trim();
    if valid_video_id(text) {
        return Some(text.to_string());
    }
    let rest = text.split_once("://").map(|(_, r)| r).unwrap_or(text);
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host = host.to_ascii_lowercase();
    let host = host.trim_start_matches("www.").trim_start_matches("m.").trim_start_matches("music.");
    let (path, query) = path.split_once('?').unwrap_or((path, ""));
    let query = query.split('#').next().unwrap_or("");
    let id = match host {
        "youtu.be" => path.split('/').next().map(str::to_string),
        "youtube.com" | "youtube-nocookie.com" => query
            .split('&')
            .find_map(|pair| pair.strip_prefix("v=").map(str::to_string))
            .or_else(|| {
                let mut parts = path.split('/');
                match parts.next() {
                    Some("shorts" | "embed" | "live" | "v") => parts.next().map(str::to_string),
                    _ => None,
                }
            }),
        _ => None,
    }?;
    valid_video_id(&id).then_some(id)
}

/// Playlist-/Album-ID aus einer ID oder einem Link (`list=`).
pub fn parse_playlist_id(input: &str) -> Option<String> {
    let text = input.trim();
    let id = if text.contains("://") || text.starts_with("youtube.com") || text.contains("youtube.com/") {
        let (host, rest) = text.split_once("://").map(|(_, r)| r).unwrap_or(text).split_once('/').unwrap_or((text, ""));
        let host = host.to_ascii_lowercase();
        let host = host.trim_start_matches("www.").trim_start_matches("m.").trim_start_matches("music.");
        if host != "youtube.com" && host != "youtu.be" {
            return None;
        }
        rest.split_once('?')?.1.split('&').find_map(|p| p.strip_prefix("list="))?.split('#').next()?.to_string()
    } else {
        text.to_string()
    };
    let id = id.strip_prefix("VL").map(str::to_string).unwrap_or(id);
    let prefix_ok = ["PL", "OLAK5uy_", "RD", "LL", "UU", "FL", "OL", "LM"].iter().any(|p| id.starts_with(p));
    let chars_ok = id.len() <= 90 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    (prefix_ok && chars_ok).then_some(id)
}

/// "3:45" bzw. "1:02:03" -> Sekunden.
pub fn parse_duration(text: &str) -> Option<f64> {
    let parts: Vec<&str> = text.trim().split(':').collect();
    if !(2..=3).contains(&parts.len()) || parts.iter().any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit())) {
        return None;
    }
    Some(parts.iter().fold(0.0, |sum, p| sum * 60.0 + p.parse::<f64>().unwrap_or(0.0)))
}

/// Video-ID aus der URL des YouTube-Music-Tabs (`…/watch?v=…`).
pub fn video_id_from_url(url: &str) -> Option<String> {
    if !url.contains("/watch") {
        return None;
    }
    parse_video_id(url)
}

pub fn invite_url(server: &str, code: &str) -> String {
    format!("{}/{code}", server.trim_end_matches('/'))
}

pub fn pretty_code(code: &str) -> String {
    if code.len() == 6 { format!("{} {}", &code[..3], &code[3..]) } else { code.to_string() }
}

pub fn format_time(seconds: f64) -> String {
    let total = if seconds.is_finite() && seconds > 0.0 { seconds as u64 } else { 0 };
    let (h, m, s) = (total / 3600, (total / 60) % 60, total % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

// ---------------------------------------------------------------------------- Zeit und Sync

/// Millisekunden seit 1970 (lokale Uhr).
pub fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0)
}

/// Zeitabgleich mit dem Server über ping/pong: behält die Messung mit der kürzesten Laufzeit.
#[derive(Clone, Debug, Default)]
pub struct Clock {
    samples: Vec<(f64, f64)>,
    pub offset: f64,
}

impl Clock {
    /// `c` eigene Sendezeit, `s` Serverzeit, `r` eigene Empfangszeit (ms).
    pub fn add(&mut self, c: f64, s: f64, r: f64) {
        let rtt = r - c;
        if !(0.0..=30_000.0).contains(&rtt) {
            return;
        }
        self.samples.push((rtt, s - (c + rtt / 2.0)));
        if self.samples.len() > 10 {
            self.samples.remove(0);
        }
        if let Some(best) = self.samples.iter().min_by(|a, b| a.0.total_cmp(&b.0)) {
            self.offset = best.1;
        }
    }

    /// Ohne Messung: grobe Schätzung aus der Serverzeit eines Zustands.
    pub fn hint(&mut self, server_now: f64, local_now: f64) {
        if self.samples.is_empty() {
            self.offset = server_now - local_now;
        }
    }

    pub fn server_now(&self, local_now: f64) -> f64 {
        local_now + self.offset
    }
}

pub fn position_at(playback: &Playback, server_now: f64) -> f64 {
    if playback.paused { playback.position } else { playback.position + (server_now - playback.reference) / 1000.0 }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub video_id: String,
    pub item_id: String,
    pub pos: f64,
    pub paused: bool,
    pub dur: Option<f64>,
}

pub fn target(state: &JamState, server_now: f64) -> Option<Target> {
    let item = state.current.as_ref()?;
    Some(Target {
        video_id: item.video_id.clone(),
        item_id: item.id.clone(),
        pos: position_at(&state.playback, server_now),
        paused: state.playback.paused,
        dur: item.dur,
    })
}

/// Stand des eigenen Players.
#[derive(Clone, Debug, PartialEq)]
pub struct Local {
    pub video_id: Option<String>,
    pub pos: f64,
    pub paused: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Load { video_id: String, pos: f64 },
    Seek(f64),
    Play,
    Pause,
}

/// Was der eigene Player tun soll, damit er zum Jam passt.
pub fn decide(want: Option<&Target>, local: &Local, tolerance: f64) -> Vec<Step> {
    let Some(want) = want else {
        return if local.video_id.is_some() && !local.paused { vec![Step::Pause] } else { Vec::new() };
    };
    let near_end = want.dur.is_some_and(|d| want.pos > d - 3.0);
    if local.video_id.as_deref() != Some(want.video_id.as_str()) {
        // Am Titelende wechselt YouTube Music von selbst zum nächsten (Autoplay) - dann nicht den
        // alten Titel neu laden, sondern auf den Wechsel im Jam warten.
        if near_end {
            return if local.paused { Vec::new() } else { vec![Step::Pause] };
        }
        let mut steps = vec![Step::Load { video_id: want.video_id.clone(), pos: want.pos.max(0.0) }];
        if want.paused || want.pos < 0.0 {
            steps.push(Step::Pause);
        }
        return steps;
    }
    let mut steps = Vec::new();
    if want.pos < 0.0 {
        // Titel startet gleich: am Anfang warten.
        if !local.paused {
            steps.push(Step::Pause);
        }
        if local.pos > 0.5 {
            steps.push(Step::Seek(0.0));
        }
        return steps;
    }
    if (local.pos - want.pos).abs() > tolerance && !(near_end && local.pos > want.pos) {
        steps.push(Step::Seek(want.pos));
    }
    if want.paused && !local.paused {
        steps.push(Step::Pause);
    }
    if !want.paused && local.paused {
        steps.push(Step::Play);
    }
    steps
}

/// Hat man im YouTube-Music-Tab selbst einen anderen Song gestartet? Wie bei Spotify Jam kommt er
/// dann in den Jam (und der Tab spielt danach wieder den Jam-Song). `baseline` ist der Song, der
/// schon beim Beitreten lief - der zählt nicht als Wunsch. Am Titelende springt YouTube Music von
/// selbst weiter (Autoplay) - das ist auch kein Wunsch.
pub fn user_pick(state: &JamState, want: Option<&Target>, local: &Local, baseline: Option<&str>) -> Option<String> {
    let id = local.video_id.as_deref()?;
    if local.paused || baseline == Some(id) {
        return None;
    }
    if let Some(w) = want {
        if w.video_id == id || w.pos < 0.0 || w.dur.is_some_and(|d| w.pos > d - 3.0) {
            return None;
        }
    }
    let known = state.current.iter().chain(&state.queue).chain(&state.history).any(|i| i.video_id == id);
    (!known && valid_video_id(id)).then(|| id.to_string())
}

/// Hat der Player den Titel zu Ende gespielt? (für die Meldung `ended`)
pub fn reached_end(local: &Local, duration: f64, ended: bool) -> bool {
    ended || (duration > 0.0 && local.pos >= duration - 0.6)
}

// ---------------------------------------------------------------------------- Netzwerk

/// Host und Port aus der Server-Adresse (`https://jam.fojadrachi.de` -> ("jam.fojadrachi.de", 443)).
pub fn host_port(server: &str) -> Option<(String, u16)> {
    let (default_port, rest) = if let Some(rest) = server.strip_prefix("https://") {
        (443, rest)
    } else if let Some(rest) = server.strip_prefix("http://") {
        (80, rest)
    } else if let Some(rest) = server.strip_prefix("wss://") {
        (443, rest)
    } else if let Some(rest) = server.strip_prefix("ws://") {
        (80, rest)
    } else {
        (443, server)
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() || authority.starts_with('[') {
        return None;
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (host, port.parse().ok()?),
        None => (authority, default_port),
    };
    Some((host.to_ascii_lowercase(), port))
}

/// Antwort von DNS-over-HTTPS (JSON-Format von Cloudflare/Google) -> erste IPv4-Adresse.
pub fn parse_doh_answer(body: &Value) -> Option<std::net::IpAddr> {
    body["Answer"]
        .as_array()?
        .iter()
        .filter(|a| a["type"].as_u64() == Some(1))
        .find_map(|a| a["data"].as_str()?.parse::<std::net::Ipv4Addr>().ok())
        .map(std::net::IpAddr::V4)
}

/// Adresse über DNS-over-HTTPS (Cloudflare, sonst Google) - für Router, die einen neuen
/// Eintrag noch nicht kennen und "gibt es nicht" zwischenspeichern. Die Abfrage geht direkt an
/// eine IP-Adresse, braucht also selbst kein DNS.
async fn resolve_doh(host: &str) -> Option<std::net::IpAddr> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(5)).build().ok()?;
    for base in ["https://1.1.1.1/dns-query", "https://1.0.0.1/dns-query", "https://8.8.8.8/resolve"] {
        let response = client
            .get(base)
            .query(&[("name", host), ("type", "A")])
            .header("accept", "application/dns-json")
            .send()
            .await;
        if let Ok(response) = response {
            if let Ok(body) = response.json::<Value>().await {
                if let Some(ip) = parse_doh_answer(&body) {
                    return Some(ip);
                }
            }
        }
    }
    None
}

/// Feste Adresse für den Jam-Server, falls das normale DNS ihn nicht findet; `None` = normales
/// DNS funktioniert (oder auch DoH kennt ihn nicht). Ergebnis wird für die Laufzeit gemerkt.
async fn dns_override(server: &str) -> Option<std::net::SocketAddr> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<String, std::net::SocketAddr>>> = OnceLock::new();
    let (host, port) = host_port(server)?;
    if host.parse::<std::net::IpAddr>().is_ok() {
        return None;
    }
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(addr) = cache.lock().ok()?.get(&host).copied() {
        return Some(addr);
    }
    if tokio::net::lookup_host((host.as_str(), port)).await.is_ok_and(|mut a| a.next().is_some()) {
        return None;
    }
    let ip = resolve_doh(&host).await?;
    let addr = std::net::SocketAddr::new(ip, port);
    crate::log::line(&format!("jam: {host} per normalem DNS nicht gefunden, nutze {ip} (DNS-over-HTTPS)"));
    cache.lock().ok()?.insert(host, addr);
    Some(addr)
}

pub(crate) async fn http_client(server: &str) -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .user_agent(format!("Playtube/{}", crate::config::VERSION))
        .timeout(Duration::from_secs(10));
    if let (Some(addr), Some((host, _))) = (dns_override(server).await, host_port(server)) {
        builder = builder.resolve(&host, addr);
    }
    builder.build().unwrap_or_default()
}

type Socket = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// WebSocket öffnen - bei DNS-Problemen über die per DoH gefundene Adresse.
async fn connect_ws(url: &str) -> Result<Socket, tokio_tungstenite::tungstenite::Error> {
    if let Some(addr) = dns_override(url).await {
        let stream = tokio::net::TcpStream::connect(addr).await?;
        let (socket, _) = tokio_tungstenite::client_async_tls(url, stream).await?;
        return Ok(socket);
    }
    let (socket, _) = tokio_tungstenite::connect_async(url).await?;
    Ok(socket)
}

/// Neuen Jam anlegen (man ist dann Host).
pub async fn create(server: &str, name: &str) -> anyhow::Result<Created> {
    let url = format!("{}/api/jam", server.trim_end_matches('/'));
    let res = http_client(server).await.post(url).json(&json!({ "name": name, "client": "pc" })).send().await?;
    anyhow::ensure!(res.status().is_success(), "HTTP {}", res.status());
    Ok(res.json().await?)
}

/// Gibt es den Jam? (`false` = unbekannt oder abgelaufen)
pub async fn exists(server: &str, code: &str) -> anyhow::Result<bool> {
    let url = format!("{}/api/jam/{code}", server.trim_end_matches('/'));
    let res = http_client(server).await.get(url).send().await?;
    if res.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(false);
    }
    anyhow::ensure!(res.status().is_success(), "HTTP {}", res.status());
    Ok(true)
}

pub async fn search(server: &str, query: &str) -> anyhow::Result<Vec<SearchResult>> {
    #[derive(Deserialize)]
    struct Results {
        results: Vec<SearchResult>,
    }
    let url = format!("{}/api/search", server.trim_end_matches('/'));
    let res = http_client(server).await.get(url).query(&[("q", query)]).send().await?;
    anyhow::ensure!(res.status().is_success(), "HTTP {}", res.status());
    Ok(res.json::<Results>().await?.results)
}

/// Ereignisse der Verbindung (werden im tokio-Runtime erzeugt).
#[derive(Debug)]
pub enum Event {
    Connected,
    Disconnected,
    Welcome { token: String },
    State(Box<JamState>),
    Pong { c: f64, s: f64, r: f64 },
    /// Jam vorbei: "host_ended", "host_left", "expired", "kicked", "left", "not_found"
    Ended(String),
    Error(String),
}

/// Laufende Verbindung zu einem Jam; baut sie bei Abbrüchen selbst wieder auf.
pub struct Connection {
    tx: mpsc::UnboundedSender<Value>,
    task: tokio::task::JoinHandle<()>,
}

impl Connection {
    pub fn start(
        runtime: &tokio::runtime::Handle,
        server: &str,
        code: &str,
        name: &str,
        token: Option<String>,
        on_event: impl Fn(Event) + Send + Sync + 'static,
    ) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let url = format!("{}/api/jam/{code}/ws", ws_base(server));
        let task = runtime.spawn(run(url, name.to_string(), token, rx, Arc::new(on_event)));
        Self { tx, task }
    }

    pub fn send(&self, msg: Value) {
        let _ = self.tx.send(msg);
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn ws_base(server: &str) -> String {
    let server = server.trim_end_matches('/');
    if let Some(rest) = server.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = server.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        format!("wss://{server}")
    }
}

async fn run(
    url: String,
    name: String,
    mut token: Option<String>,
    mut rx: mpsc::UnboundedReceiver<Value>,
    on_event: Arc<dyn Fn(Event) + Send + Sync>,
) {
    let mut delay = Duration::from_secs(1);
    loop {
        match connect_ws(&url).await {
            Ok(stream) => {
                delay = Duration::from_secs(1);
                let (mut write, mut read) = stream.split();
                let hello = json!({ "t": "hello", "name": name, "client": "pc", "token": token });
                if write.send(Message::text(hello.to_string())).await.is_err() {
                    continue;
                }
                on_event(Event::Connected);
                let mut pings = 0u32;
                let mut next_ping = tokio::time::Instant::now();
                let finished = loop {
                    tokio::select! {
                        incoming = read.next() => {
                            let Some(Ok(message)) = incoming else { break false };
                            let Message::Text(text) = message else { continue };
                            let Ok(value) = serde_json::from_str::<Value>(text.as_str()) else { continue };
                            match value["t"].as_str().unwrap_or("") {
                                "pong" => on_event(Event::Pong {
                                    c: value["c"].as_f64().unwrap_or(0.0),
                                    s: value["s"].as_f64().unwrap_or(0.0),
                                    r: now_ms(),
                                }),
                                "welcome" => {
                                    token = value["token"].as_str().map(str::to_string);
                                    on_event(Event::Welcome { token: token.clone().unwrap_or_default() });
                                }
                                "state" => {
                                    if let Ok(state) = serde_json::from_value::<JamState>(value) {
                                        on_event(Event::State(Box::new(state)));
                                    }
                                }
                                "ended" => {
                                    on_event(Event::Ended(value["reason"].as_str().unwrap_or("ended").to_string()));
                                    break true;
                                }
                                "error" => {
                                    let error = value["error"].as_str().unwrap_or("error").to_string();
                                    if matches!(error.as_str(), "not_found" | "kicked" | "full") {
                                        on_event(Event::Ended(error));
                                        break true;
                                    }
                                    on_event(Event::Error(error));
                                }
                                _ => {}
                            }
                        }
                        outgoing = rx.recv() => {
                            let Some(msg) = outgoing else { return };
                            if write.send(Message::text(msg.to_string())).await.is_err() {
                                break false;
                            }
                        }
                        _ = tokio::time::sleep_until(next_ping) => {
                            // Anfangs schnell hintereinander (Zeitabgleich), danach alle 15 s.
                            pings += 1;
                            next_ping = tokio::time::Instant::now() + if pings < 6 { Duration::from_millis(800) } else { Duration::from_secs(15) };
                            let ping = json!({ "t": "ping", "c": now_ms() });
                            if write.send(Message::text(ping.to_string())).await.is_err() {
                                break false;
                            }
                        }
                    }
                };
                if finished {
                    let _ = write.close().await;
                    return;
                }
                on_event(Event::Disconnected);
            }
            Err(e) => {
                crate::log::line(&format!("jam: Verbindung fehlgeschlagen: {e}"));
                on_event(Event::Disconnected);
            }
        }
        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(Duration::from_secs(20));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_codes_and_links() {
        assert_eq!(normalize_code("abc-234").as_deref(), Some("ABC234"));
        assert_eq!(normalize_code(" ABC 234 ").as_deref(), Some("ABC234"));
        assert_eq!(normalize_code("https://jam.fojadrachi.de/abc234").as_deref(), Some("ABC234"));
        assert_eq!(normalize_code("https://jam.fojadrachi.de/abc234/?x=1").as_deref(), Some("ABC234"));
        assert_eq!(normalize_code("ABC10O"), None);
        assert_eq!(normalize_code("ABC23"), None);
        assert_eq!(normalize_code("ÄBC234"), None);
    }

    #[test]
    fn reads_deep_links_from_arguments() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(code_from_args(&args(&["Playtube.exe", "playtube://jam/abc234"])).as_deref(), Some("ABC234"));
        assert_eq!(code_from_args(&args(&["Playtube.exe", "playtube://jam/ABC234/"])).as_deref(), Some("ABC234"));
        assert_eq!(code_from_args(&args(&["Playtube.exe", "--jam", "abc-234"])).as_deref(), Some("ABC234"));
        assert_eq!(code_from_args(&args(&["Playtube.exe"])), None);
        assert_eq!(code_from_args(&args(&["Playtube.exe", "playtube://anderes/ABC234"])), None);
    }

    #[test]
    fn parses_video_ids() {
        let id = "dQw4w9WgXcQ";
        for input in [
            id.to_string(),
            format!("https://www.youtube.com/watch?v={id}&list=RD"),
            format!("https://music.youtube.com/watch?v={id}"),
            format!("https://youtu.be/{id}?si=x"),
            format!("youtube.com/shorts/{id}"),
            format!("https://m.youtube.com/watch?feature=share&v={id}"),
        ] {
            assert_eq!(parse_video_id(&input).as_deref(), Some(id), "{input}");
        }
        assert_eq!(parse_video_id("https://evil.example/watch?v=dQw4w9WgXcQ"), None);
        assert_eq!(parse_video_id("kurz"), None);
        assert_eq!(video_id_from_url("https://music.youtube.com/").as_deref(), None);
        assert_eq!(video_id_from_url(&format!("https://music.youtube.com/watch?v={id}&list=x")).as_deref(), Some(id));
    }

    #[test]
    fn parses_playlist_links_and_durations() {
        assert_eq!(parse_playlist_id("https://music.youtube.com/playlist?list=PLabc_1").as_deref(), Some("PLabc_1"));
        assert_eq!(parse_playlist_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=RDxyz").as_deref(), Some("RDxyz"));
        assert_eq!(parse_playlist_id("OLAK5uy_abc").as_deref(), Some("OLAK5uy_abc"));
        assert_eq!(parse_playlist_id("VLPLabc").as_deref(), Some("PLabc"));
        assert_eq!(parse_playlist_id("https://evil.example/playlist?list=PLabc"), None);
        assert_eq!(parse_playlist_id("queen"), None);
        assert_eq!(parse_duration("3:45"), Some(225.0));
        assert_eq!(parse_duration("1:02:03"), Some(3723.0));
        assert_eq!(parse_duration("3"), None);
        assert_eq!(parse_duration("a:bc"), None);
    }

    #[test]
    fn formats() {
        assert_eq!(format_time(0.0), "0:00");
        assert_eq!(format_time(65.9), "1:05");
        assert_eq!(format_time(3725.0), "1:02:05");
        assert_eq!(format_time(f64::NAN), "0:00");
        assert_eq!(pretty_code("ABC234"), "ABC 234");
        assert_eq!(invite_url("https://jam.fojadrachi.de/", "ABC234"), "https://jam.fojadrachi.de/ABC234");
        assert_eq!(ws_base("https://jam.fojadrachi.de"), "wss://jam.fojadrachi.de");
        assert_eq!(ws_base("http://127.0.0.1:8787/"), "ws://127.0.0.1:8787");
    }

    #[test]
    fn splits_host_and_port() {
        assert_eq!(host_port("https://jam.fojadrachi.de"), Some(("jam.fojadrachi.de".into(), 443)));
        assert_eq!(host_port("wss://Jam.Fojadrachi.de/api/jam/ABC234/ws"), Some(("jam.fojadrachi.de".into(), 443)));
        assert_eq!(host_port("http://127.0.0.1:8787/"), Some(("127.0.0.1".into(), 8787)));
        assert_eq!(host_port("ws://localhost:8787/x"), Some(("localhost".into(), 8787)));
        assert_eq!(host_port("https://"), None);
    }

    #[test]
    fn reads_dns_over_https_answers() {
        let body = json!({"Status": 0, "Answer": [
            {"name": "jam.fojadrachi.de", "type": 5, "data": "x.example."},
            {"name": "jam.fojadrachi.de", "type": 1, "data": "188.114.96.0"}
        ]});
        assert_eq!(parse_doh_answer(&body), Some("188.114.96.0".parse().unwrap()));
        assert_eq!(parse_doh_answer(&json!({"Status": 3})), None);
        assert_eq!(parse_doh_answer(&json!({"Answer": [{"type": 1, "data": "kaputt"}]})), None);
    }

    /// Echter Netzwerktest des Ausweichwegs (nicht in der normalen Testrunde):
    /// `cargo test doh_fallback -- --ignored`
    #[test]
    #[ignore]
    fn doh_fallback_reaches_the_jam_server() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let ip = resolve_doh("jam.fojadrachi.de").await.expect("DoH liefert keine Adresse");
            let stream = tokio::net::TcpStream::connect((ip, 443)).await.unwrap();
            let (mut socket, _) = tokio_tungstenite::client_async_tls("wss://jam.fojadrachi.de/api/jam/ZZZZZZ/ws", stream).await.unwrap();
            let first = socket.next().await.unwrap().unwrap();
            assert!(first.to_string().contains("not_found"), "{first}");
        });
    }

    #[test]
    fn clock_takes_fastest_sample() {
        let mut clock = Clock::default();
        clock.hint(9000.0, 1000.0);
        assert_eq!(clock.offset, 8000.0);
        clock.add(1000.0, 6050.0, 1100.0);
        clock.add(2000.0, 7900.0, 2800.0);
        clock.add(3000.0, 8010.0, 3020.0);
        assert_eq!(clock.offset, 5000.0);
        clock.add(5000.0, 0.0, 4000.0); // unsinnig
        assert_eq!(clock.offset, 5000.0);
        assert_eq!(clock.server_now(10_000.0), 15_000.0);
    }

    #[test]
    fn reads_server_state() {
        let text = r#"{"t":"state","code":"ABC234","seq":3,"you":"m2","hostId":"m1","allControl":false,
            "canControl":false,"features":{"search":true},
            "members":[{"id":"m1","name":"Chris","client":"pc","online":true,"host":true,"cantPlay":false}],
            "current":{"id":"i1","videoId":"dQw4w9WgXcQ","title":"T","artist":"A","thumb":"x","dur":212,"by":"m1","byName":"Chris"},
            "queue":[],"history":[],"playback":{"paused":false,"position":10,"ref":1000},"serverNow":6000,"neu":1}"#;
        let state: JamState = serde_json::from_str(text).unwrap();
        assert!(!state.is_host());
        assert!(state.features.search);
        assert_eq!(state.current.as_ref().unwrap().dur, Some(212.0));
        let want = target(&state, 6000.0).unwrap();
        assert_eq!(want.pos, 15.0);
        assert_eq!(want.video_id, "dQw4w9WgXcQ");
    }

    fn want(pos: f64, paused: bool) -> Target {
        Target { video_id: "aaaaaaaaaaa".into(), item_id: "i".into(), pos, paused, dur: Some(200.0) }
    }

    fn local(id: Option<&str>, pos: f64, paused: bool) -> Local {
        Local { video_id: id.map(str::to_string), pos, paused }
    }

    #[test]
    fn latecomer_loads_at_the_right_position() {
        assert_eq!(
            decide(Some(&want(42.5, false)), &local(Some("bbbbbbbbbbb"), 3.0, false), TOLERANCE_S),
            vec![Step::Load { video_id: "aaaaaaaaaaa".into(), pos: 42.5 }]
        );
        assert_eq!(
            decide(Some(&want(-1.0, false)), &local(None, 0.0, true), TOLERANCE_S),
            vec![Step::Load { video_id: "aaaaaaaaaaa".into(), pos: 0.0 }, Step::Pause]
        );
    }

    #[test]
    fn waits_at_start_and_follows_pause() {
        let id = Some("aaaaaaaaaaa");
        assert_eq!(decide(Some(&want(-1.0, false)), &local(id, 2.0, false), TOLERANCE_S), vec![Step::Pause, Step::Seek(0.0)]);
        assert_eq!(decide(Some(&want(30.0, true)), &local(id, 30.2, false), TOLERANCE_S), vec![Step::Pause]);
        assert_eq!(decide(Some(&want(30.0, false)), &local(id, 30.2, true), TOLERANCE_S), vec![Step::Play]);
        assert_eq!(decide(None, &local(id, 3.0, false), TOLERANCE_S), vec![Step::Pause]);
        assert!(decide(None, &local(None, 0.0, true), TOLERANCE_S).is_empty());
    }

    #[test]
    fn seeks_only_beyond_tolerance() {
        let id = Some("aaaaaaaaaaa");
        assert!(decide(Some(&want(30.0, false)), &local(id, 31.2, false), TOLERANCE_S).is_empty());
        assert_eq!(decide(Some(&want(30.0, false)), &local(id, 34.0, false), TOLERANCE_S), vec![Step::Seek(30.0)]);
        assert_eq!(decide(Some(&want(30.0, false)), &local(id, 20.0, false), TOLERANCE_S), vec![Step::Seek(30.0)]);
    }

    #[test]
    fn does_not_reload_the_old_track_at_its_end() {
        // YouTube Music ist schon zum nächsten (eigenen) Titel gesprungen, der Jam noch nicht.
        assert_eq!(decide(Some(&want(198.5, false)), &local(Some("zzzzzzzzzzz"), 1.0, false), TOLERANCE_S), vec![Step::Pause]);
        assert!(decide(Some(&want(198.5, false)), &local(Some("zzzzzzzzzzz"), 1.0, true), TOLERANCE_S).is_empty());
        // Lokal schon fertig, Server knapp dahinter: nicht zurückspulen.
        assert!(decide(Some(&want(198.0, false)), &local(Some("aaaaaaaaaaa"), 199.9, false), TOLERANCE_S).is_empty());
    }

    #[test]
    fn songs_picked_in_youtube_music_go_to_the_jam() {
        let mut state = JamState::default();
        state.current = Some(Item { id: "i1".into(), video_id: "aaaaaaaaaaa".into(), dur: Some(200.0), ..Default::default() });
        state.history = vec![Item { id: "i0".into(), video_id: "hhhhhhhhhhh".into(), ..Default::default() }];
        let w = want(30.0, false);
        // Neuer Song angetippt
        assert_eq!(user_pick(&state, Some(&w), &local(Some("bbbbbbbbbbb"), 1.0, false), None).as_deref(), Some("bbbbbbbbbbb"));
        // Jam-Song selbst, schon bekannter Song, pausiert, Song vom Beitreten: kein Wunsch
        assert_eq!(user_pick(&state, Some(&w), &local(Some("aaaaaaaaaaa"), 30.0, false), None), None);
        assert_eq!(user_pick(&state, Some(&w), &local(Some("hhhhhhhhhhh"), 1.0, false), None), None);
        assert_eq!(user_pick(&state, Some(&w), &local(Some("bbbbbbbbbbb"), 1.0, true), None), None);
        assert_eq!(user_pick(&state, Some(&w), &local(Some("bbbbbbbbbbb"), 1.0, false), Some("bbbbbbbbbbb")), None);
        // Autoplay am Titelende und Start-Vorlauf: kein Wunsch
        assert_eq!(user_pick(&state, Some(&want(198.5, false)), &local(Some("zzzzzzzzzzz"), 1.0, false), None), None);
        assert_eq!(user_pick(&state, Some(&want(-1.0, false)), &local(Some("zzzzzzzzzzz"), 1.0, false), None), None);
        // Läuft im Jam noch nichts, wird der Song einfach der erste
        let empty = JamState::default();
        assert_eq!(user_pick(&empty, None, &local(Some("bbbbbbbbbbb"), 1.0, false), None).as_deref(), Some("bbbbbbbbbbb"));
    }

    #[test]
    fn detects_track_end() {
        assert!(reached_end(&local(Some("a"), 199.5, false), 200.0, false));
        assert!(!reached_end(&local(Some("a"), 150.0, false), 200.0, false));
        assert!(reached_end(&local(Some("a"), 0.0, true), 0.0, true));
        assert!(!reached_end(&local(Some("a"), 5.0, false), 0.0, false));
    }
}
