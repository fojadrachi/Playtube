//! Discord-Bot im Jam: schickt den Playtube-Ton (wie er aus der Seite kommt, js/audio_tap.js) an
//! einen Discord-Bot, der den Jam im Sprachkanal abspielt. Der Bot bittet über den Jam-Server darum
//! ("audioRequest" mit Adresse und Einmal-Ticket); Playtube verbindet sich dann per WebSocket und
//! schickt rohes PCM: 48 kHz, 16 Bit, Stereo, verschachtelt (Little Endian).

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

pub const RATE: u32 = 48_000;
/// Höchstens so viele Ton-Stücke warten auf das Netz (je ~10-40 ms) - sonst wird verworfen.
const QUEUE: usize = 64;

/// Geprüfte Anfrage des Bots (fremde Daten aus dem Jam).
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub url: String,
    pub ticket: String,
    pub bot: String,
}

pub fn parse_request(value: &Value) -> Option<Request> {
    let url = value["url"].as_str()?.trim();
    let ticket = value["ticket"].as_str()?;
    let url_ok = (url.starts_with("wss://") || url.starts_with("ws://"))
        && url.len() <= 260
        && !url.contains(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>'));
    let ticket_ok = (16..=64).contains(&ticket.len()) && ticket.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if !url_ok || !ticket_ok {
        return None;
    }
    let bot: String = value["bot"].as_str().unwrap_or("Discord-Bot").chars().take(40).collect();
    Some(Request { url: url.to_string(), ticket: ticket.to_string(), bot })
}

/// Bringt Stereo-Ton auf 48 kHz (linear; die Seite liefert meist schon 48 kHz).
pub fn resample(rate: u32, samples: &[i16]) -> Vec<i16> {
    if rate == RATE || rate == 0 || samples.len() < 4 {
        return samples.to_vec();
    }
    let frames_in = samples.len() / 2;
    let frames_out = ((frames_in as u64 * RATE as u64) / rate as u64) as usize;
    let mut out = Vec::with_capacity(frames_out * 2);
    for i in 0..frames_out {
        let pos = i as f64 * rate as f64 / RATE as f64;
        let a = (pos as usize).min(frames_in - 1);
        let b = (a + 1).min(frames_in - 1);
        let t = pos - a as f64;
        for ch in 0..2 {
            let x = samples[a * 2 + ch] as f64;
            let y = samples[b * 2 + ch] as f64;
            out.push((x + (y - x) * t).round() as i16);
        }
    }
    out
}

pub fn to_bytes(samples: &[i16]) -> Vec<u8> {
    samples.iter().flat_map(|s| s.to_le_bytes()).collect()
}

/// Was am Ende passiert ist (für die Anzeige).
#[derive(Debug)]
pub enum Ended {
    /// Bot hat das Ticket nicht angenommen (z. B. schon ein anderer PC, Ticket abgelaufen)
    Refused,
    /// Verbindung nicht möglich oder abgebrochen
    Failed(String),
    /// Bot hat beendet
    Closed,
}

/// Laufende Übertragung; endet beim Drop.
pub struct Stream {
    tx: mpsc::Sender<Vec<u8>>,
    task: tokio::task::JoinHandle<()>,
    /// Laufende Nummer (eine ersetzte Übertragung meldet ihr Ende mit ihrer eigenen)
    pub id: u64,
}

pub fn next_id() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl Stream {
    pub fn start(
        runtime: &tokio::runtime::Handle,
        id: u64,
        request: Request,
        name: String,
        on_started: impl FnOnce() + Send + 'static,
        on_end: impl FnOnce(Ended) + Send + 'static,
    ) -> Self {
        let (tx, rx) = mpsc::channel(QUEUE);
        let task = runtime.spawn(async move {
            let end = run(request, name, rx, on_started).await;
            on_end(end);
        });
        Self { tx, task, id }
    }

    /// Ton (48 kHz Stereo) anhängen; ist das Netz zu langsam, wird das Stück verworfen.
    pub fn push(&self, samples: &[i16]) {
        let _ = self.tx.try_send(to_bytes(samples));
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn run(request: Request, name: String, mut rx: mpsc::Receiver<Vec<u8>>, on_started: impl FnOnce()) -> Ended {
    let connect = tokio::time::timeout(Duration::from_secs(10), tokio_tungstenite::connect_async(request.url.as_str()));
    let socket = match connect.await {
        Ok(Ok((socket, _))) => socket,
        Ok(Err(e)) => return Ended::Failed(e.to_string()),
        Err(_) => return Ended::Failed("Zeitüberschreitung".into()),
    };
    let (mut write, mut read) = socket.split();
    let hello = json!({ "t": "hello", "ticket": request.ticket, "rate": RATE, "channels": 2, "name": name, "client": "playtube" });
    if write.send(Message::text(hello.to_string())).await.is_err() {
        return Ended::Failed("Senden fehlgeschlagen".into());
    }
    // Erst auf "ok" warten: der Bot nimmt nur einen PC und jedes Ticket nur einmal.
    let answer = tokio::time::timeout(Duration::from_secs(10), read.next()).await;
    match answer {
        Ok(Some(Ok(Message::Text(text)))) if serde_json::from_str::<Value>(text.as_str()).ok().is_some_and(|v| v["t"] == "ok") => {}
        _ => return Ended::Refused,
    }
    on_started();
    loop {
        tokio::select! {
            chunk = rx.recv() => {
                let Some(chunk) = chunk else { return Ended::Closed };
                if write.send(Message::binary(chunk)).await.is_err() {
                    return Ended::Failed("Verbindung verloren".into());
                }
            }
            incoming = read.next() => match incoming {
                Some(Ok(Message::Close(_))) | None => return Ended::Closed,
                Some(Err(e)) => return Ended::Failed(e.to_string()),
                _ => {}
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_requests_from_the_jam() {
        let ok = json!({ "t": "audioRequest", "url": "wss://bot.example.de/playtube/0", "ticket": "a".repeat(32), "bot": "ModBot 🎧" });
        assert_eq!(
            parse_request(&ok),
            Some(Request { url: "wss://bot.example.de/playtube/0".into(), ticket: "a".repeat(32), bot: "ModBot 🎧".into() })
        );
        for bad in [
            json!({ "url": "https://x.de", "ticket": "a".repeat(32) }),
            json!({ "url": "ws://x.de/\"><script>", "ticket": "a".repeat(32) }),
            json!({ "url": "ws://x.de", "ticket": "kurz" }),
            json!({ "url": "ws://x.de", "ticket": "a b".repeat(10) }),
            json!({ "ticket": "a".repeat(32) }),
        ] {
            assert_eq!(parse_request(&bad), None, "{bad}");
        }
    }

    #[test]
    fn resamples_to_48k() {
        let input: Vec<i16> = (0..441 * 2).map(|i| (i / 2) as i16).collect();
        let out = resample(44_100, &input);
        assert_eq!(out.len(), 480 * 2);
        assert_eq!(out[0], 0);
        assert!(out[out.len() - 2] >= 438 && out[out.len() - 2] <= 440);
        assert_eq!(resample(48_000, &[1, 2, 3, 4]), vec![1, 2, 3, 4]);
    }

    #[test]
    fn bytes_are_little_endian() {
        assert_eq!(to_bytes(&[1, -2]), vec![1, 0, 0xFE, 0xFF]);
    }

    #[tokio::test]
    async fn sends_hello_then_pcm_after_ok() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
            let hello = ws.next().await.unwrap().unwrap();
            let hello: Value = serde_json::from_str(hello.to_text().unwrap()).unwrap();
            assert_eq!(hello["ticket"], json!("t".repeat(20)));
            assert_eq!(hello["rate"], json!(48_000));
            ws.send(Message::text(r#"{"t":"ok"}"#)).await.unwrap();
            let data = ws.next().await.unwrap().unwrap();
            data.into_data().to_vec()
        });
        let runtime = tokio::runtime::Handle::current();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let request = Request { url: format!("ws://127.0.0.1:{port}"), ticket: "t".repeat(20), bot: "Bot".into() };
        let stream = Stream::start(&runtime, 1, request, "Test-PC".into(), move || { let _ = started_tx.send(()); }, |_| {});
        started_rx.await.unwrap();
        stream.push(&[1, -2]);
        assert_eq!(server.await.unwrap(), vec![1, 0, 0xFE, 0xFF]);
    }
}
