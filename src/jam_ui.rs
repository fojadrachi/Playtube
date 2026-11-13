//! Playtube Jam in der App: Jam-Tab (Slint), Verbindung, Abgleich des YouTube-Music-Tabs mit
//! dem Jam und Umleitung der Wiedergabe-Befehle (Tray, Stream Dock, Handy) an den Jam.
//! Protokoll und reine Sync-Logik stehen in `jam.rs`.

use super::*;
use crate::jam::{self, Step};
use serde_json::json;
use std::collections::HashSet;
use std::time::Instant;

/// Tab-Index der Jam-Seite (nach YouTube, YouTube Music, Einstellungen).
pub const TAB_JAM: usize = 3;
/// Der Jam spielt im YouTube-Music-Tab.
const JAM_TAB: usize = 1;
const SYNC_EVERY: Duration = Duration::from_millis(700);
/// Spielt ein Titel so lange nicht an, gilt er hier als nicht abspielbar.
const STUCK_LIMIT: Duration = Duration::from_secs(15);

pub struct JamSession {
    code: String,
    connection: jam::Connection,
    state: Option<jam::JamState>,
    clock: jam::Clock,
    unplayable: HashSet<String>,
    reported: HashSet<String>,
    /// Bis dahin sind Zustandswechsel des Players unsere eigenen Befehle.
    expect_until: Instant,
    pending_load: Option<(String, Instant)>,
    stuck_since: Option<(String, Instant)>,
    diverge: u32,
    last_duration: f64,
    /// Song, der beim Beitreten im YouTube-Music-Tab lief (zählt nicht als Wunsch) - beim
    /// Starten eines eigenen Jams bleibt das leer, dann wird der laufende Song der erste.
    baseline: Option<Option<String>>,
    /// Schon an den Jam geschickte Wünsche (bis der Server sie bestätigt).
    picked: HashSet<String>,
}

fn status_text(key: &str) -> String {
    tr(key).to_string()
}

fn error_text(error: &str) -> String {
    let key = format!("jam.error.{error}");
    let text = tr(&key);
    if text == "?" { trf("jam.error.other", &[("error", error)]) } else { text.to_string() }
}

fn ended_text(reason: &str) -> String {
    match reason {
        "not_found" => status_text("jam.status.not_found"),
        other => {
            let text = tr(&format!("jam.ended.{other}"));
            if text == "?" { status_text("jam.ended.host_left") } else { text.to_string() }
        }
    }
}

fn default_name() -> String {
    std::env::var("USERNAME").unwrap_or_default()
}

pub fn qr_image(text: &str) -> slint::Image {
    let Ok(code) = qrcode::QrCode::new(text.as_bytes()) else { return slint::Image::default() };
    let width = code.width();
    let colors = code.to_colors();
    let scale = 4;
    let size = width * scale;
    let mut buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(size as u32, size as u32);
    let pixels = buffer.make_mut_slice();
    for y in 0..size {
        for x in 0..size {
            let dark = colors[(y / scale) * width + x / scale] == qrcode::Color::Dark;
            let v = if dark { 0 } else { 255 };
            pixels[y * size + x] = slint::Rgba8Pixel { r: v, g: v, b: v, a: 255 };
        }
    }
    slint::Image::from_rgba8(buffer)
}

/// Text in die Windows-Zwischenablage legen.
pub fn set_clipboard(text: &str) -> bool {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    const CF_UNICODETEXT: u32 = 13;
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        if OpenClipboard(None).is_err() {
            return false;
        }
        let _ = EmptyClipboard();
        let ok = (|| -> windows::core::Result<()> {
            let memory = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2)?;
            let target = GlobalLock(memory) as *mut u16;
            if target.is_null() {
                return Err(windows::core::Error::from_thread());
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), target, wide.len());
            let _ = GlobalUnlock(memory);
            SetClipboardData(CF_UNICODETEXT, Some(HANDLE(memory.0)))?;
            Ok(())
        })()
        .is_ok();
        let _ = CloseClipboard();
        ok
    }
}

pub fn apply_jam_texts(ui: &AppWindow) {
    ui.set_jt(JamTexts {
        tab: tr("tab.jam").into(),
        title: tr("jam.title").into(),
        intro: tr("jam.intro").into(),
        name: tr("jam.name").into(),
        start: tr("jam.start").into(),
        join: tr("jam.join").into(),
        code_placeholder: tr("jam.code_placeholder").into(),
        invite: tr("jam.invite").into(),
        copy: tr("jam.copy").into(),
        all_control: tr("jam.all_control").into(),
        end: tr("jam.end").into(),
        leave: tr("jam.leave").into(),
        members: tr("jam.members").into(),
        queue: tr("jam.queue").into(),
        queue_empty: tr("jam.queue_empty").into(),
        add: tr("jam.add").into(),
        add_placeholder: tr("jam.add_placeholder").into(),
        add_current: tr("jam.add_current").into(),
        results: tr("jam.results").into(),
        nothing: tr("jam.nothing").into(),
        hint: tr("jam.hint").into(),
        kick: tr("jam.kick").into(),
        play_now: tr("jam.play_now").into(),
        remove: tr("jam.remove").into(),
        add_next: tr("jam.add_next").into(),
        cant_play_here: tr("jam.cant_play_here").into(),
    });
}

/// Verbindet die Jam-Callbacks des Fensters.
pub fn wire(ui: &AppWindow) {
    ui.on_jam_start(|| with_app(|app| app.jam_start()));
    ui.on_jam_join(|| with_app(|app| app.jam_join()));
    ui.on_jam_leave(|| with_app(|app| app.jam_leave()));
    ui.on_jam_end(|| with_app(|app| app.jam_send(json!({ "t": "end" }))));
    ui.on_jam_copy_link(|| with_app(|app| app.jam_copy_link()));
    ui.on_jam_play_pause(|| with_app(|app| app.jam_play_pause()));
    ui.on_jam_next(|| with_app(|app| app.jam_send(json!({ "t": "skip" }))));
    ui.on_jam_previous(|| with_app(|app| app.jam_send(json!({ "t": "previous" }))));
    ui.on_jam_seek(|fraction| with_app(|app| app.jam_seek_fraction(fraction as f64)));
    ui.on_jam_add(|| with_app(|app| app.jam_add()));
    ui.on_jam_add_current(|| with_app(|app| app.jam_add_current()));
    ui.on_jam_add_result(|index, next| with_app(|app| app.jam_add_result(index as usize, next)));
    ui.on_jam_remove(|id| with_app(|app| app.jam_send(json!({ "t": "remove", "itemId": id.as_str() }))));
    ui.on_jam_jump(|id| with_app(|app| app.jam_send(json!({ "t": "jump", "itemId": id.as_str() }))));
    ui.on_jam_kick(|id| with_app(|app| app.jam_send(json!({ "t": "kick", "memberId": id.as_str() }))));
    ui.on_jam_set_all_control(|on| with_app(|app| app.jam_send(json!({ "t": "settings", "allControl": on }))));
}

impl App {
    pub fn jam_init_ui(&self) {
        let name = {
            let config = self.config.borrow();
            if config.jam.name.trim().is_empty() { default_name() } else { config.jam.name.clone() }
        };
        self.ui.set_jam_name(name.into());
        self.jam_update_add_field(false);
    }

    fn jam_server(&self) -> String {
        self.config.borrow().jam.server.clone()
    }

    fn jam_set_status(&self, text: impl Into<SharedString>) {
        self.ui.set_jam_status(text.into());
    }

    /// Name aus dem Feld übernehmen und speichern; `None`, wenn leer.
    fn jam_take_name(&self) -> Option<String> {
        let name = self.ui.get_jam_name().trim().to_string();
        if name.is_empty() {
            self.jam_set_status(status_text("jam.status.need_name"));
            return None;
        }
        let mut config = self.config.borrow_mut();
        if config.jam.name != name {
            config.jam.name = name.clone();
            config::save(&config);
        }
        Some(name)
    }

    pub fn jam_start(&self) {
        if self.jam.borrow().is_some() {
            return;
        }
        let Some(name) = self.jam_take_name() else { return };
        self.ui.set_jam_busy(true);
        self.jam_set_status(status_text("jam.status.creating"));
        let server = self.jam_server();
        self.runtime.spawn(async move {
            let result = jam::create(&server, &name).await;
            on_ui(move |app| {
                app.ui.set_jam_busy(false);
                match result {
                    Ok(created) => app.jam_connect(&created.code, Some(created.token), true),
                    Err(e) => {
                        log::line(&format!("jam: Anlegen fehlgeschlagen: {e:#}"));
                        app.jam_set_status(status_text("jam.status.failed"));
                    }
                }
            });
        });
    }

    pub fn jam_join(&self) {
        let Some(code) = jam::normalize_code(&self.ui.get_jam_code_input()) else {
            self.jam_set_status(status_text("jam.status.bad_code"));
            return;
        };
        self.jam_join_code(code);
    }

    fn jam_join_code(&self, code: String) {
        if let Some(session) = self.jam.borrow().as_ref() {
            if session.code != code {
                self.jam_set_status(status_text("jam.status.in_other"));
            }
            return;
        }
        if self.jam_take_name().is_none() {
            return;
        }
        self.ui.set_jam_busy(true);
        self.jam_set_status(status_text("jam.status.joining"));
        let server = self.jam_server();
        self.runtime.spawn(async move {
            let result = jam::exists(&server, &code).await;
            on_ui(move |app| {
                app.ui.set_jam_busy(false);
                match result {
                    Ok(true) => {
                        let token = {
                            let config = app.config.borrow();
                            (config.jam.last_code == code && !config.jam.last_token.is_empty()).then(|| config.jam.last_token.clone())
                        };
                        app.jam_connect(&code, token, false);
                    }
                    Ok(false) => app.jam_set_status(status_text("jam.status.not_found")),
                    Err(e) => {
                        log::line(&format!("jam: Beitreten fehlgeschlagen: {e:#}"));
                        app.jam_set_status(status_text("jam.status.failed"));
                    }
                }
            });
        });
    }

    /// Beim Start: War Playtube noch in einem Jam (beendet, abgestürzt, Neustart nach Update), still
    /// mit demselben Schlüssel zurück - gleiche Rolle und Rechte wie vorher (Host bleibt Host).
    /// Gibt es den Jam nicht mehr, wird er vergessen.
    pub fn jam_resume(&self) {
        let (code, token) = {
            let config = self.config.borrow();
            (config.jam.last_code.clone(), config.jam.last_token.clone())
        };
        if code.is_empty() || token.is_empty() || self.jam.borrow().is_some() {
            return;
        }
        let server = self.jam_server();
        self.runtime.spawn(async move {
            let result = jam::exists(&server, &code).await;
            on_ui(move |app| match result {
                Ok(true) if app.jam.borrow().is_none() => {
                    log::line("jam: nach dem Start wieder im letzten Jam");
                    app.jam_connect(&code, Some(token), false);
                }
                Ok(false) => {
                    let mut config = app.config.borrow_mut();
                    config.jam.last_code.clear();
                    config.jam.last_token.clear();
                    config::save(&config);
                }
                // Offline o. Ä.: beim nächsten Start wieder versuchen.
                _ => {}
            });
        });
    }

    /// Einladung von außen (playtube://jam/<code>): Jam-Tab zeigen und beitreten.
    pub fn jam_open_code(&self, code: String) {
        self.show_window();
        self.select_tab(TAB_JAM);
        self.ui.set_jam_code_input(code.clone().into());
        self.jam_join_code(code);
    }

    fn jam_connect(&self, code: &str, token: Option<String>, host: bool) {
        let name = self.ui.get_jam_name().trim().to_string();
        {
            let mut config = self.config.borrow_mut();
            config.jam.last_code = code.to_string();
            config.jam.last_token = token.clone().unwrap_or_default();
            config::save(&config);
        }
        let server = self.jam_server();
        let connection = jam::Connection::start(self.runtime.handle(), &server, code, &name, token, |event| {
            on_ui(move |app| app.jam_event(event));
        });
        *self.jam.borrow_mut() = Some(JamSession {
            code: code.to_string(),
            connection,
            state: None,
            clock: jam::Clock::default(),
            unplayable: HashSet::new(),
            reported: HashSet::new(),
            expect_until: Instant::now(),
            pending_load: None,
            stuck_since: None,
            diverge: 0,
            last_duration: 0.0,
            baseline: if host { Some(None) } else { None },
            picked: HashSet::new(),
        });
        let link = jam::invite_url(&server, code);
        self.ui.set_jam_code(jam::pretty_code(code).into());
        self.ui.set_jam_link(link.clone().into());
        self.ui.set_jam_qr(qr_image(&link));
        self.ui.set_jam_active(true);
        self.ui.set_jam_has_current(false);
        self.ui.set_jam_members(ModelRc::default());
        self.ui.set_jam_queue(ModelRc::default());
        self.ui.set_jam_results(ModelRc::default());
        self.jam_results.borrow_mut().clear();
        self.jam_set_status(status_text("jam.status.joining"));
        self.jam_timer.start(slint::TimerMode::Repeated, SYNC_EVERY, || with_app(|app| app.jam_tick()));
    }

    /// Jam lokal beenden (Verbindung zu, Oberfläche zurück).
    fn jam_close(&self, status: String, forget: bool) {
        self.ui.global::<Music>().set_in_jam(false);
        self.ui.global::<Music>().set_queue(ModelRc::default());
        // Feinabgleich zurücknehmen: wieder normales Tempo.
        if let Some(script) = scripts::jam_rate(1.0) {
            self.tabs[JAM_TAB].eval(&script);
        }
        self.jam_timer.stop();
        self.jam.borrow_mut().take();
        self.ui.set_jam_active(false);
        self.ui.set_jam_busy(false);
        self.jam_set_status(status);
        self.ui.set_jam_results(ModelRc::default());
        if forget {
            let mut config = self.config.borrow_mut();
            config.jam.last_code.clear();
            config.jam.last_token.clear();
            config::save(&config);
        }
    }

    pub fn jam_leave(&self) {
        if self.jam.borrow().is_none() {
            return;
        }
        self.jam_send(json!({ "t": "leave" }));
        // Antwortet der Server nicht (offline), trotzdem nach kurzer Zeit schließen.
        let timer = slint::Timer::default();
        timer.start(slint::TimerMode::SingleShot, Duration::from_millis(800), || {
            with_app(|app| {
                if app.jam.borrow().is_some() {
                    app.jam_close(ended_text("left"), true);
                }
            })
        });
        self.timers.borrow_mut().push(timer);
    }

    pub fn jam_send(&self, msg: Value) {
        if let Some(session) = self.jam.borrow().as_ref() {
            session.connection.send(msg);
        }
    }

    fn jam_event(&self, event: jam::Event) {
        match event {
            jam::Event::Connected => self.jam_set_status(status_text("jam.status.connected")),
            jam::Event::Disconnected => {
                if self.jam.borrow().is_some() {
                    self.jam_set_status(status_text("jam.status.reconnecting"));
                }
            }
            jam::Event::Welcome { token, .. } => {
                let mut config = self.config.borrow_mut();
                config.jam.last_token = token;
                config::save(&config);
            }
            jam::Event::Pong { c, s, r } => {
                if let Some(session) = self.jam.borrow_mut().as_mut() {
                    session.clock.add(c, s, r);
                }
            }
            jam::Event::State(state) => {
                {
                    let mut borrowed = self.jam.borrow_mut();
                    let Some(session) = borrowed.as_mut() else { return };
                    session.clock.hint(state.server_now, jam::now_ms());
                    session.state = Some(*state);
                }
                self.jam_render();
                self.jam_tick();
            }
            jam::Event::Ended(reason) => {
                self.jam_close(ended_text(&reason), true);
                // Pausieren, damit nicht der letzte Jam-Titel einfach weiterläuft.
                self.media_command(JAM_TAB, "pause", 0.0);
            }
            jam::Event::Error(error) => self.jam_set_status(error_text(&error)),
        }
    }

    // ------------------------------------------------------------------ Bedienung

    pub(crate) fn jam_state(&self) -> Option<jam::JamState> {
        self.jam.borrow().as_ref().and_then(|s| s.state.clone())
    }

    fn jam_play_pause(&self) {
        if let Some(state) = self.jam_state() {
            self.jam_send(json!({ "t": if state.playback.paused { "play" } else { "pause" } }));
        }
    }

    fn jam_seek_fraction(&self, fraction: f64) {
        let duration = self.jam.borrow().as_ref().and_then(|s| {
            let item = s.state.as_ref()?.current.as_ref()?;
            item.dur.or((s.last_duration > 0.0).then_some(s.last_duration))
        });
        if let Some(duration) = duration {
            self.jam_send(json!({ "t": "seek", "pos": (fraction.clamp(0.0, 1.0) * duration).round() }));
        }
    }

    /// Wiedergabe-Befehle (Tray, Stream Dock, Handy) für den Jam-Tab an den Jam umleiten.
    /// `true` = erledigt, nicht an den Player weitergeben.
    pub fn jam_intercept(&self, tab: usize, command: &str, value: f64) -> bool {
        if tab != JAM_TAB || !matches!(command, "play_pause" | "play" | "pause" | "next" | "previous" | "seek") {
            return false;
        }
        let Some(state) = self.jam_state() else { return false };
        if !state.can_control {
            // Ohne Steuerrecht nichts tun - sonst springt der Abgleich gleich wieder zurück.
            return true;
        }
        match command {
            "play_pause" => self.jam_play_pause(),
            "play" | "pause" => self.jam_send(json!({ "t": command })),
            "next" => self.jam_send(json!({ "t": "skip" })),
            "previous" => self.jam_send(json!({ "t": "previous" })),
            _ => {
                let server_now = self.jam.borrow().as_ref().map(|s| s.clock.server_now(jam::now_ms())).unwrap_or(0.0);
                if let Some(want) = jam::target(&state, server_now) {
                    self.jam_send(json!({ "t": "seek", "pos": (want.pos + value).max(0.0) }));
                }
            }
        }
        true
    }

    fn jam_update_add_field(&self, search: bool) {
        self.ui.set_jam_add_placeholder(tr(if search { "jam.search_placeholder" } else { "jam.add_placeholder" }).into());
        self.ui.set_jam_add_label(tr(if search { "jam.search" } else { "jam.add" }).into());
    }

    fn jam_add(&self) {
        let text = self.ui.get_jam_add_input().trim().to_string();
        if text.is_empty() {
            return;
        }
        // Playlist-/Album-Link ohne einzelnes Video: alle Songs (löst der Jam-Server auf).
        if let Some(playlist_id) = jam::parse_playlist_id(&text).filter(|_| jam::parse_video_id(&text).is_none()) {
            self.jam_send(json!({ "t": "addPlaylist", "playlistId": playlist_id }));
            self.ui.set_jam_add_input(SharedString::new());
            self.jam_set_status(status_text("jam.playlist_adding"));
            return;
        }
        if let Some(video_id) = jam::parse_video_id(&text) {
            self.jam_send(json!({ "t": "add", "videoId": video_id }));
            self.ui.set_jam_add_input(SharedString::new());
            self.jam_set_status(trf("jam.added", &[("title", &video_id)]));
            return;
        }
        if !self.jam_state().is_some_and(|s| s.features.search) {
            self.jam_set_status(error_text("bad_video"));
            return;
        }
        let server = self.jam_server();
        self.runtime.spawn(async move {
            let result = jam::search(&server, &text).await;
            on_ui(move |app| match result {
                Ok(results) => {
                    let rows: Vec<JamItem> = results
                        .iter()
                        .map(|r| JamItem {
                            id: r.video_id.clone().into(),
                            title: r.title.clone().into(),
                            subtitle: r.artist.clone().into(),
                            can_jump: false,
                            can_remove: false,
                        })
                        .collect();
                    app.ui.set_jam_results(ModelRc::new(VecModel::from(rows)));
                    *app.jam_results.borrow_mut() = results;
                }
                Err(e) => {
                    log::line(&format!("jam: Suche fehlgeschlagen: {e:#}"));
                    app.jam_set_status(error_text("search"));
                }
            });
        });
    }

    fn jam_add_result(&self, index: usize, next: bool) {
        let Some(result) = self.jam_results.borrow().get(index).cloned() else { return };
        self.jam_send(json!({
            "t": "add", "videoId": result.video_id, "title": result.title, "artist": result.artist,
            "thumb": result.thumb, "next": next,
        }));
        self.jam_set_status(trf("jam.added", &[("title", &result.title)]));
    }

    /// Das Video, das gerade im YouTube-Tab offen ist, zum Jam hinzufügen.
    fn jam_add_current(&self) {
        let media = self.media.borrow()[0].clone();
        let video_id = media.as_ref().and_then(|m| m["url"].as_str()).and_then(jam::video_id_from_url);
        let (Some(media), Some(video_id)) = (media, video_id) else {
            self.jam_set_status(error_text("no_current"));
            return;
        };
        let title = media["title"].as_str().unwrap_or("").to_string();
        self.jam_send(json!({
            "t": "add", "videoId": video_id, "title": title,
            "artist": media["subtitle"].as_str().unwrap_or(""), "thumb": media["thumbnail"].as_str().unwrap_or(""),
        }));
        self.jam_set_status(trf("jam.added", &[("title", if title.is_empty() { &video_id } else { &title })]));
    }

    fn jam_copy_link(&self) {
        let link = self.ui.get_jam_link().to_string();
        if set_clipboard(&link) {
            self.jam_set_status(status_text("jam.copied"));
        }
    }

    // ------------------------------------------------------------------ Anzeige

    fn jam_render(&self) {
        let Some(state) = self.jam_state() else { return };
        self.ui.global::<Music>().set_in_jam(true);
        let item_subtitle = |item: &jam::Item| {
            let by = trf("jam.by", &[("name", &item.by_name)]);
            if item.artist.is_empty() { by } else { format!("{} · {by}", item.artist) }
        };
        let is_host = state.is_host();
        self.ui.set_jam_host(is_host);
        self.ui.set_jam_can_control(state.can_control);
        self.ui.set_jam_all_control(state.all_control);
        self.ui.set_jam_paused(state.playback.paused);
        self.ui.set_jam_has_current(state.current.is_some());
        if let Some(item) = &state.current {
            self.ui.set_jam_now_title(item.title.clone().into());
            self.ui.set_jam_now_artist(item_subtitle(item).into());
        }
        let members: Vec<JamMember> = state
            .members
            .iter()
            .map(|m| {
                let mut info = vec![tr(match m.client.as_str() {
                    "pc" => "jam.client.pc",
                    "android" => "jam.client.android",
                    _ => "jam.client.web",
                })
                .to_string()];
                if m.host {
                    info.insert(0, tr("jam.host").to_string());
                }
                if !m.online {
                    info.push(tr("jam.offline").to_string());
                }
                if m.cant_play {
                    info.push(tr("jam.cant_play").to_string());
                }
                let name = if m.id == state.you { format!("{} ({})", m.name, tr("jam.you")) } else { m.name.clone() };
                JamMember {
                    id: m.id.clone().into(),
                    initial: m.name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default().into(),
                    name: name.into(),
                    info: info.join(" · ").into(),
                    host: m.host,
                    online: m.online,
                    can_kick: is_host && m.id != state.you,
                }
            })
            .collect();
        self.ui.set_jam_members(ModelRc::new(VecModel::from(members)));
        let queue: Vec<JamItem> = state
            .queue
            .iter()
            .map(|q| JamItem {
                id: q.id.clone().into(),
                title: q.title.clone().into(),
                subtitle: item_subtitle(q).into(),
                can_jump: state.can_control,
                can_remove: state.can_control || q.by == state.you,
            })
            .collect();
        self.ui.set_jam_queue(ModelRc::new(VecModel::from(queue)));
        // Warteschlange der Playtube-Ansicht gleich mitziehen.
        if self.ui.global::<Music>().get_queue_open() {
            self.music_show_jam_queue(&state);
        }
        self.jam_update_add_field(state.features.search);
        let cant_play = {
            let session = self.jam.borrow();
            session.as_ref().zip(state.current.as_ref()).is_some_and(|(s, item)| s.unplayable.contains(&item.id))
        };
        self.ui.set_jam_cant_play(cant_play);
    }

    fn jam_render_progress(&self) {
        let session = self.jam.borrow();
        let Some(session) = session.as_ref() else { return };
        let Some(state) = session.state.as_ref() else { return };
        let Some(want) = jam::target(state, session.clock.server_now(jam::now_ms())) else {
            self.ui.set_jam_time("0:00".into());
            self.ui.set_jam_duration("0:00".into());
            self.ui.set_jam_progress(0.0);
            return;
        };
        let duration = want.dur.unwrap_or(session.last_duration);
        let pos = want.pos.max(0.0);
        let pos = if duration > 0.0 { pos.min(duration) } else { pos };
        self.ui.set_jam_time(jam::format_time(pos).into());
        self.ui.set_jam_duration(jam::format_time(duration).into());
        self.ui.set_jam_progress(if duration > 0.0 { (pos / duration) as f32 } else { 0.0 });
    }

    // ------------------------------------------------------------------ Abgleich

    /// Alle 700 ms: Player im YouTube-Music-Tab auslesen und angleichen.
    fn jam_tick(&self) {
        self.jam_render_progress();
        if self.jam.borrow().as_ref().is_none_or(|s| s.state.is_none()) {
            return;
        }
        self.tabs[JAM_TAB].eval_with(scripts::JAM_PROBE, |probe| {
            if std::env::var_os("PLAYTUBE_JAM_DEBUG").is_some() {
                log::line(&format!("jam-debug: {probe:?}"));
            }
            if let Some(probe) = probe.filter(Value::is_object) {
                on_ui(move |app| app.jam_sync(probe));
            }
        });
    }

    fn jam_sync(&self, probe: Value) {
        let now = Instant::now();
        let mut outgoing: Vec<Value> = Vec::new();
        let mut scripts_to_run: Vec<String> = Vec::new();
        let mut load: Option<String> = None;
        let mut rerender = false;
        let mut added: Option<String> = None;
        {
            let mut borrowed = self.jam.borrow_mut();
            let Some(session) = borrowed.as_mut() else { return };
            let Some(state) = session.state.as_ref() else { return };
            // Soll-Position zum Zeitpunkt der Messung im Tab (nicht erst nach der Antwort).
            let sampled = probe["now"].as_f64().filter(|t| (t - jam::now_ms()).abs() < 5_000.0).unwrap_or_else(jam::now_ms);
            let want = jam::target(state, session.clock.server_now(sampled));
            let local = jam::Local {
                video_id: probe["id"].as_str().filter(|s| !s.is_empty()).map(str::to_string),
                pos: probe["t"].as_f64().unwrap_or(0.0),
                paused: probe["paused"].as_bool().unwrap_or(true),
                rate: probe["rate"].as_f64().filter(|r| *r > 0.0).unwrap_or(1.0),
            };
            let duration = probe["d"].as_f64().unwrap_or(0.0);
            let ad = probe["ad"].as_bool().unwrap_or(false);
            let page_error = probe["error"].as_bool().unwrap_or(false);
            let matches = want.as_ref().is_some_and(|w| local.video_id.as_deref() == Some(w.video_id.as_str()));
            // Im YouTube-Music-Tab selbst einen Song gestartet: ab in den Jam (wie bei Spotify Jam).
            if session.baseline.is_none() {
                session.baseline = Some(local.video_id.clone());
            }
            let baseline = session.baseline.clone().flatten();
            if !probe["ad"].as_bool().unwrap_or(false) {
                if let Some(id) = jam::user_pick(state, want.as_ref(), &local, baseline.as_deref()) {
                    if session.picked.insert(id.clone()) {
                        let title = probe["title"].as_str().unwrap_or("").to_string();
                        outgoing.push(json!({
                            "t": "add", "videoId": id, "title": title, "artist": probe["artist"].as_str().unwrap_or(""),
                        }));
                        added = Some(if title.is_empty() { id } else { title });
                    }
                }
            }
            if matches {
                session.pending_load = None;
                if duration > 0.0 {
                    session.last_duration = duration;
                }
            }

            'sync: {
                let Some(w) = want.as_ref() else {
                    for step in jam::decide(None, &local, jam::TOLERANCE_S) {
                        if step == Step::Pause {
                            scripts_to_run.extend(Some(scripts::jam_play(false)));
                        }
                    }
                    break 'sync;
                };
                // Hier nicht abspielbar: still bleiben, die anderen hören weiter.
                if session.unplayable.contains(&w.item_id) {
                    if !local.paused {
                        scripts_to_run.extend(Some(scripts::jam_play(false)));
                    }
                    break 'sync;
                }
                // Werbung läuft: nicht dazwischenfunken.
                if ad {
                    break 'sync;
                }
                if matches {
                    if duration > 0.0 && w.dur.is_none() && session.reported.insert(format!("d{}", w.item_id)) {
                        outgoing.push(json!({ "t": "duration", "itemId": w.item_id, "dur": duration.round() }));
                    }
                    let ended = probe["ended"].as_bool().unwrap_or(false);
                    if w.pos > 5.0 && jam::reached_end(&local, duration, ended) && session.reported.insert(format!("e{}", w.item_id)) {
                        outgoing.push(json!({ "t": "ended", "itemId": w.item_id }));
                    }
                }

                // Mit Steuerrecht selbst im YouTube-Music-Tab pausiert/fortgesetzt: für alle übernehmen.
                if state.can_control && matches && now > session.expect_until && w.pos >= 1.0 && duration > 0.0 && local.pos < duration - 2.0 {
                    if local.paused != w.paused {
                        session.diverge += 1;
                    } else {
                        session.diverge = 0;
                    }
                    if session.diverge >= 2 {
                        session.diverge = 0;
                        session.expect_until = now + Duration::from_secs(3);
                        outgoing.push(json!({ "t": if local.paused { "pause" } else { "play" } }));
                        break 'sync;
                    }
                } else {
                    session.diverge = 0;
                }

                // Spielt der Titel hier einfach nicht an (gesperrt, Region, privat), überspringen.
                let should_play = !w.paused && w.pos >= 1.0;
                let not_playing = !matches || local.paused || page_error;
                if should_play && not_playing {
                    match &session.stuck_since {
                        Some((item, since)) if *item == w.item_id => {
                            let limit = if page_error { Duration::from_secs(3) } else { STUCK_LIMIT };
                            if now.duration_since(*since) > limit {
                                log::line(&format!("jam: Titel {} spielt hier nicht an, Player: {probe}", w.video_id));
                                session.unplayable.insert(w.item_id.clone());
                                outgoing.push(json!({ "t": "unplayable", "itemId": w.item_id }));
                                scripts_to_run.extend(Some(scripts::jam_play(false)));
                                rerender = true;
                                break 'sync;
                            }
                        }
                        _ => session.stuck_since = Some((w.item_id.clone(), now)),
                    }
                } else {
                    session.stuck_since = None;
                }

                for step in jam::decide(Some(w), &local, jam::TOLERANCE_S) {
                    match step {
                        Step::Load { video_id, .. } => {
                            let busy = session
                                .pending_load
                                .as_ref()
                                .is_some_and(|(id, since)| *id == video_id && now.duration_since(*since) < Duration::from_secs(12));
                            if !busy {
                                session.pending_load = Some((video_id.clone(), now));
                                session.expect_until = now + Duration::from_secs(8);
                                log::line(&format!("jam: lade {video_id}"));
                                load = Some(format!("https://music.youtube.com/watch?v={video_id}"));
                            }
                            // Nach dem Laden spult der nächste Abgleich an die richtige Stelle.
                            break;
                        }
                        Step::Seek(pos) => {
                            session.expect_until = now + Duration::from_secs(3);
                            scripts_to_run.extend(scripts::jam_seek(pos));
                        }
                        Step::Play => {
                            session.expect_until = now + Duration::from_secs(3);
                            scripts_to_run.extend(Some(scripts::jam_play(true)));
                        }
                        Step::Pause => {
                            session.expect_until = now + Duration::from_secs(3);
                            scripts_to_run.extend(Some(scripts::jam_play(false)));
                        }
                        Step::Rate(rate) => scripts_to_run.extend(scripts::jam_rate(rate)),
                    }
                }
            }
            for msg in outgoing.drain(..) {
                session.connection.send(msg);
            }
        }
        if let Some(url) = load {
            self.tabs[JAM_TAB].load(&url);
        }
        for script in scripts_to_run {
            self.tabs[JAM_TAB].eval(&script);
        }
        if rerender {
            self.jam_render();
        }
        if let Some(title) = added {
            self.jam_set_status(trf("jam.added", &[("title", &title)]));
        }
    }
}
