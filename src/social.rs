//! Playtube-Konten und Freunde (Tab „Sozial“): REST-Aufrufe und die WebSocket-Verbindung zum
//! Dienst in `playtube-jam` (`src/social_hub.js`). Konten sind freiwillig – Sync geht auch ohne.
//!
//! Protokoll (WebSocket `/api/social/ws`): erst `{"t":"auth","token","client":"pc"}`, dann kommen
//! `ready`, `friends` (Liste mit Online-Status und laufendem Titel), `invite` (Einladung eines
//! Freundes in seinen Sync), `invited`, `error`, `signedOut`. Wir schicken `presence` (eigener
//! Titel), `invite` (Freund in den eigenen Sync holen) und `ping`.

use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct User {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub email: String,
    #[serde(default = "yes")]
    pub share: bool,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Track {
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub thumb: Option<String>,
    #[serde(default, rename = "videoId")]
    pub video_id: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Friend {
    pub id: String,
    pub name: String,
    /// "offline", "online" oder "listening"
    pub status: String,
    #[serde(default)]
    pub track: Option<Track>,
    #[serde(default)]
    pub client: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Person {
    pub id: String,
    pub name: String,
    /// Nur in Suchergebnissen: "none", "friend", "outgoing", "incoming"
    #[serde(default)]
    pub relation: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Friends {
    #[serde(default)]
    pub friends: Vec<Friend>,
    #[serde(default)]
    pub incoming: Vec<Person>,
    #[serde(default)]
    pub outgoing: Vec<Person>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct InviteSong {
    pub title: String,
    #[serde(default)]
    pub artist: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Invite {
    pub id: String,
    pub from: Person,
    pub code: String,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub current: Option<InviteSong>,
}

/// Was wir den Freunden über den eigenen Player sagen (aus dem Ergebnis von `media_probe.js`;
/// `None` = kein Player).
pub fn presence_payload(info: Option<&Value>) -> Value {
    let Some(info) = info else { return json!({ "t": "presence", "playing": false }) };
    let playing = info["playing"].as_bool().unwrap_or(false) && info["hasVideo"].as_bool().unwrap_or(true);
    let title = info["title"].as_str().unwrap_or("").trim();
    if !playing || title.is_empty() {
        return json!({ "t": "presence", "playing": false });
    }
    let video_id = info["videoId"]
        .as_str()
        .map(str::to_string)
        .or_else(|| info["url"].as_str().and_then(crate::jam::video_id_from_url));
    json!({
        "t": "presence",
        "playing": true,
        "title": title,
        "artist": info["subtitle"].as_str().unwrap_or("").trim(),
        "thumb": info["thumbnail"].as_str().filter(|t| t.starts_with("https://")),
        "videoId": video_id,
    })
}

/// Fehler einer REST-Antwort (Schlüssel wie "wrong_login", "network").
#[derive(Debug, Clone, PartialEq)]
pub struct ApiError(pub String);

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Fehlerschlüssel aus einer Antwort des Servers.
pub fn error_of(status: u16, body: &Value) -> ApiError {
    match body["error"].as_str() {
        Some(e) if !e.is_empty() => ApiError(e.to_string()),
        _ if status == 401 => ApiError("unauthorized".into()),
        _ if status == 429 => ApiError("rate_limited".into()),
        _ => ApiError("failed".into()),
    }
}

pub async fn call(server: &str, method: &str, path: &str, token: Option<&str>, body: Option<Value>) -> Result<Value, ApiError> {
    let url = format!("{}{path}", server.trim_end_matches('/'));
    let client = crate::jam::http_client(server).await;
    let mut request = match method {
        "GET" => client.get(url),
        "PATCH" => client.patch(url),
        "DELETE" => client.delete(url),
        _ => client.post(url),
    };
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request.send().await.map_err(|e| {
        crate::log::line(&format!("social: {path} nicht erreichbar: {e}"));
        ApiError("network".into())
    })?;
    let status = response.status().as_u16();
    let value: Value = response.json().await.unwrap_or(Value::Null);
    if (200..300).contains(&status) && value["ok"].as_bool() != Some(false) {
        Ok(value)
    } else {
        Err(error_of(status, &value))
    }
}

/// Anmelden bzw. registrieren: (Token, Konto).
pub fn session_of(value: &Value) -> Option<(String, User)> {
    let token = value["token"].as_str()?.to_string();
    let user = serde_json::from_value(value["user"].clone()).ok()?;
    Some((token, user))
}

// ---------------------------------------------------------------------------- Verbindung

#[derive(Debug)]
pub enum Event {
    Connected,
    Disconnected,
    Ready(User),
    Friends(Friends),
    Invite(Invite),
    Invited { delivered: bool },
    Error { error: String, context: String },
    /// Token ungültig (abgemeldet, Passwort neu, Konto gelöscht)
    SignedOut(String),
}

pub struct Connection {
    tx: mpsc::UnboundedSender<Value>,
    task: tokio::task::JoinHandle<()>,
}

impl Connection {
    pub fn start(runtime: &tokio::runtime::Handle, server: &str, token: &str, on_event: impl Fn(Event) + Send + Sync + 'static) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let url = format!("{}/api/social/ws", ws_base(server));
        let task = runtime.spawn(run(url, token.to_string(), rx, Arc::new(on_event)));
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

/// Eine Nachricht des Servers in ein Ereignis übersetzen (`None` = nichts zu tun).
pub fn parse_message(value: &Value) -> Option<Event> {
    match value["t"].as_str()? {
        "ready" => serde_json::from_value(value["user"].clone()).ok().map(Event::Ready),
        "friends" => serde_json::from_value(value.clone()).ok().map(Event::Friends),
        "invite" => serde_json::from_value(value.clone()).ok().map(Event::Invite),
        "invited" => Some(Event::Invited { delivered: value["delivered"].as_bool().unwrap_or(false) }),
        "error" => Some(Event::Error {
            error: value["error"].as_str().unwrap_or("failed").to_string(),
            context: value["for"].as_str().unwrap_or("").to_string(),
        }),
        "signedOut" => Some(Event::SignedOut(value["reason"].as_str().unwrap_or("unauthorized").to_string())),
        _ => None,
    }
}

async fn run(url: String, token: String, mut rx: mpsc::UnboundedReceiver<Value>, on_event: Arc<dyn Fn(Event) + Send + Sync>) {
    let mut delay = Duration::from_secs(2);
    // Zuletzt gesendete Präsenz – nach einem Neuaufbau noch einmal schicken.
    let mut last_presence: Option<Value> = None;
    loop {
        match crate::jam::connect_ws(&url).await {
            Ok(stream) => {
                delay = Duration::from_secs(2);
                let (mut write, mut read) = stream.split();
                let auth = json!({ "t": "auth", "token": token, "client": "pc" });
                if write.send(Message::text(auth.to_string())).await.is_err() {
                    continue;
                }
                if let Some(presence) = &last_presence {
                    let _ = write.send(Message::text(presence.to_string())).await;
                }
                on_event(Event::Connected);
                let mut ping = tokio::time::interval(Duration::from_secs(30));
                ping.tick().await;
                let signed_out = loop {
                    tokio::select! {
                        incoming = read.next() => {
                            let Some(Ok(message)) = incoming else { break false };
                            let Message::Text(text) = message else { continue };
                            let Ok(value) = serde_json::from_str::<Value>(text.as_str()) else { continue };
                            match parse_message(&value) {
                                Some(Event::SignedOut(reason)) => {
                                    on_event(Event::SignedOut(reason));
                                    break true;
                                }
                                Some(event) => on_event(event),
                                None => {}
                            }
                        }
                        outgoing = rx.recv() => {
                            let Some(msg) = outgoing else { return };
                            if msg["t"] == "presence" {
                                last_presence = Some(msg.clone());
                            }
                            if write.send(Message::text(msg.to_string())).await.is_err() {
                                break false;
                            }
                        }
                        _ = ping.tick() => {
                            if write.send(Message::text(json!({ "t": "ping" }).to_string())).await.is_err() {
                                break false;
                            }
                        }
                    }
                };
                if signed_out {
                    let _ = write.close().await;
                    return;
                }
                on_event(Event::Disconnected);
            }
            Err(e) => {
                crate::log::line(&format!("social: Verbindung fehlgeschlagen: {e}"));
                on_event(Event::Disconnected);
            }
        }
        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(Duration::from_secs(60));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_presence_from_the_player() {
        let info = json!({ "playing": true, "hasVideo": true, "title": " Song ", "subtitle": "Band",
            "thumbnail": "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg", "url": "https://music.youtube.com/watch?v=dQw4w9WgXcQ&list=x" });
        let p = presence_payload(Some(&info));
        assert_eq!(p["playing"], true);
        assert_eq!(p["title"], "Song");
        assert_eq!(p["artist"], "Band");
        assert_eq!(p["videoId"], "dQw4w9WgXcQ");
        assert_eq!(presence_payload(None), json!({ "t": "presence", "playing": false }));
        let paused = json!({ "playing": false, "title": "Song" });
        assert_eq!(presence_payload(Some(&paused))["playing"], false);
        let no_title = json!({ "playing": true, "title": "" });
        assert_eq!(presence_payload(Some(&no_title))["playing"], false);
        let bad_thumb = json!({ "playing": true, "title": "x", "thumbnail": "data:image/png;base64,AAA" });
        assert!(presence_payload(Some(&bad_thumb))["thumb"].is_null());
    }

    #[test]
    fn reads_server_messages() {
        let friends = json!({ "t": "friends", "friends": [
            { "id": "u1", "name": "Anna", "status": "listening", "client": "android",
              "track": { "title": "Song", "artist": "Band", "thumb": null, "videoId": "dQw4w9WgXcQ" } },
            { "id": "u2", "name": "Max", "status": "offline", "track": null, "client": null }
        ], "incoming": [{ "id": "u3", "name": "Zoe" }], "outgoing": [] });
        let Some(Event::Friends(list)) = parse_message(&friends) else { panic!("keine Freundesliste") };
        assert_eq!(list.friends.len(), 2);
        assert_eq!(list.friends[0].track.as_ref().unwrap().video_id.as_deref(), Some("dQw4w9WgXcQ"));
        assert_eq!(list.incoming[0].name, "Zoe");

        let invite = json!({ "t": "invite", "id": "i1", "from": { "id": "u1", "name": "Anna" }, "code": "ABC234",
            "host": "Anna", "current": { "title": "Song", "artist": "Band" }, "at": 5 });
        let Some(Event::Invite(inv)) = parse_message(&invite) else { panic!("keine Einladung") };
        assert_eq!(inv.code, "ABC234");
        assert_eq!(inv.from.name, "Anna");
        assert_eq!(inv.current.unwrap().title, "Song");

        let ready = json!({ "t": "ready", "user": { "id": "u0", "name": "Chris", "email": "c@example.com", "share": false } });
        assert!(matches!(parse_message(&ready), Some(Event::Ready(User { share: false, .. }))));
        assert!(matches!(parse_message(&json!({ "t": "signedOut", "reason": "deleted" })), Some(Event::SignedOut(r)) if r == "deleted"));
        assert!(matches!(parse_message(&json!({ "t": "error", "error": "jam_not_found", "for": "invite" })), Some(Event::Error { .. })));
        assert!(parse_message(&json!({ "t": "pong" })).is_none());
    }

    #[test]
    fn reads_sessions_and_errors() {
        let ok = json!({ "ok": true, "token": "abc", "user": { "id": "u1", "name": "Chris", "email": "c@example.com", "share": true } });
        let (token, user) = session_of(&ok).unwrap();
        assert_eq!(token, "abc");
        assert_eq!(user.name, "Chris");
        assert!(session_of(&json!({ "ok": true })).is_none());
        assert_eq!(error_of(401, &json!({ "error": "wrong_login" })), ApiError("wrong_login".into()));
        assert_eq!(error_of(401, &Value::Null), ApiError("unauthorized".into()));
        assert_eq!(error_of(429, &json!({})), ApiError("rate_limited".into()));
        assert_eq!(error_of(500, &json!({})), ApiError("failed".into()));
        assert_eq!(ws_base("https://jam.fojadrachi.de/"), "wss://jam.fojadrachi.de");
    }

    /// Gegen den echten Server (nicht in der normalen Testrunde): `cargo test social_live -- --ignored`
    #[test]
    #[ignore]
    fn social_live_register_and_delete() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let server = std::env::var("PLAYTUBE_SOCIAL_SERVER").unwrap_or_else(|_| crate::jam::DEFAULT_SERVER.into());
            let stamp = format!("{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() % 0xffffff);
            let body = json!({ "name": format!("pc_{stamp}"), "email": format!("pc_{stamp}@example.com"), "password": "geheim123", "client": "pc" });
            let (token, user) = session_of(&call(&server, "POST", "/api/account/register", None, Some(body)).await.unwrap()).unwrap();
            assert_eq!(user.name, format!("pc_{stamp}"));
            let wrong = call(&server, "POST", "/api/account/login", None, Some(json!({ "login": user.name, "password": "falsch!!" }))).await;
            assert_eq!(wrong.unwrap_err(), ApiError("wrong_login".into()));
            call(&server, "DELETE", "/api/account", Some(&token), Some(json!({ "password": "geheim123" }))).await.unwrap();
        });
    }
}
