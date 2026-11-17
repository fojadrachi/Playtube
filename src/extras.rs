//! Verlauf (Seite in der Playtube-Ansicht), Schlaf-Timer und Mini-Player.

use super::*;
use crate::history::{self, History};
use crate::sleep::{self, SleepTimer, Step};
use crate::ytm::{Entry, Nav, Page, Shelf};
use serde_json::json;
use std::time::Instant;

/// So viele Einträge zeigt die Verlaufsseite höchstens.
const HISTORY_SHOWN: usize = 300;
/// Bildgröße für das Cover im Mini-Player (Schlüssel im Bild-Cache der Playtube-Ansicht).
const MINI_COVER_PX: u32 = 120;

pub fn history_path() -> std::path::PathBuf {
    config::data_dir().join("history.json")
}

pub fn load_history() -> History {
    History::load(history_path())
}

/// Auswahl für Menü und Playtube-Ansicht: (Minuten, Text). -1 = Songende, 0 = aus.
pub fn sleep_choices() -> Vec<(i32, String)> {
    let mut list: Vec<(i32, String)> =
        sleep::CHOICES.iter().map(|m| (*m as i32, format!("{m} {}", tr("sleep.minutes")))).collect();
    list.push((-1, tr("sleep.end").into()));
    list.push((0, tr("sleep.off").into()));
    list
}

pub fn apply_extras_texts(ui: &AppWindow) {
    let choices: Vec<SleepChoice> =
        sleep_choices().into_iter().map(|(minutes, label)| SleepChoice { minutes, label: label.into() }).collect();
    ui.global::<Music>().set_sleep_choices(ModelRc::new(VecModel::from(choices)));
}

impl App {
    // ------------------------------------------------------------------ Verlauf

    /// Neuer Wiedergabestand (aus on_media): gehörten Song zählen.
    pub fn history_observe(&self) {
        if !self.config.borrow().history {
            return;
        }
        let media = self.media.borrow().clone();
        let playing = (0..2).find(|&i| media[i].as_ref().is_some_and(|m| m["playing"].as_bool().unwrap_or(false)));
        let changed = {
            let info = playing.and_then(|i| media[i].as_ref().map(|m| (remote::TAB_NAMES[i], m)));
            self.history.borrow_mut().observe(info, Instant::now(), history::unix_now())
        };
        if changed {
            self.music_refresh_history();
        }
    }

    pub fn history_clear(&self) {
        self.history.borrow_mut().clear();
        self.music_refresh_history();
        self.ui.set_saved_text(tr("history.cleared").into());
    }

    /// Die Verlaufsseite: nach Tagen gruppiert, neueste zuerst.
    pub fn history_page(&self, query: &str) -> Page {
        let today = history::local_time(history::unix_now());
        let yesterday = history::local_time(history::unix_now() - 86_400);
        let history = self.history.borrow();
        let mut shelves: Vec<Shelf> = Vec::new();
        let mut last_day = None;
        for entry in history.search(query, HISTORY_SHOWN) {
            let (y, mo, d, h, mi) = history::local_time(entry.played_at);
            let day = (y, mo, d);
            if last_day != Some(day) {
                last_day = Some(day);
                let title = if day == (today.0, today.1, today.2) {
                    tr("history.today").to_string()
                } else if day == (yesterday.0, yesterday.1, yesterday.2) {
                    tr("history.yesterday").to_string()
                } else if i18n::current() == "en" {
                    format!("{mo}/{d}/{y}")
                } else {
                    format!("{d}.{mo}.{y}")
                };
                shelves.push(Shelf { title, entries: Vec::new(), rows: true });
            }
            let nav = Nav::Play { video_id: Some(entry.video_id.clone()), playlist_id: None };
            if let Some(shelf) = shelves.last_mut() {
                shelf.entries.push(Entry {
                    title: entry.title.clone(),
                    subtitle: entry.artist.clone(),
                    thumb: entry.thumb.clone(),
                    duration: Some(format!("{h:02}:{mi:02}")),
                    open: Some(nav.clone()),
                    play: Some(nav),
                    ..Default::default()
                });
            }
        }
        Page { title: tr("music.history").into(), shelves, ..Default::default() }
    }

    /// Die letzten `limit` Songs für die Fernsteuerung (Handy).
    pub fn history_json(&self, limit: usize) -> Value {
        let history = self.history.borrow();
        Value::Array(
            history
                .search("", limit)
                .into_iter()
                .map(|e| {
                    json!({
                        "videoId": e.video_id,
                        "title": e.title,
                        "artist": e.artist,
                        "thumbnail": e.thumb.as_deref().map(crate::discord::upsize_thumbnail),
                        "source": e.source,
                        "playedAt": e.played_at,
                        "listened": e.listened,
                    })
                })
                .collect(),
        )
    }

    // ------------------------------------------------------------------ Schlaf-Timer

    /// Video-ID des gerade laufenden Songs.
    fn sleep_current_video(&self) -> Option<String> {
        self.active_media()
            .filter(|m| m["playing"].as_bool().unwrap_or(false))
            .and_then(|m| m["url"].as_str().and_then(history::video_id))
    }

    /// Minuten; 0 = aus, -1 = am Ende des Songs.
    pub fn sleep_set(&self, minutes: i32) {
        self.sleep_restore_volume();
        let timer = match minutes {
            0 => None,
            -1 => Some(SleepTimer::end_of_track(self.sleep_current_video())),
            m => Some(SleepTimer::minutes(m.max(1) as u32, Instant::now())),
        };
        let active = timer.is_some();
        *self.sleep.borrow_mut() = timer;
        if active {
            self.sleep_ticker.start(slint::TimerMode::Repeated, Duration::from_secs(1), || with_app(|app| app.sleep_tick()));
        } else {
            self.sleep_ticker.stop();
        }
        self.sleep_render();
    }

    /// Restzeit für den Status der Fernsteuerung: Sekunden, -1 = Songende, null = aus.
    pub fn sleep_status(&self) -> Value {
        match self.sleep.borrow().as_ref() {
            Some(timer) => json!(timer.remaining_secs(Instant::now())),
            None => Value::Null,
        }
    }

    fn sleep_tick(&self) {
        let track = self.sleep_current_video();
        let step = match self.sleep.borrow_mut().as_mut() {
            Some(timer) => timer.tick(Instant::now(), track.as_deref()),
            None => return,
        };
        match step {
            Step::Wait => {}
            Step::Fade(factor) => {
                if self.sleep_fade.get().is_none() {
                    let media = self.media.borrow().clone();
                    let playing = (0..2).find(|&i| media[i].as_ref().is_some_and(|m| m["playing"].as_bool().unwrap_or(false)));
                    if let Some(i) = playing {
                        let volume = media[i].as_ref().and_then(|m| m["volume"].as_f64()).unwrap_or(100.0);
                        self.sleep_fade.set(Some((i, volume)));
                    }
                }
                if let Some((tab, volume)) = self.sleep_fade.get() {
                    if let Some(js) = scripts::control("set_volume", (volume * factor).round().max(1.0)) {
                        self.tabs[tab].eval(&js);
                    }
                }
            }
            Step::Stop => {
                *self.sleep.borrow_mut() = None;
                self.sleep_ticker.stop();
                // Im Jam nur für sich selbst aufhören: den Jam verlassen statt ihn für alle anzuhalten.
                if self.jam.borrow().is_some() {
                    self.jam_leave();
                }
                if let Some(js) = scripts::control("pause", 0.0) {
                    for tab in &self.tabs {
                        tab.eval(&js);
                    }
                }
                // Lautstärke erst nach dem Pausieren zurückstellen.
                let timer = slint::Timer::default();
                timer.start(slint::TimerMode::SingleShot, Duration::from_millis(1500), || {
                    with_app(|app| app.sleep_restore_volume())
                });
                self.timers.borrow_mut().push(timer);
                log::line("sleep: Schlaf-Timer abgelaufen, Wiedergabe pausiert");
            }
        }
        self.sleep_render();
    }

    fn sleep_restore_volume(&self) {
        if let Some((tab, volume)) = self.sleep_fade.take() {
            if let Some(js) = scripts::control("set_volume", volume) {
                self.tabs[tab].eval(&js);
            }
        }
    }

    fn sleep_text(&self) -> String {
        match self.sleep.borrow().as_ref() {
            Some(timer) => match timer.remaining_secs(Instant::now()) {
                -1 => tr("sleep.end_short").into(),
                secs => sleep::format_remaining(secs),
            },
            None => String::new(),
        }
    }

    fn sleep_render(&self) {
        let text = self.sleep_text();
        let music = self.ui.global::<Music>();
        music.set_sleep_active(!text.is_empty());
        music.set_sleep_text(text.clone().into());
        if let Some(mini) = self.mini.borrow().as_ref() {
            mini.set_sleep_text(text.into());
        }
    }

    // ------------------------------------------------------------------ Discord-Bot im Jam

    /// Ein Discord-Bot im Jam bittet um den Ton (Nachricht "audioRequest" vom Jam-Server).
    pub fn bot_audio_request(&self, value: &Value) {
        if !self.config.borrow().jam.bot_audio {
            log::line("bot: Ton-Anfrage abgelehnt (in den Einstellungen aus)");
            return;
        }
        let Some(request) = botaudio::parse_request(value) else {
            log::line("bot: ungültige Ton-Anfrage");
            return;
        };
        let name = {
            let config = self.config.borrow();
            if config.jam.name.trim().is_empty() { APP_NAME.to_string() } else { config.jam.name.clone() }
        };
        let bot = request.bot.clone();
        // Vorherige Übertragung ersetzen (z. B. der Bot hat neu gefragt).
        self.bot_audio.borrow_mut().take();
        let id = botaudio::next_id();
        let stream = botaudio::Stream::start(
            self.runtime.handle(),
            id,
            request,
            name,
            move || on_ui(move |app| app.bot_audio_started(&bot)),
            move |end| on_ui(move |app| app.bot_audio_ended(id, end)),
        );
        *self.bot_audio.borrow_mut() = Some(stream);
        // Ton-Abgriff einschalten (der Ton läuft dann über Playtube.exe).
        self.apply_audio();
    }

    fn bot_audio_started(&self, bot: &str) {
        log::line("bot: Ton geht an einen Discord-Bot");
        self.jam_set_status(trf("bot.listening", &[("bot", bot)]));
    }

    fn bot_audio_ended(&self, id: u64, end: botaudio::Ended) {
        // Nur die laufende Übertragung zählt (eine ersetzte meldet ihr Ende auch).
        if self.bot_audio.borrow().as_ref().is_none_or(|s| s.id != id) {
            return;
        }
        match &end {
            botaudio::Ended::Refused => log::line("bot: Bot hat abgelehnt (anderer PC oder Ticket abgelaufen)"),
            botaudio::Ended::Failed(e) => log::line(&format!("bot: Verbindung zum Bot fehlgeschlagen: {e}")),
            botaudio::Ended::Closed => log::line("bot: Bot hat beendet"),
        }
        let was_streaming = self.bot_audio.borrow_mut().take().is_some();
        if was_streaming {
            self.apply_audio();
            if !matches!(end, botaudio::Ended::Refused) && self.jam.borrow().is_some() {
                self.jam_set_status(tr("bot.stopped").to_string());
            }
        }
    }

    pub fn bot_audio_stop(&self) {
        if self.bot_audio.borrow_mut().take().is_some() {
            self.apply_audio();
        }
    }

    // ------------------------------------------------------------------ Mini-Player

    pub fn mini_toggle(&self) {
        if self.mini.borrow().is_some() {
            self.mini_close(true);
        } else {
            self.mini_open();
        }
    }

    fn mini_open(&self) {
        let mini = match MiniWindow::new() {
            Ok(mini) => mini,
            Err(e) => {
                log::line(&format!("mini: {e}"));
                return;
            }
        };
        #[cfg(target_os = "linux")]
        slint::ComponentHandle::global::<crate::Fonts>(&mini).set_icons(crate::linux::ICON_FONT.into());
        mini.set_idle_text(tr("mini.idle").into());
        mini.on_media(|command| with_app(|app| app.tray_command(command.as_str())));
        mini.on_expand(|| with_app(|app| app.mini_close(true)));
        // Fenster schließen = zurück zum großen Fenster.
        mini.window().on_close_requested(|| {
            let _ = slint::invoke_from_event_loop(|| with_app(|app| app.mini_close(true)));
            slint::CloseRequestResponse::KeepWindowShown
        });
        if let Err(e) = mini.show() {
            log::line(&format!("mini: {e}"));
            return;
        }
        *self.mini.borrow_mut() = Some(mini);
        let _ = self.ui.hide();
        self.mini_render();
        self.sleep_render();
    }

    fn mini_close(&self, show_main: bool) {
        if let Some(mini) = self.mini.borrow_mut().take() {
            let _ = mini.hide();
        }
        if show_main {
            self.show_window();
        }
    }

    /// Mini-Player auf den aktuellen Stand bringen (aus on_media und wenn ein Bild fertig ist).
    pub fn mini_render(&self) {
        let Some(mini) = self.mini.borrow().as_ref().map(|m| m.clone_strong()) else { return };
        let info = self.active_media().unwrap_or(Value::Null);
        let has_video = info["hasVideo"].as_bool().unwrap_or(false);
        let title = if has_video { info["title"].as_str().unwrap_or("") } else { "" };
        mini.set_song_title(title.into());
        mini.set_artist(if has_video { info["subtitle"].as_str().unwrap_or("") } else { "" }.into());
        mini.set_playing(info["playing"].as_bool().unwrap_or(false));
        mini.set_liked(info["likeStatus"].as_str() == Some("like"));
        let (pos, dur) = (info["currentTime"].as_f64().unwrap_or(0.0), info["duration"].as_f64().unwrap_or(0.0));
        mini.set_progress(if dur > 0.0 { (pos / dur) as f32 } else { 0.0 });
        let url = info["thumbnail"].as_str().filter(|_| has_video).map(|u| ytm::sized_thumb(u, MINI_COVER_PX));
        match url {
            Some(url) => match self.music_cached(&url) {
                Some(image) => {
                    mini.set_cover(image);
                    mini.set_has_cover(true);
                }
                None => {
                    mini.set_has_cover(false);
                    self.music_fetch_image(url);
                }
            },
            None => mini.set_has_cover(false),
        }
    }
}
