//! Verlauf: welche Songs/Videos gehört wurden (lokal in %APPDATA%\Playtube\history.json).
//! Ein Song zählt, sobald er 30 s lief (bei kürzeren Songs: die Hälfte) – wie bei Spotify.
//! Die Hördauer wird mitgezählt (Grundlage für einen späteren Jahresrückblick).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::time::Instant;

/// So viele Einträge bleiben gespeichert (die ältesten fallen weg).
pub const MAX_ENTRIES: usize = 5000;
const COUNT_AFTER_S: f64 = 30.0;
/// Längere Lücken zwischen zwei Abfragen (Ruhezustand, hängende Seite) zählen nicht als Hörzeit.
const MAX_STEP_S: f64 = 10.0;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HistoryEntry {
    pub video_id: String,
    pub title: String,
    pub artist: String,
    pub thumb: Option<String>,
    /// "music" oder "youtube"
    pub source: String,
    /// Beginn (Unix-Sekunden)
    pub played_at: i64,
    /// Gehörte Sekunden
    pub listened: u32,
}

struct Current {
    entry: HistoryEntry,
    duration: f64,
    listened: f64,
    /// Index in `entries`, sobald der Song gezählt wurde
    stored: Option<usize>,
}

#[derive(Default)]
pub struct History {
    pub entries: Vec<HistoryEntry>,
    current: Option<Current>,
    last_tick: Option<Instant>,
    path: Option<PathBuf>,
    /// Hördauer geändert, aber noch nicht gespeichert
    dirty: bool,
}

/// Video-ID aus einer YouTube-/YouTube-Music-Adresse (`?v=...`).
pub fn video_id(url: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    let id = query.split('&').find_map(|pair| pair.strip_prefix("v="))?;
    let id: String = id.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
    (id.len() >= 6).then_some(id)
}

impl History {
    pub fn load(path: PathBuf) -> Self {
        let entries = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<Vec<HistoryEntry>>(&text).ok())
            .unwrap_or_default();
        Self { entries, path: Some(path), ..Default::default() }
    }

    fn save(&self) {
        let Some(path) = &self.path else { return };
        if let Ok(text) = serde_json::to_string(&self.entries) {
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(&tmp, path);
            }
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.current = None;
        self.save();
    }

    /// Neuer Wiedergabestand. `playing` = (Quelle, Stand aus media_probe) des Tabs, der gerade
    /// abspielt, sonst `None`. Gibt `true` zurück, wenn sich die gespeicherte Liste geändert hat.
    pub fn observe(&mut self, playing: Option<(&str, &Value)>, now: Instant, unix_now: i64) -> bool {
        let step = self.last_tick.map(|t| now.duration_since(t).as_secs_f64()).unwrap_or(0.0).min(MAX_STEP_S);
        self.last_tick = Some(now);

        let Some((source, info)) = playing else { return self.finish_listening() };
        let Some(id) = info["url"].as_str().and_then(video_id) else { return false };
        let title = info["title"].as_str().unwrap_or("").trim().to_string();
        if title.is_empty() || !info["hasVideo"].as_bool().unwrap_or(false) {
            return false;
        }
        let duration = info["duration"].as_f64().unwrap_or(0.0);

        let mut changed = false;
        let same = self.current.as_ref().is_some_and(|c| c.entry.video_id == id);
        if !same {
            changed |= self.finish_listening();
            self.current = Some(Current {
                entry: HistoryEntry {
                    video_id: id,
                    title,
                    artist: info["subtitle"].as_str().unwrap_or("").trim().to_string(),
                    thumb: info["thumbnail"].as_str().map(str::to_string),
                    source: source.to_string(),
                    played_at: unix_now,
                    listened: 0,
                },
                duration,
                listened: 0.0,
                stored: None,
            });
            // Der erste Takt eines neuen Songs zählt noch nicht.
            return changed;
        }
        let current = self.current.as_mut().unwrap();
        current.listened += step;
        if duration > 0.0 {
            current.duration = duration;
        }
        let needed = if current.duration > 0.0 { COUNT_AFTER_S.min(current.duration / 2.0) } else { COUNT_AFTER_S };
        if current.stored.is_none() && current.listened >= needed {
            current.entry.listened = current.listened as u32;
            self.entries.push(current.entry.clone());
            current.stored = Some(self.entries.len() - 1);
            if self.entries.len() > MAX_ENTRIES {
                let drop = self.entries.len() - MAX_ENTRIES;
                self.entries.drain(..drop);
                current.stored = Some(self.entries.len() - 1);
            }
            self.save();
            return true;
        }
        if let Some(index) = current.stored {
            if let Some(entry) = self.entries.get_mut(index) {
                if entry.listened != current.listened as u32 {
                    entry.listened = current.listened as u32;
                    self.dirty = true;
                }
            }
        }
        changed
    }

    /// Wiedergabe pausiert/beendet: gezählte Hördauer speichern.
    /// (Pause vor 30 s: der Song bleibt "aktuell" und zählt beim Weiterhören weiter.)
    fn finish_listening(&mut self) -> bool {
        if self.dirty {
            self.dirty = false;
            self.save();
        }
        false
    }

    /// Neueste zuerst, optional nach Titel/Interpret gefiltert (ohne Groß-/Kleinschreibung).
    pub fn search(&self, query: &str, limit: usize) -> Vec<&HistoryEntry> {
        let query = query.trim().to_lowercase();
        self.entries
            .iter()
            .rev()
            .filter(|e| query.is_empty() || e.title.to_lowercase().contains(&query) || e.artist.to_lowercase().contains(&query))
            .take(limit)
            .collect()
    }
}

/// Unix-Sekunden jetzt.
pub fn unix_now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Lokales Datum und Uhrzeit (Jahr, Monat, Tag, Stunde, Minute) zu Unix-Sekunden.
#[cfg(not(windows))]
pub fn local_time(unix: i64) -> (i32, u32, u32, u32, u32) {
    let time = unix as libc::time_t;
    // SAFETY: localtime_r schreibt nur in `tm`.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&time, &mut tm) }.is_null() {
        return (1970, 1, 1, 0, 0);
    }
    (tm.tm_year + 1900, (tm.tm_mon + 1) as u32, tm.tm_mday as u32, tm.tm_hour as u32, tm.tm_min as u32)
}

#[cfg(windows)]
pub fn local_time(unix: i64) -> (i32, u32, u32, u32, u32) {
    use windows::Win32::Foundation::{FILETIME, SYSTEMTIME};
    use windows::Win32::System::Time::{FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime};
    // FILETIME: 100-ns-Schritte seit 1601.
    let ticks = (unix + 11_644_473_600) as u64 * 10_000_000;
    let file = FILETIME { dwLowDateTime: ticks as u32, dwHighDateTime: (ticks >> 32) as u32 };
    let mut utc = SYSTEMTIME::default();
    let mut local = SYSTEMTIME::default();
    unsafe {
        if FileTimeToSystemTime(&file, &mut utc).is_err() || SystemTimeToTzSpecificLocalTime(None, &utc, &mut local).is_err() {
            return (1970, 1, 1, 0, 0);
        }
    }
    (local.wYear as i32, local.wMonth as u32, local.wDay as u32, local.wHour as u32, local.wMinute as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Duration;

    fn info(id: &str, title: &str, duration: f64) -> Value {
        json!({ "url": format!("https://music.youtube.com/watch?v={id}&list=RD"), "title": title, "subtitle": "Band",
                "hasVideo": true, "duration": duration, "thumbnail": "https://i.ytimg.com/x.jpg" })
    }

    #[test]
    fn reads_video_ids() {
        assert_eq!(video_id("https://music.youtube.com/watch?v=abcDEF_-123&list=RD").as_deref(), Some("abcDEF_-123"));
        assert_eq!(video_id("https://www.youtube.com/watch?feature=x&v=dQw4w9WgXcQ").as_deref(), Some("dQw4w9WgXcQ"));
        assert_eq!(video_id("https://music.youtube.com/"), None);
    }

    #[test]
    fn counts_a_song_after_30_seconds_and_tracks_listening_time() {
        let mut history = History::default();
        let start = Instant::now();
        let song = info("song000001", "Lied", 200.0);
        assert!(!history.observe(Some(("music", &song)), start, 1000));
        let mut changed = false;
        for step in 1..=14 {
            changed |= history.observe(Some(("music", &song)), start + Duration::from_secs(2 * step), 1000);
        }
        assert!(!changed, "nach 28 s noch nicht gezählt");
        assert!(history.observe(Some(("music", &song)), start + Duration::from_secs(30), 1000));
        assert_eq!(history.entries.len(), 1);
        assert_eq!(history.entries[0].title, "Lied");
        assert_eq!(history.entries[0].artist, "Band");
        assert_eq!(history.entries[0].played_at, 1000);
        history.observe(Some(("music", &song)), start + Duration::from_secs(40), 1000);
        assert_eq!(history.entries[0].listened, 40);
        assert!(history.dirty, "Hördauer wartet aufs Speichern");
        history.observe(None, start + Duration::from_secs(41), 1000);
        assert!(!history.dirty, "beim Pausieren gespeichert");
        // Gleicher Song läuft weiter: kein zweiter Eintrag.
        history.observe(Some(("music", &song)), start + Duration::from_secs(42), 1000);
        assert_eq!(history.entries.len(), 1);
    }

    #[test]
    fn short_songs_count_after_half_and_skips_do_not_count() {
        let mut history = History::default();
        let t = Instant::now();
        let skipped = info("skip000001", "Weg", 180.0);
        history.observe(Some(("music", &skipped)), t, 0);
        history.observe(Some(("music", &skipped)), t + Duration::from_secs(5), 0);
        let short = info("short00001", "Kurz", 40.0);
        history.observe(Some(("music", &short)), t + Duration::from_secs(6), 0);
        history.observe(Some(("music", &short)), t + Duration::from_secs(16), 0);
        history.observe(Some(("music", &short)), t + Duration::from_secs(26), 0);
        assert_eq!(history.entries.iter().map(|e| e.title.as_str()).collect::<Vec<_>>(), vec!["Kurz"]);
    }

    #[test]
    fn pauses_and_gaps_do_not_count_as_listening() {
        let mut history = History::default();
        let t = Instant::now();
        let song = info("song000002", "Pause", 300.0);
        history.observe(Some(("music", &song)), t, 0);
        history.observe(Some(("music", &song)), t + Duration::from_secs(8), 0);
        history.observe(None, t + Duration::from_secs(10), 0);
        // Nach der Pause (und einer langen Lücke) geht es weiter: höchstens 10 s je Schritt.
        history.observe(Some(("music", &song)), t + Duration::from_secs(500), 0);
        assert!(history.entries.is_empty());
        for s in 1..=10 {
            history.observe(Some(("music", &song)), t + Duration::from_secs(500 + 2 * s), 0);
        }
        assert_eq!(history.entries.len(), 1);
        assert!(history.entries[0].listened < 60);
    }

    #[test]
    fn search_is_newest_first_and_filters() {
        let mut history = History::default();
        for (i, (title, artist)) in [("Alpha", "X"), ("Beta", "Y"), ("Gamma", "alphaband")].iter().enumerate() {
            history.entries.push(HistoryEntry { title: title.to_string(), artist: artist.to_string(), played_at: i as i64, ..Default::default() });
        }
        let titles: Vec<_> = history.search("", 10).iter().map(|e| e.title.clone()).collect();
        assert_eq!(titles, ["Gamma", "Beta", "Alpha"]);
        let titles: Vec<_> = history.search("ALPHA", 10).iter().map(|e| e.title.clone()).collect();
        assert_eq!(titles, ["Gamma", "Alpha"]);
    }

    #[test]
    fn keeps_at_most_max_entries() {
        let mut history = History::default();
        history.entries = (0..MAX_ENTRIES).map(|i| HistoryEntry { played_at: i as i64, ..Default::default() }).collect();
        let t = Instant::now();
        let song = info("song000003", "Neu", 100.0);
        for s in 0..=20 {
            history.observe(Some(("music", &song)), t + Duration::from_secs(2 * s), 99);
        }
        assert_eq!(history.entries.len(), MAX_ENTRIES);
        assert_eq!(history.entries.last().unwrap().title, "Neu");
        assert_eq!(history.entries[0].played_at, 1);
    }
}
