//! Fernsteuerung: Protokoll und lokaler Named-Pipe-Server (Stream-Dock-Plugin
//! com.fojadrachi.playtube.sdPlugin). Dasselbe Protokoll nutzt die Steuerung vom Handy (lan.rs).
//!
//! Eine JSON-Nachricht pro Zeile, Verbindung bleibt offen:
//!   -> {"id": 1, "cmd": "play_pause", "target": "auto"}     <- {"id": 1, "ok": true}
//!   -> {"id": 2, "cmd": "status"}                           <- {"id": 2, "ok": true, "state": {...}}
//!   -> {"id": 4, "cmd": "list_playlists"}                   <- {"id": 4, "ok": true, "playlists": [...]}
//!   -> {"id": 5, "cmd": "play_playlist", "playlist": "PL..."}
//!   -> {"id": 6, "cmd": "queue", "target": "auto"}          <- {"id": 6, "ok": true, "queue": [{"title", "subtitle", "thumbnail", "current"}]}
//!   -> {"id": 7, "cmd": "play_queue_item", "target": "music", "value": 3}
//! `cmd` kommt aus einer festen Whitelist; beliebiges JavaScript oder URLs lassen sich nicht senden.

use crate::scripts::MEDIA_COMMANDS;
use serde_json::{json, Map, Value};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};

pub const PROTOCOL_VERSION: u32 = 1;
pub const PIPE_PREFIX: &str = r"\\.\pipe\Playtube.Remote.";
pub const MAX_LINE_BYTES: usize = 4096;
const MAX_CLIENTS: usize = 8;

pub const TAB_NAMES: [&str; 2] = ["youtube", "music"];
const APP_COMMANDS: &[&str] =
    &["status", "show", "switch_tab", "list_playlists", "play_playlist", "playlist_cover", "queue", "play_queue_item"];
const MAX_PLAYLISTS: usize = 100;
const MAX_PLAYLIST_TITLE: usize = 100;
const MAX_QUEUE: usize = 200;
const MAX_QUEUE_TEXT: usize = 150;

/// Nur diese Felder des Wiedergabestands gehen nach außen.
const STATE_FIELDS: &[&str] = &[
    "isMusic", "title", "subtitle", "thumbnail", "playing", "currentTime", "duration", "hasVideo",
    "volume", "muted", "likeStatus", "repeatMode",
];

#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub id: Value,
    pub command: String,
    pub target: String,
    pub value: f64,
    pub playlist: Option<String>,
}

/// Was die App für eine Anfrage tun soll (läuft im UI-Thread, siehe main.rs).
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Status,
    Show,
    SwitchTab(String),
    Media { target: String, command: String, value: f64 },
    ListPlaylists,
    PlayPlaylist(String),
    PlaylistCover(String),
    Queue(String),
    PlayQueueItem { target: String, index: u32 },
}

pub type Reply = Box<dyn FnOnce(Value) + Send>;
/// Übergibt eine Aktion an die App; die Antwort (Status, Playlists oder null) kommt über `Reply`.
pub type Dispatch = Arc<dyn Fn(Action, Reply) + Send + Sync>;

pub fn is_playlist_id(id: &str) -> bool {
    (2..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Parst und prüft eine Anfragezeile.
pub fn parse_request(line: &str) -> Result<Request, String> {
    let data: Value = serde_json::from_str(line).map_err(|_| "ungueltiges JSON".to_string())?;
    let obj = data.as_object().ok_or("Anfrage muss ein JSON-Objekt sein")?;
    let id = obj.get("id").cloned().unwrap_or(Value::Null);
    if !matches!(id, Value::Null | Value::String(_)) && !id.is_i64() && !id.is_u64() {
        return Err("id muss Zahl oder Text sein".into());
    }
    let command = obj.get("cmd").and_then(Value::as_str).unwrap_or_default();
    if !APP_COMMANDS.contains(&command) && !MEDIA_COMMANDS.contains(&command) {
        return Err(format!("unbekannter Befehl: {}", obj.get("cmd").unwrap_or(&Value::Null)));
    }
    let target = match obj.get("target") {
        None => "auto",
        Some(Value::String(t)) if t == "auto" || TAB_NAMES.contains(&t.as_str()) => t.as_str(),
        Some(other) => return Err(format!("unbekanntes Ziel: {other}")),
    };
    let playlist = if command == "play_playlist" || command == "playlist_cover" {
        match obj.get("playlist").and_then(Value::as_str) {
            Some(p) if is_playlist_id(p) => Some(p.to_string()),
            _ => return Err(format!("{command} braucht eine gueltige Playlist-ID")),
        }
    } else {
        None
    };
    let range = match command {
        "seek" => Some((-3600.0, 3600.0)),
        "volume_change" => Some((-100.0, 100.0)),
        "set_volume" => Some((0.0, 100.0)),
        "play_queue_item" => Some((0.0, (MAX_QUEUE - 1) as f64)),
        _ => None,
    };
    let value = match range {
        None => 0.0,
        Some((low, high)) => {
            let raw = obj.get("value").filter(|v| v.is_number()).and_then(Value::as_f64);
            let raw = raw.filter(|v| v.is_finite()).ok_or(format!("{command} braucht einen Zahlenwert"))?;
            if !(low..=high).contains(&raw) {
                return Err(format!("{command}: Wert muss zwischen {low} und {high} liegen"));
            }
            if command == "play_queue_item" && raw.fract() != 0.0 {
                return Err("play_queue_item braucht eine ganze Zahl".into());
            }
            raw
        }
    };
    Ok(Request { id, command: command.to_string(), target: target.to_string(), value, playlist })
}

pub fn action_for(request: &Request) -> Action {
    match request.command.as_str() {
        "status" => Action::Status,
        "show" => Action::Show,
        "switch_tab" => Action::SwitchTab(request.target.clone()),
        "list_playlists" => Action::ListPlaylists,
        "play_playlist" => Action::PlayPlaylist(request.playlist.clone().unwrap_or_default()),
        "playlist_cover" => Action::PlaylistCover(request.playlist.clone().unwrap_or_default()),
        "queue" => Action::Queue(request.target.clone()),
        "play_queue_item" => Action::PlayQueueItem { target: request.target.clone(), index: request.value as u32 },
        _ => Action::Media { target: request.target.clone(), command: request.command.clone(), value: request.value },
    }
}

/// Cover-Adresse aus der Seite (fremde Daten!): nur https-Bilder von YouTube/Google, groß angefordert.
pub fn sanitize_cover(raw: &Value) -> Option<String> {
    const HOSTS: &[&str] = &["https://lh3.googleusercontent.com/", "https://yt3.googleusercontent.com/",
                             "https://yt3.ggpht.com/", "https://i.ytimg.com/", "https://i9.ytimg.com/"];
    let url = raw.as_str()?.trim();
    let allowed = url.len() < 2000 && HOSTS.iter().any(|h| url.starts_with(h)) && !url.contains(['"', '<', '>', ' ']);
    allowed.then(|| crate::discord::upsize_thumbnail(url))
}

fn clean_text(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_QUEUE_TEXT).collect())
        .unwrap_or_default()
}

/// Bereinigt die von der Seite gelieferte Wiedergabeliste (fremde Daten!): nur Text, nur
/// https-Bilder (in großer Fassung), höchstens 200 Einträge. Die Reihenfolge bleibt, damit
/// `play_queue_item` mit demselben Index den richtigen Titel trifft.
pub fn sanitize_queue(raw: &Value) -> Vec<Value> {
    raw.as_array()
        .into_iter()
        .flatten()
        .take(MAX_QUEUE)
        .map(|item| {
            let thumbnail = item
                .get("thumbnail")
                .and_then(Value::as_str)
                .filter(|t| t.starts_with("https://") && t.len() < 2000)
                .map(crate::discord::upsize_thumbnail)
                .unwrap_or_default();
            json!({
                "title": clean_text(item.get("title")),
                "subtitle": clean_text(item.get("subtitle")),
                "thumbnail": thumbnail,
                "current": item.get("current").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect()
}

/// Bereinigt die von der Seite gelieferte Playlist-Liste (fremde Daten!).
pub fn sanitize_playlists(raw: &Value) -> Vec<Value> {
    let mut result = vec![json!({"id": "LM", "title": "Titel, die ich mag"})];
    let mut seen = vec!["LM".to_string()];
    for item in raw.as_array().into_iter().flatten() {
        if result.len() >= MAX_PLAYLISTS {
            break;
        }
        let Some(id) = item.get("id").and_then(Value::as_str).filter(|id| is_playlist_id(id)) else { continue };
        if seen.iter().any(|s| s == id) {
            continue;
        }
        seen.push(id.to_string());
        let title: String = item
            .get("title")
            .and_then(Value::as_str)
            .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_PLAYLIST_TITLE).collect())
            .unwrap_or_default();
        result.push(json!({"id": id, "title": if title.is_empty() { id.to_string() } else { title }}));
    }
    result
}

/// Welcher Tab einen Medienbefehl bekommt ("auto": der abspielende, sonst der sichtbare,
/// sonst der mit Video, sonst YouTube).
pub fn resolve_target<'a>(target: &'a str, visible: &str, tabs: &[Option<&Value>; 2]) -> &'a str {
    if TAB_NAMES.contains(&target) {
        return target;
    }
    let flag = |i: usize, key: &str| tabs[i].and_then(|t| t.get(key)).and_then(Value::as_bool).unwrap_or(false);
    let playing: Vec<usize> = (0..2).filter(|&i| flag(i, "playing")).collect();
    if playing.len() == 1 {
        return TAB_NAMES[playing[0]];
    }
    if let Some(i) = TAB_NAMES.iter().position(|n| *n == visible) {
        return TAB_NAMES[i];
    }
    (0..2).find(|&i| flag(i, "hasVideo")).map(|i| TAB_NAMES[i]).unwrap_or(TAB_NAMES[0])
}

pub fn build_state(version: &str, visible: &str, tabs: &[Option<&Value>; 2]) -> Value {
    let mut public = Map::new();
    for (i, name) in TAB_NAMES.iter().enumerate() {
        let entry = tabs[i].map(|info| {
            let mut fields: Map<String, Value> =
                STATE_FIELDS.iter().map(|k| (k.to_string(), info.get(*k).cloned().unwrap_or(Value::Null))).collect();
            // Das Cover der YouTube-Music-Player-Leiste ist nur 60x60 Pixel groß - für Handy und
            // Stream Dock die große Fassung schicken (wie für Discord).
            if let Some(Value::String(thumb)) = fields.get_mut("thumbnail") {
                *thumb = crate::discord::upsize_thumbnail(thumb);
            }
            Value::Object(fields)
        });
        public.insert(name.to_string(), entry.unwrap_or(Value::Null));
    }
    json!({
        "protocol": PROTOCOL_VERSION,
        "version": version,
        "visibleTab": visible,
        "activeTab": resolve_target("auto", visible, tabs),
        "tabs": public,
    })
}

pub fn encode(id: &Value, mut response: Map<String, Value>) -> Vec<u8> {
    response.insert("id".into(), id.clone());
    let mut bytes = serde_json::to_vec(&Value::Object(response)).unwrap_or_default();
    bytes.push(b'\n');
    bytes
}

pub fn error(id: &Value, message: &str) -> Vec<u8> {
    let mut map = Map::new();
    map.insert("ok".into(), Value::Bool(false));
    map.insert("error".into(), Value::String(message.to_string()));
    encode(id, map)
}

/// Eine Anfragezeile -> genau eine Antwortzeile.
pub async fn process_line(line: &str, dispatch: &Dispatch) -> Vec<u8> {
    let request = match parse_request(line) {
        Ok(r) => r,
        Err(e) => return error(&Value::Null, &e),
    };
    let (tx, rx) = tokio::sync::oneshot::channel::<Value>();
    dispatch(action_for(&request), Box::new(move |v| {
        let _ = tx.send(v);
    }));
    // Die Seite antwortet normalerweise sofort; nach 10 s gilt sie als hängend.
    let result = tokio::time::timeout(std::time::Duration::from_secs(10), rx).await;
    let mut map = Map::new();
    match (request.command.as_str(), result) {
        (_, Err(_)) | (_, Ok(Err(_))) => return error(&request.id, "keine Antwort von der Seite"),
        ("status", Ok(Ok(state))) => {
            map.insert("ok".into(), Value::Bool(true));
            map.insert("state".into(), state);
        }
        ("list_playlists", Ok(Ok(raw))) => {
            map.insert("ok".into(), Value::Bool(true));
            map.insert("playlists".into(), Value::Array(sanitize_playlists(&raw)));
        }
        ("queue", Ok(Ok(raw))) => {
            map.insert("ok".into(), Value::Bool(true));
            map.insert("queue".into(), Value::Array(sanitize_queue(&raw)));
        }
        ("playlist_cover", Ok(Ok(raw))) => {
            map.insert("ok".into(), Value::Bool(true));
            map.insert("cover".into(), sanitize_cover(&raw).map(Value::String).unwrap_or(Value::Null));
        }
        _ => {
            map.insert("ok".into(), Value::Bool(true));
        }
    }
    encode(&request.id, map)
}

/// Liest Zeilen von `stream`, beantwortet sie über `handle` und schreibt die Antworten zurück.
/// Zu lange Zeilen beenden die Verbindung.
pub async fn serve_lines<S, F, Fut>(stream: S, mut handle: F)
where
    S: AsyncRead + AsyncWrite + Unpin,
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = Vec<u8>>,
{
    let (read, mut write) = tokio::io::split(stream);
    let mut reader = BufReader::new(read);
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        let mut limited = (&mut reader).take(MAX_LINE_BYTES as u64 + 1);
        match limited.read_until(b'\n', &mut buffer).await {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        if buffer.len() > MAX_LINE_BYTES && !buffer.ends_with(b"\n") {
            let _ = write.write_all(&error(&Value::Null, "Nachricht zu lang")).await;
            return;
        }
        let line = String::from_utf8_lossy(&buffer).trim().to_string();
        if line.is_empty() {
            continue;
        }
        let answer = handle(line).await;
        if write.write_all(&answer).await.is_err() {
            return;
        }
    }
}

/// Named-Pipe-Server für das Stream-Dock-Plugin (nur lokal, keine Netzwerk-Clients).
pub async fn run_pipe_server(name: String, dispatch: Dispatch) {
    use tokio::net::windows::named_pipe::ServerOptions;
    let slots = Arc::new(tokio::sync::Semaphore::new(MAX_CLIENTS));
    let mut first = true;
    loop {
        let server = match ServerOptions::new().first_pipe_instance(first).reject_remote_clients(true).create(&name) {
            Ok(s) => s,
            Err(e) => {
                crate::log::line(&format!("remote: Pipe {name} nicht erstellbar: {e}"));
                return;
            }
        };
        first = false;
        if server.connect().await.is_err() {
            continue;
        }
        let Ok(permit) = slots.clone().try_acquire_owned() else { continue };
        let dispatch = dispatch.clone();
        tokio::spawn(async move {
            serve_lines(server, |line| {
                let dispatch = dispatch.clone();
                async move { process_line(&line, &dispatch).await }
            })
            .await;
            drop(permit);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_dispatch(log: Arc<std::sync::Mutex<Vec<Action>>>) -> Dispatch {
        Arc::new(move |action, reply| {
            log.lock().unwrap().push(action.clone());
            reply(match action {
                Action::Status => json!({"protocol": 1}),
                Action::ListPlaylists => json!([{"id": "PL1", "title": "Mix"}]),
                _ => Value::Null,
            });
        })
    }

    #[test]
    fn parses_media_command_with_default_target() {
        let r = parse_request(r#"{"id": 7, "cmd": "play_pause"}"#).unwrap();
        assert_eq!((r.command.as_str(), r.target.as_str(), r.value), ("play_pause", "auto", 0.0));
        let r = parse_request(r#"{"cmd": "seek", "target": "music", "value": -10}"#).unwrap();
        assert_eq!((r.target.as_str(), r.value), ("music", -10.0));
        assert_eq!(parse_request(r#"{"cmd": "next", "value": "evil()"}"#).unwrap().value, 0.0);
    }

    #[test]
    fn rejects_invalid_input() {
        for line in [
            "not json", "[1, 2]", r#"{"cmd": "eval"}"#, r#"{"cmd": ["play_pause"]}"#,
            r#"{"cmd": "next", "target": "https://example.com"}"#, r#"{"cmd": "next", "id": {"x": 1}}"#,
            r#"{"cmd": "next", "id": true}"#, r#"{"cmd": "set_volume"}"#, r#"{"cmd": "set_volume", "value": "50"}"#,
            r#"{"cmd": "set_volume", "value": true}"#, r#"{"cmd": "set_volume", "value": 101}"#,
            r#"{"cmd": "seek", "value": 99999}"#, r#"{"cmd": "play_playlist"}"#,
            r#"{"cmd": "play_playlist", "playlist": "x"}"#, r#"{"cmd": "play_playlist", "playlist": "PL1&autoplay=0"}"#,
            r#"{"cmd": "play_playlist", "playlist": "../evil"}"#, r#"{"cmd": "play_playlist", "playlist": 123}"#,
        ] {
            assert!(parse_request(line).is_err(), "{line}");
        }
    }

    #[test]
    fn parses_queue_commands() {
        let r = parse_request(r#"{"cmd": "play_queue_item", "target": "music", "value": 3}"#).unwrap();
        assert_eq!(action_for(&r), Action::PlayQueueItem { target: "music".into(), index: 3 });
        assert_eq!(action_for(&parse_request(r#"{"cmd": "queue"}"#).unwrap()), Action::Queue("auto".into()));
        for bad in [r#"{"cmd": "play_queue_item"}"#, r#"{"cmd": "play_queue_item", "value": -1}"#,
                    r#"{"cmd": "play_queue_item", "value": 1.5}"#, r#"{"cmd": "play_queue_item", "value": 5000}"#] {
            assert!(parse_request(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn parses_playlist_cover_and_checks_the_url() {
        let r = parse_request(r#"{"id": 1, "cmd": "playlist_cover", "playlist": "PLabc_-1"}"#).unwrap();
        assert_eq!(action_for(&r), Action::PlaylistCover("PLabc_-1".into()));
        for bad in [r#"{"cmd": "playlist_cover"}"#, r#"{"cmd": "playlist_cover", "playlist": "a\"b"}"#,
                    r#"{"cmd": "playlist_cover", "playlist": "PL1&x=1"}"#] {
            assert!(parse_request(bad).is_err(), "{bad}");
        }
        assert_eq!(
            sanitize_cover(&json!("https://lh3.googleusercontent.com/abc=w120-h120-l90-rj")).as_deref(),
            Some("https://lh3.googleusercontent.com/abc=w544-h544-l90-rj")
        );
        assert_eq!(sanitize_cover(&json!("https://i.ytimg.com/vi/x/hqdefault.jpg")).as_deref(), Some("https://i.ytimg.com/vi/x/hqdefault.jpg"));
        for bad in [json!("http://i.ytimg.com/x.jpg"), json!("https://evil.example/x.jpg"), json!("javascript:alert(1)"),
                    json!("https://i.ytimg.com/x\".jpg"), Value::Null, json!(5)] {
            assert_eq!(sanitize_cover(&bad), None, "{bad}");
        }
    }

    #[tokio::test]
    async fn playlist_cover_reply_contains_the_cover() {
        let dispatch: Dispatch = Arc::new(|_action, reply| reply(json!("https://yt3.ggpht.com/c=s88-c")));
        let reply: Value = serde_json::from_slice(
            &process_line(r#"{"id": 9, "cmd": "playlist_cover", "playlist": "LM"}"#, &dispatch).await,
        )
        .unwrap();
        assert_eq!(reply["ok"], true);
        assert_eq!(reply["cover"], "https://yt3.ggpht.com/c=s88-c");
        let empty: Dispatch = Arc::new(|_action, reply| reply(Value::Null));
        let reply: Value =
            serde_json::from_slice(&process_line(r#"{"cmd": "playlist_cover", "playlist": "LM"}"#, &empty).await).unwrap();
        assert!(reply["cover"].is_null());
    }

    #[test]
    fn sanitizes_queue() {
        let raw = json!([
            {"title": "  Song\n A ", "subtitle": "Artist", "thumbnail": "https://lh3.googleusercontent.com/x=w60-h60-l90-rj", "current": true},
            {"title": "B", "thumbnail": "javascript:alert(1)", "current": "ja"},
            "kein Objekt"
        ]);
        let queue = sanitize_queue(&raw);
        assert_eq!(queue.len(), 3);
        assert_eq!(queue[0], json!({"title": "Song A", "subtitle": "Artist",
            "thumbnail": "https://lh3.googleusercontent.com/x=w544-h544-l90-rj", "current": true}));
        assert_eq!(queue[1]["thumbnail"], "");
        assert_eq!(queue[1]["current"], false);
        assert_eq!(queue[2]["title"], "");
        let many: Vec<Value> = (0..500).map(|i| json!({"title": format!("T{i}")})).collect();
        assert_eq!(sanitize_queue(&Value::Array(many)).len(), 200);
    }

    #[test]
    fn sanitizes_playlists() {
        let raw = json!([{"id": "PL1", "title": "  Mein   Mix \t"}, {"id": "LM", "title": "x"}, {"id": "PL1"},
                         {"id": "bad id!"}, {"id": "PL2", "title": null}, "kein Objekt"]);
        assert_eq!(sanitize_playlists(&raw), vec![
            json!({"id": "LM", "title": "Titel, die ich mag"}),
            json!({"id": "PL1", "title": "Mein Mix"}),
            json!({"id": "PL2", "title": "PL2"}),
        ]);
        let many: Vec<Value> = (0..500).map(|i| json!({"id": format!("PL{i:03}"), "title": "t".repeat(500)})).collect();
        let result = sanitize_playlists(&Value::Array(many));
        assert_eq!(result.len(), 100);
        assert!(result.iter().all(|p| p["title"].as_str().unwrap().chars().count() <= 100));
    }

    #[test]
    fn resolves_auto_target() {
        let playing = json!({"playing": true});
        let idle = json!({"playing": false});
        assert_eq!(resolve_target("music", "youtube", &[None, None]), "music");
        assert_eq!(resolve_target("auto", "youtube", &[Some(&idle), Some(&playing)]), "music");
        assert_eq!(resolve_target("auto", "music", &[Some(&playing), Some(&playing)]), "music");
        let video = json!({"hasVideo": true});
        assert_eq!(resolve_target("auto", "settings", &[None, Some(&video)]), "music");
        assert_eq!(resolve_target("auto", "settings", &[None, None]), "youtube");
    }

    #[test]
    fn state_exposes_only_public_fields() {
        let yt = json!({"title": "T", "playing": true, "url": "https://x"});
        let state = build_state("9.9.9", "youtube", &[Some(&yt), None]);
        assert_eq!(state["activeTab"], "youtube");
        assert!(state["tabs"]["music"].is_null());
        assert_eq!(state["tabs"]["youtube"]["title"], "T");
        assert!(state["tabs"]["youtube"].get("url").is_none());
    }

    #[test]
    fn state_sends_large_music_covers() {
        let music = json!({"thumbnail": "https://lh3.googleusercontent.com/abc=w60-h60-l90-rj"});
        let state = build_state("1", "music", &[None, Some(&music)]);
        assert_eq!(state["tabs"]["music"]["thumbnail"], "https://lh3.googleusercontent.com/abc=w544-h544-l90-rj");
    }

    #[tokio::test]
    async fn processes_lines() {
        let log = Arc::new(std::sync::Mutex::new(Vec::new()));
        let dispatch = fake_dispatch(log.clone());
        let reply: Value = serde_json::from_slice(&process_line(r#"{"id": 1, "cmd": "next"}"#, &dispatch).await).unwrap();
        assert_eq!(reply, json!({"id": 1, "ok": true}));
        let reply: Value = serde_json::from_slice(&process_line(r#"{"id": 2, "cmd": "status"}"#, &dispatch).await).unwrap();
        assert_eq!(reply, json!({"id": 2, "ok": true, "state": {"protocol": 1}}));
        let reply: Value = serde_json::from_slice(&process_line(r#"{"id": 3, "cmd": "list_playlists"}"#, &dispatch).await).unwrap();
        assert_eq!(reply["playlists"][1], json!({"id": "PL1", "title": "Mix"}));
        let reply: Value = serde_json::from_slice(&process_line(r#"{"cmd": "nope"}"#, &dispatch).await).unwrap();
        assert_eq!(reply["ok"], false);
        assert!(reply["id"].is_null());
        assert_eq!(log.lock().unwrap()[0], Action::Media { target: "auto".into(), command: "next".into(), value: 0.0 });
    }

    #[tokio::test]
    async fn round_trip_over_named_pipe() {
        use tokio::io::AsyncWriteExt;
        let name = format!(r"\\.\pipe\Playtube.Remote.Test.{}", std::process::id());
        let log = Arc::new(std::sync::Mutex::new(Vec::new()));
        tokio::spawn(run_pipe_server(name.clone(), fake_dispatch(log.clone())));
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let client = tokio::net::windows::named_pipe::ClientOptions::new().open(&name).unwrap();
        let (read, mut write) = tokio::io::split(client);
        write.write_all(b"{\"id\": 1, \"cmd\": \"next\"}\n{\"id\": 2, \"cmd\": \"status\"}\n").await.unwrap();
        let mut lines = BufReader::new(read).lines();
        let first: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        let second: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(first, json!({"id": 1, "ok": true}));
        assert_eq!(second["state"], json!({"protocol": 1}));
    }
}
