//! Discord Rich Presence in einem eigenen Thread (die IPC-Aufrufe blockieren). Updates werden
//! höchstens alle `interval` Sekunden (mindestens 15, sonst verwirft Discord sie) gesendet; ist
//! Discord nicht da, wird alle 10 Sekunden neu versucht.

use crate::config::APP_NAME;
use crate::i18n::{tr, trf};
use discord_rich_presence::activity::{Activity, ActivityType, Assets, Button, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use serde_json::Value;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const ASSET_YOUTUBE: &str = "youtube_logo";
const ASSET_MUSIC: &str = "music_logo";
const ASSET_DEFAULT: &str = "logo";
const ASSET_PLAY: &str = "play_icon";
const ASSET_PAUSE: &str = "pause_icon";
const RECONNECT_DELAY: Duration = Duration::from_secs(10);

/// Was Discord gerade zeigen soll.
#[derive(Clone, Debug, PartialEq)]
pub enum Presence {
    Media(Value),
    Idle,
    Settings,
}

enum Message {
    Show(Presence),
    Stop,
}

pub struct DiscordWorker {
    tx: Sender<Message>,
    thread: Option<std::thread::JoinHandle<()>>,
}

/// Discord-Anwendung "Playtube" (öffentliche ID, kein Geheimnis). Fest eingebaut, damit der Status
/// immer als "Playtube" mit den eigenen Bildern erscheint; eine alte `client_id` in der config.json
/// wird ignoriert.
pub const CLIENT_ID: &str = "1548023494976086127";

impl DiscordWorker {
    pub fn start(interval_seconds: u64, show_idle: bool) -> Self {
        let client_id = CLIENT_ID.to_string();
        let (tx, rx) = mpsc::channel();
        let interval = Duration::from_secs(interval_seconds.max(15));
        let thread = std::thread::Builder::new()
            .name("discord".into())
            .spawn(move || run(rx, client_id, interval, show_idle))
            .ok();
        Self { tx, thread }
    }

    pub fn show(&self, presence: Presence) {
        let _ = self.tx.send(Message::Show(presence));
    }
}

impl Drop for DiscordWorker {
    fn drop(&mut self) {
        let _ = self.tx.send(Message::Stop);
        // Nicht ewig warten, falls Discord gerade hängt.
        if let Some(thread) = self.thread.take() {
            let deadline = Instant::now() + Duration::from_secs(2);
            while !thread.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn truncate(text: Option<&str>, fallback: &str) -> String {
    let text = text.map(str::trim).filter(|t| !t.is_empty()).unwrap_or(fallback);
    if text.chars().count() > 128 {
        let cut: String = text.chars().take(127).collect();
        format!("{}…", cut.trim_end())
    } else {
        text.to_string()
    }
}

/// YouTube-Music-Cover kommen sehr klein ("=w60-h60-l90-rj") - für Discord größer anfordern.
pub fn upsize_thumbnail(url: &str) -> String {
    if let Some(pos) = url.rfind("=w") {
        let tail = &url[pos + 2..];
        let looks_like_size = tail.split('-').next().map(|w| w.chars().all(|c| c.is_ascii_digit())).unwrap_or(false)
            && tail.contains("-h");
        if looks_like_size {
            return format!("{}=w544-h544-l90-rj", &url[..pos]);
        }
    }
    url.to_string()
}

struct State {
    client: Option<DiscordIpcClient>,
    client_id: String,
    show_idle: bool,
    session_start: i64,
    track_key: Option<(bool, String, String)>,
    track_start: i64,
}

fn run(rx: Receiver<Message>, client_id: String, interval: Duration, show_idle: bool) {
    let mut state = State { client: None, client_id, show_idle, session_start: now_secs(), track_key: None, track_start: 0 };
    let mut last_sent: Option<Instant> = None;
    let mut pending: Option<Presence> = None;
    loop {
        // Warten, bis gesendet werden darf; neuere Stände ersetzen ältere.
        let wait = match (&pending, last_sent) {
            (None, _) => Duration::from_secs(3600),
            (Some(_), Some(t)) => interval.saturating_sub(t.elapsed()),
            (Some(_), None) => Duration::ZERO,
        };
        match rx.recv_timeout(wait) {
            Ok(Message::Show(p)) => {
                pending = Some(p);
                continue;
            }
            Ok(Message::Stop) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
        let Some(presence) = pending.take() else { continue };
        if state.client.is_none() {
            let mut client = DiscordIpcClient::new(&state.client_id);
            if client.connect().is_ok() {
                state.client = Some(client);
            } else {
                pending = Some(presence);
                match rx.recv_timeout(RECONNECT_DELAY) {
                    Ok(Message::Show(p)) => pending = Some(p),
                    Ok(Message::Stop) | Err(RecvTimeoutError::Disconnected) => break,
                    Err(RecvTimeoutError::Timeout) => {}
                }
                continue;
            }
        }
        if send(&mut state, &presence).is_err() {
            // Discord evtl. geschlossen -> beim nächsten Mal neu verbinden.
            state.client = None;
            pending = Some(presence);
        }
        last_sent = Some(Instant::now());
    }
    if let Some(mut client) = state.client.take() {
        let _ = client.clear_activity();
        let _ = client.close();
    }
}

fn send(state: &mut State, presence: &Presence) -> Result<(), discord_rich_presence::error::Error> {
    let session_start = state.session_start;
    let client = state.client.as_mut().expect("verbunden");
    match presence {
        Presence::Idle => {
            state.track_key = None;
            if !state.show_idle {
                return client.clear_activity();
            }
            let details = trf("rpc.idle_details", &[("app", APP_NAME)]);
            client.set_activity(
                Activity::new()
                    .details(details)
                    .state(tr("rpc.idle_state"))
                    .assets(Assets::new().large_image(ASSET_DEFAULT).large_text(APP_NAME))
                    .timestamps(Timestamps::new().start(session_start)),
            )
        }
        Presence::Settings => {
            state.track_key = None;
            client.set_activity(
                Activity::new()
                    .details(tr("rpc.settings_details"))
                    .state(tr("rpc.settings_state"))
                    .assets(Assets::new().large_image(ASSET_DEFAULT).large_text(APP_NAME))
                    .timestamps(Timestamps::new().start(session_start)),
            )
        }
        Presence::Media(info) => {
            let is_music = info["isMusic"].as_bool().unwrap_or(false);
            let playing = info["playing"].as_bool().unwrap_or(false);
            let title = truncate(info["title"].as_str(), tr("rpc.unknown_title"));
            let subtitle = truncate(info["subtitle"].as_str(), if is_music { "YouTube Music" } else { "YouTube" });
            let duration = info["duration"].as_f64().unwrap_or(0.0);
            let current = info["currentTime"].as_f64().unwrap_or(0.0);

            // Zeitanker nur bei einem neuen Titel setzen: YouTube Music zählt currentTime bei
            // nahtlosen Songwechseln teils einfach weiter.
            let key = (is_music, title.clone(), subtitle.clone());
            if state.track_key.as_ref() != Some(&key) {
                state.track_key = Some(key);
                let offset = if duration <= 0.0 || (0.0..=duration).contains(&current) { current } else { 0.0 };
                state.track_start = now_secs() - offset as i64;
            }
            let mut timestamps = Timestamps::new().start(state.track_start);
            if duration > 0.0 {
                timestamps = timestamps.end(state.track_start + duration as i64);
            }

            let thumbnail = info["thumbnail"].as_str().filter(|t| t.starts_with("http"));
            let large_image = match thumbnail {
                Some(t) if is_music => upsize_thumbnail(t),
                Some(t) => t.to_string(),
                None => (if is_music { ASSET_MUSIC } else { ASSET_YOUTUBE }).to_string(),
            };
            let assets = Assets::new()
                .large_image(large_image)
                .large_text(if is_music { "YouTube Music" } else { "YouTube" })
                .small_image(if playing { ASSET_PLAY } else { ASSET_PAUSE })
                .small_text(if playing { tr("rpc.playing") } else { tr("rpc.paused") });
            let mut activity = Activity::new()
                .activity_type(if is_music { ActivityType::Listening } else { ActivityType::Watching })
                .details(title)
                .state(subtitle)
                .assets(assets)
                .timestamps(timestamps);
            let url = info["url"].as_str().filter(|u| u.starts_with("http")).map(str::to_string);
            if let Some(url) = &url {
                let label = if is_music { tr("rpc.open_music") } else { tr("rpc.watch_youtube") };
                activity = activity.buttons(vec![Button::new(label, url.as_str())]);
            }
            client.set_activity(activity)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsizes_music_thumbnails_only() {
        assert_eq!(
            upsize_thumbnail("https://lh3.googleusercontent.com/abc=w60-h60-l90-rj"),
            "https://lh3.googleusercontent.com/abc=w544-h544-l90-rj"
        );
        assert_eq!(upsize_thumbnail("https://i.ytimg.com/vi/x/hqdefault.jpg"), "https://i.ytimg.com/vi/x/hqdefault.jpg");
    }

    #[test]
    fn truncates_long_text() {
        let long = "x".repeat(300);
        assert_eq!(truncate(Some(&long), "f").chars().count(), 128);
        assert_eq!(truncate(Some("  "), "Fallback"), "Fallback");
        assert_eq!(truncate(None, "Fallback"), "Fallback");
    }
}
