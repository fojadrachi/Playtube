//! Playtube - eigenständige Desktop-App für YouTube und YouTube Music (Windows).
//!
//! Aufbau: Slint-Fenster (Tab-Leiste, Navigation, Einstellungen) mit zwei WebView2-Kindfenstern
//! (browser.rs). Netzwerk-/Pipe-Server, Updates und Discord laufen im Hintergrund; alles, was
//! Fenster oder WebViews anfasst, läuft im UI-Thread (über `with_app`).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod autostart;
mod browser;
mod config;
mod diag;
mod discord;
mod i18n;
mod instance;
mod jam;
mod jam_ui;
mod lan;
mod music;
mod log;
mod relay;
mod remote;
mod scripts;
mod updater;
mod ytm;

use browser::{Tab, TabEvents};
use config::{Config, APP_NAME, VERSION};
use discord::{DiscordWorker, Presence};
use i18n::{tr, trf};
use remote::{Action, Dispatch, Reply};
use serde_json::Value;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

slint::include_modules!();

const TAB_SETTINGS: usize = 2;
const MEDIA_POLL: Duration = Duration::from_secs(2);
const REFRESH_AFTER_COMMAND: Duration = Duration::from_millis(300);

thread_local! {
    static APP: RefCell<Option<Rc<App>>> = const { RefCell::new(None) };
}

/// Führt `f` mit der App aus (nur im UI-Thread; vor dem Aufbau passiert nichts).
fn with_app(f: impl FnOnce(&Rc<App>)) {
    let app = APP.with(|a| a.borrow().clone());
    if let Some(app) = app {
        f(&app);
    }
}

/// Aus einem Hintergrund-Thread in den UI-Thread wechseln.
fn on_ui(f: impl FnOnce(&Rc<App>) + Send + 'static) {
    let _ = slint::invoke_from_event_loop(move || with_app(f));
}

struct TrayIds {
    show: tray_icon::menu::MenuId,
    play_pause: tray_icon::menu::MenuId,
    next: tray_icon::menu::MenuId,
    previous: tray_icon::menu::MenuId,
    quit: tray_icon::menu::MenuId,
}

struct App {
    ui: AppWindow,
    runtime: tokio::runtime::Runtime,
    tabs: Vec<Rc<Tab>>,
    _web_context: Box<wry::WebContext>,
    config: RefCell<Config>,
    current: Cell<usize>,
    fullscreen: Cell<bool>,
    media: RefCell<[Option<Value>; 2]>,
    discord: RefCell<Option<DiscordWorker>>,
    last_presence: RefCell<Option<Presence>>,
    pairing: lan::Pairing,
    lan: RefCell<Option<lan::LanServer>>,
    dispatch: Dispatch,
    audio_values: RefCell<Vec<String>>,
    update: RefCell<Option<updater::UpdateInfo>>,
    /// Ohne Setup eingespieltes Update (Version), aktiv ab dem nächsten Start.
    update_staged: RefCell<Option<String>>,
    tray: RefCell<Option<(TrayIcon, Menu, TrayIds)>>,
    timers: RefCell<Vec<slint::Timer>>,
    update_timer: slint::Timer,
    code_timer: slint::Timer,
    saved_timer: slint::Timer,
    jam: RefCell<Option<jam_ui::JamSession>>,
    jam_timer: slint::Timer,
    jam_results: RefCell<Vec<jam::SearchResult>>,
    music: RefCell<music::MusicState>,
    music_timer: slint::Timer,
    /// Google-Anmeldung läuft (YouTube-Music-Tab zeigt kurz die echte Seite)
    music_login: Cell<bool>,
    /// Playtube-Ton je Tab (YouTube, YouTube Music)
    relays: RefCell<[Option<relay::Relay>; 2]>,
}

struct Events;

impl TabEvents for Events {
    fn url_changed(&self, tab: usize, url: String) {
        with_app(|app| {
            *app.tabs[tab].url.borrow_mut() = url.clone();
            if tab == music::TAB_MUSIC {
                let url = url.clone();
                let _ = slint::invoke_from_event_loop(move || with_app(|app| app.music_url_changed(&url)));
            }
            if app.current.get() == tab && !app.music_active() {
                app.ui.set_url_text(url.into());
            }
        });
    }

    fn load_finished(&self, tab: usize) {
        // Die Freigabe der Gerätenamen nach jedem Seitenaufbau erneut setzen: auf der
        // YouTube-Startseite greift eine nur vorher erteilte Freigabe nicht.
        with_app(|app| {
            if app.audio_routing_wanted() {
                app.tabs[tab].set_device_names_allowed(true);
            }
            if tab == music::TAB_MUSIC {
                // Erst nach der WebView2-Meldung (dort darf nichts auf die WebView warten).
                let _ = slint::invoke_from_event_loop(|| with_app(|app| app.music_page_loaded()));
            }
            // Playtube-Ton nach jedem Seitenaufbau auf die aktuelle Einstellung bringen.
            let on = app.config.borrow().audio.stream_mode;
            app.tabs[tab].eval(&scripts::audio_tap_enable(on));
        });
    }

    fn fullscreen_changed(&self, _tab: usize, fullscreen: bool) {
        with_app(|app| app.set_fullscreen(fullscreen));
    }

    fn ipc(&self, tab: usize, body: String) {
        with_app(|app| app.on_tab_ipc(tab, &body));
    }

    fn open_url(&self, tab: usize, url: String) {
        // Erst nach dem Ereignis laden (WebView2 erlaubt im Handler keine Navigation).
        let _ = slint::invoke_from_event_loop(move || with_app(|app| app.tabs[tab].load(&url)));
    }
}

impl App {
    // ------------------------------------------------------------------ Fenster und Tabs

    fn layout(&self) {
        let window = self.ui.window();
        let scale = window.scale_factor() as f64;
        let size = window.size();
        let width = size.width as f64 / scale;
        let height = size.height as f64 / scale;
        let top = self.ui.get_content_top() as f64;
        let music_view = self.music_active();
        let current = self.current.get();
        for tab in &self.tabs {
            // Nicht gezeigte Tabs (und YouTube Music hinter der Playtube-Ansicht) bleiben für die
            // Edge-Engine "sichtbar", liegen aber außerhalb des Fensters - siehe select_tab.
            let away = (music_view && tab.index == music::TAB_MUSIC) || tab.index != current;
            let x = if away { -(width + 4000.0) } else { 0.0 };
            tab.set_bounds(x, top, width, height - top);
        }
    }

    fn select_tab(&self, index: usize) {
        self.current.set(index);
        self.ui.set_current_tab(index as i32);
        // Tabs nie verstecken, sondern nur aus dem Fenster schieben (layout): Eine versteckte Seite
        // startet keine neue Wiedergabe (Jam, Playtube-Ansicht, Playlist aus der Ferne) und hält im
        // Modus "Für Discord-Streams" ein für sie stummes Video an - das gab ständiges Play/Pause.
        for tab in &self.tabs {
            tab.set_visible(true);
        }
        self.layout();
        if index < self.tabs.len() {
            self.ui.set_url_text(self.tabs[index].url.borrow().clone().into());
            let _ = self.tabs[index].webview.focus();
        } else {
            self.ui.set_url_text(SharedString::new());
        }
        // Playtube-Ansicht: YouTube-Music-WebView verstecken, eigene Oberfläche zeigen.
        self.music_on_select();
        self.update_presence();
    }

    fn current_browser(&self) -> Option<&Rc<Tab>> {
        self.tabs.get(self.current.get())
    }

    fn show_window(&self) {
        let _ = self.ui.show();
        self.ui.window().set_minimized(false);
        self.layout();
        // Ein verstecktes Fenster kommt sonst hinter anderen Fenstern hoch.
        bring_to_front(&self.ui);
    }

    fn set_fullscreen(&self, fullscreen: bool) {
        self.fullscreen.set(fullscreen);
        self.ui.set_fullscreen_mode(fullscreen);
        self.ui.window().set_fullscreen(fullscreen);
        self.layout();
    }

    fn navigate(&self, text: &str) {
        if self.music_active() {
            self.music_search(text);
            return;
        }
        let text = text.trim();
        let Some(tab) = self.current_browser() else { return };
        if text.is_empty() {
            return;
        }
        let url = if text.contains(' ') || (!text.contains('.') && !text.contains("://")) {
            format!("https://www.google.com/search?q={}", urlencode(text))
        } else if text.contains("://") {
            text.to_string()
        } else {
            format!("https://{text}")
        };
        tab.load(&url);
    }

    fn quit(&self) {
        self.music_timer.stop();
        self.jam_timer.stop();
        self.jam.borrow_mut().take();
        self.lan.borrow_mut().take();
        *self.relays.borrow_mut() = [None, None];
        self.discord.borrow_mut().take();
        self.tray.borrow_mut().take();
        let _ = slint::quit_event_loop();
    }

    // ------------------------------------------------------------------ Wiedergabe

    fn poll_media(&self) {
        for tab in &self.tabs {
            let index = tab.index;
            tab.eval_with(scripts::MEDIA_PROBE, move |info| {
                if let Some(info) = info.filter(Value::is_object) {
                    on_ui(move |app| app.on_media(index, info));
                }
            });
        }
    }

    fn on_media(&self, index: usize, mut info: Value) {
        // Im Jam weiß Playtube selbst, was läuft: fehlt der Titel in der Seite, den des Jams nehmen.
        if index == music::TAB_MUSIC && info["title"].as_str().is_none_or(str::is_empty) {
            if let Some(item) = self.jam_state().and_then(|s| s.current) {
                info["title"] = Value::String(item.title.clone());
                if info["subtitle"].as_str().is_none_or(str::is_empty) && !item.artist.is_empty() {
                    info["subtitle"] = Value::String(item.artist.clone());
                }
            }
        }
        if index == music::TAB_MUSIC {
            self.music_on_media(&info);
        }
        self.media.borrow_mut()[index] = Some(info);
        let title = self.active_media().and_then(|m| m["title"].as_str().map(str::to_string)).filter(|t| !t.is_empty());
        self.ui.set_window_title(match title {
            Some(t) => format!("{t} – {APP_NAME}").into(),
            None => APP_NAME.into(),
        });
        self.update_presence();
    }

    /// Der Tab, der gerade abspielt, sonst der sichtbare.
    fn active_media(&self) -> Option<Value> {
        let media = self.media.borrow();
        media
            .iter()
            .flatten()
            .find(|m| m["playing"].as_bool().unwrap_or(false))
            .cloned()
            .or_else(|| media.get(self.current.get()).cloned().flatten())
    }

    fn update_presence(&self) {
        let discord = self.discord.borrow();
        let Some(worker) = discord.as_ref() else { return };
        let show_settings = self.current.get() == TAB_SETTINGS && self.config.borrow().discord.show_idle_presence;
        let presence = if show_settings {
            Presence::Settings
        } else {
            match self.active_media() {
                Some(info) if info["hasVideo"].as_bool().unwrap_or(false) => Presence::Media(info),
                _ => Presence::Idle,
            }
        };
        if self.last_presence.borrow().as_ref() == Some(&presence) {
            return;
        }
        *self.last_presence.borrow_mut() = Some(presence.clone());
        worker.show(presence);
    }

    fn start_discord(&self) {
        let config = self.config.borrow();
        let worker = config.discord.enabled.then(|| {
            DiscordWorker::start(
                config.discord.show_idle_presence,
            )
        });
        drop(config);
        *self.discord.borrow_mut() = worker;
        *self.last_presence.borrow_mut() = None;
        self.update_presence();
    }

    fn media_command(&self, tab: usize, command: &str, value: f64) {
        // Im Jam gehen Play/Pause/Weiter/Zurück/Spulen an den Jam statt an den Player.
        if self.jam_intercept(tab, command, value) {
            return;
        }
        if let Some(js) = scripts::control(command, value) {
            self.tabs[tab].eval(&js);
        }
        let timer = slint::Timer::default();
        timer.start(slint::TimerMode::SingleShot, REFRESH_AFTER_COMMAND, || with_app(|app| app.poll_media()));
        self.timers.borrow_mut().push(timer);
        self.timers.borrow_mut().retain(|t| t.running());
    }

    /// Ziel für Tray-Befehle: der abspielende Tab, sonst der sichtbare Browser-Tab.
    fn tray_command(&self, command: &str) {
        let media = self.media.borrow().clone();
        let name = remote::resolve_target("auto", self.visible_name(), &[media[0].as_ref(), media[1].as_ref()]);
        let index = remote::TAB_NAMES.iter().position(|n| *n == name).unwrap_or(0);
        self.media_command(index, command, 0.0);
    }

    fn visible_name(&self) -> &'static str {
        match self.current.get() {
            0 => "youtube",
            1 => "music",
            _ => "settings",
        }
    }

    // ------------------------------------------------------------------ Fernsteuerung

    fn remote(&self, action: Action, reply: Reply) {
        match action {
            Action::Status => {
                let media = self.media.borrow().clone();
                reply(remote::build_state(VERSION, self.visible_name(), &[media[0].as_ref(), media[1].as_ref()]));
            }
            Action::Show => {
                self.show_window();
                reply(Value::Null);
            }
            Action::SwitchTab(target) => {
                let index = match target.as_str() {
                    "youtube" => 0,
                    "music" => 1,
                    _ if self.current.get() == 0 => 1,
                    _ => 0,
                };
                self.select_tab(index);
                reply(Value::Null);
            }
            Action::Media { target, command, value } => {
                let media = self.media.borrow().clone();
                let name = remote::resolve_target(&target, self.visible_name(), &[media[0].as_ref(), media[1].as_ref()]);
                let index = remote::TAB_NAMES.iter().position(|n| *n == name).unwrap_or(0);
                self.media_command(index, &command, value);
                reply(Value::Null);
            }
            Action::ListPlaylists => {
                let reply = Mutex::new(Some(reply));
                self.tabs[1].eval_with(scripts::LIST_PLAYLISTS, move |result| {
                    if let Some(reply) = reply.lock().unwrap().take() {
                        reply(result.unwrap_or(Value::Null));
                    }
                });
            }
            Action::PlayPlaylist(id) => {
                self.tabs[1].load(&format!("{}{id}", scripts::MUSIC_PLAYLIST_URL));
                reply(Value::Null);
            }
            Action::Queue(target) => {
                let index = self.resolve_tab(&target);
                let reply = Mutex::new(Some(reply));
                self.tabs[index].eval_with(scripts::QUEUE, move |result| {
                    if let Some(reply) = reply.lock().unwrap().take() {
                        reply(result.unwrap_or(Value::Null));
                    }
                });
            }
            Action::PlayQueueItem { target, index } => {
                let tab = self.resolve_tab(&target);
                self.tabs[tab].eval(&scripts::play_queue_item(index));
                let timer = slint::Timer::default();
                timer.start(slint::TimerMode::SingleShot, std::time::Duration::from_millis(600), || {
                    with_app(|app| app.poll_media())
                });
                self.timers.borrow_mut().push(timer);
                self.timers.borrow_mut().retain(|t| t.running());
                reply(Value::Null);
            }
        }
    }

    fn resolve_tab(&self, target: &str) -> usize {
        let media = self.media.borrow().clone();
        let name = remote::resolve_target(target, self.visible_name(), &[media[0].as_ref(), media[1].as_ref()]);
        remote::TAB_NAMES.iter().position(|n| *n == name).unwrap_or(0)
    }

    // ------------------------------------------------------------------ Audio

    fn audio_routing_wanted(&self) -> bool {
        let config = self.config.borrow();
        !config.audio.youtube_output.is_empty() || !config.audio.music_output.is_empty()
    }

    fn apply_audio(&self) {
        let wanted = self.audio_routing_wanted();
        for tab in &self.tabs {
            tab.set_device_names_allowed(wanted);
        }
        let config = self.config.borrow();
        self.tabs[0].set_audio_output(&config.audio.youtube_output);
        self.tabs[1].set_audio_output(&config.audio.music_output);
        // Playtube-Ton: an/aus und Ausgabegerät je Tab
        let on = config.audio.stream_mode;
        let outputs = [config.audio.youtube_output.clone(), config.audio.music_output.clone()];
        drop(config);
        let mut relays = self.relays.borrow_mut();
        for (index, tab) in self.tabs.iter().enumerate() {
            tab.eval(&scripts::audio_tap_enable(on));
            if !on {
                relays[index] = None;
            } else if let Some(relay) = &relays[index] {
                relay.set_device(&outputs[index]);
            }
        }
    }

    /// Ton aus der Seite (js/audio_tap.js) über Playtube.exe ausgeben.
    fn on_tab_ipc(&self, tab: usize, body: &str) {
        if tab > 1 || !self.config.borrow().audio.stream_mode {
            return;
        }
        let Some((rate, samples)) = relay::parse_message(body) else { return };
        let mut relays = self.relays.borrow_mut();
        let relay = relays[tab].get_or_insert_with(|| {
            let config = self.config.borrow();
            relay::Relay::start(if tab == 0 { &config.audio.youtube_output } else { &config.audio.music_output })
        });
        relay.push(rate, &samples);
    }

    /// Füllt die Geräteauswahl: "Systemstandard", alle Geräte, ein gespeichertes aber gerade
    /// nicht angeschlossenes Gerät als "(nicht verfügbar)".
    fn fill_audio_devices(&self) {
        let config = self.config.borrow();
        let mut values = vec![String::new()];
        values.extend(audio::output_devices());
        let mut labels: Vec<SharedString> = values.iter().map(|v| v.as_str().into()).collect();
        labels[0] = tr("audio.system_default").into();
        for saved in [&config.audio.youtube_output, &config.audio.music_output] {
            if !saved.is_empty() && !values.contains(saved) {
                values.push(saved.clone());
                labels.push(trf("audio.unavailable", &[("name", saved)]).into());
            }
        }
        let index = |name: &str| values.iter().position(|v| v == name).unwrap_or(0) as i32;
        self.ui.set_youtube_output(index(&config.audio.youtube_output));
        self.ui.set_music_output(index(&config.audio.music_output));
        self.ui.set_audio_devices(ModelRc::new(VecModel::from(labels)));
        *self.audio_values.borrow_mut() = values;
    }

    // ------------------------------------------------------------------ Handy

    fn apply_lan(&self) {
        let (enabled, port) = {
            let config = self.config.borrow();
            (config.lan_remote.enabled, config.lan_remote.port)
        };
        if !enabled {
            self.lan.borrow_mut().take();
            self.ui.set_phone_running(false);
            self.ui.set_phone_status(tr("phone.off").into());
            self.clear_code();
        } else if self.lan.borrow().is_none() {
            match lan::LanServer::start(self.runtime.handle(), port, self.pairing.clone(), self.dispatch.clone()) {
                Ok(server) => {
                    *self.lan.borrow_mut() = Some(server);
                }
                Err(e) => {
                    log::line(&format!("lan: Start fehlgeschlagen: {e}"));
                    self.ui.set_phone_running(false);
                    self.ui.set_phone_status(trf("phone.not_running", &[("port", &port.to_string())]).into());
                    self.show_devices();
                    return;
                }
            }
        }
        if self.lan.borrow().is_some() {
            self.ui.set_phone_running(true);
            let addresses = lan::local_addresses().join(", ");
            self.ui.set_phone_status(trf("phone.address", &[("addresses", if addresses.is_empty() { "?" } else { &addresses })]).into());
        }
        self.show_devices();
    }

    fn show_devices(&self) {
        let devices = self.pairing.devices();
        let names: Vec<&str> = devices.iter().map(|d| d.name.as_str()).collect();
        self.ui.set_phone_has_devices(!devices.is_empty());
        self.ui.set_phone_devices(if names.is_empty() {
            tr("phone.no_devices").into()
        } else {
            trf("phone.devices", &[("names", &names.join(", "))]).into()
        });
    }

    fn pair_phone(&self) {
        if self.lan.borrow().is_none() {
            return;
        }
        let code = self.pairing.new_code();
        let shown = format!("{} {}", &code[..3], &code[3..]);
        self.ui.set_pairing_code(trf("phone.code", &[("code", &shown)]).into());
        self.code_timer.start(slint::TimerMode::SingleShot, lan::CODE_LIFETIME, || with_app(|app| app.clear_code()));
    }

    fn clear_code(&self) {
        self.code_timer.stop();
        self.ui.set_pairing_code(SharedString::new());
    }

    fn on_devices_changed(&self, devices: Vec<config::PairedDevice>) {
        self.config.borrow_mut().lan_remote.devices = devices;
        config::save(&self.config.borrow());
        self.clear_code();
        self.show_devices();
    }

    // ------------------------------------------------------------------ Diagnose

    /// Neue Logzeilen an den Server schicken. Automatisch nur, wenn die Option an ist und es
    /// seit dem letzten Bericht neue Zeilen gibt; `manual` = Knopf "Jetzt senden".
    fn send_diagnostics(&self, manual: bool) {
        let (body, latest, server) = {
            let mut config = self.config.borrow_mut();
            if !manual && !config.diagnostics.enabled {
                return;
            }
            let log_text = std::fs::read_to_string(diag::log_path()).unwrap_or_default();
            let (lines, latest) = diag::collect(&log_text, config.diagnostics.last_sent);
            // Automatisch nur mit neuen Zeilen; "Jetzt senden" geht immer raus (dann mit Vermerk).
            if lines.is_empty() && !manual {
                return;
            }
            if config.diagnostics.install_id.is_empty() {
                config.diagnostics.install_id = diag::new_install_id();
                config::save(&config);
            }
            (diag::report(&config.diagnostics.install_id, &lines, diag::note(manual, lines.is_empty())), latest, config.jam.server.clone())
        };
        if manual {
            self.ui.set_diag_busy(true);
            self.ui.set_diag_status(tr("diag.sending").into());
        }
        self.runtime.spawn(async move {
            let result = diag::send(&server, body).await.map_err(|e| format!("{e:#}"));
            on_ui(move |app| {
                app.ui.set_diag_busy(false);
                match result {
                    Ok(_) => {
                        let mut config = app.config.borrow_mut();
                        config.diagnostics.last_sent = latest;
                        config::save(&config);
                        if manual {
                            app.ui.set_diag_status(trf("diag.sent", &[("id", &config.diagnostics.install_id)]).into());
                        }
                    }
                    // Nicht ins Log schreiben – sonst schickt der nächste Versuch nur den eigenen Fehler.
                    Err(e) if manual => app.ui.set_diag_status(trf("diag.failed", &[("error", &e)]).into()),
                    Err(_) => {}
                }
            });
        });
    }

    // ------------------------------------------------------------------ Updates

    fn setup_update_timer(&self) {
        let config = self.config.borrow();
        self.update_timer.stop();
        if config.updates.enabled {
            let every = Duration::from_secs(config.updates.check_interval_hours.max(1) * 3600);
            self.update_timer.start(slint::TimerMode::Repeated, every, || with_app(|app| app.check_updates(false)));
        }
    }

    fn check_updates(&self, manual: bool) {
        self.ui.set_update_busy(true);
        self.ui.set_update_status(tr("updates.checking").into());
        self.runtime.spawn(async move {
            let result = updater::check().await;
            on_ui(move |app| app.on_update_checked(result, manual));
        });
    }

    fn on_update_checked(&self, result: anyhow::Result<updater::UpdateInfo>, manual: bool) {
        self.ui.set_update_busy(false);
        // Schon eingespielt, nur noch nicht neu gestartet.
        if let (Ok(info), Some(staged)) = (&result, self.update_staged.borrow().as_ref()) {
            if info.latest == *staged {
                self.ui.set_update_status(trf("updates.staged", &[("version", staged)]).into());
                return;
            }
        }
        match result {
            Ok(info) if info.available && self.config.borrow().updates.auto_install && updater::is_installed() && updater::is_minor(&info, VERSION) => {
                self.install_in_place(info);
            }
            Ok(info) if info.available => {
                let first_time = self.update.borrow().as_ref().map(|u| u.latest != info.latest).unwrap_or(true);
                self.ui.set_update_status(trf("updates.available", &[("version", &info.latest)]).into());
                self.ui.set_update_available(true);
                *self.update.borrow_mut() = Some(info);
                // Wie beim NNN Launcher: einmal pro neuer Version auf die Einstellungen hinweisen.
                if first_time && !manual {
                    log::line("update: neue Version gefunden");
                }
            }
            Ok(_) => {
                self.ui.set_update_available(false);
                self.ui.set_update_status(trf("updates.up_to_date", &[("version", VERSION)]).into());
            }
            Err(e) => {
                log::line(&format!("update: Prüfung fehlgeschlagen: {e:#}"));
                self.ui.set_update_status(tr("updates.failed").into());
            }
        }
    }

    /// Kleines Update still laden und einspielen; Playtube läuft dabei einfach weiter.
    fn install_in_place(&self, info: updater::UpdateInfo) {
        log::line(&format!("update: lade {} im Hintergrund", info.latest));
        self.runtime.spawn(async move {
            let result = updater::install_in_place(&info).await;
            on_ui(move |app| match result {
                Ok(()) => {
                    log::line(&format!("update: {} eingespielt, aktiv ab dem nächsten Start", info.latest));
                    app.ui.set_update_available(false);
                    app.ui.set_update_restart(true);
                    app.ui.set_update_status(trf("updates.staged", &[("version", &info.latest)]).into());
                    *app.update_staged.borrow_mut() = Some(info.latest.clone());
                }
                Err(e) => {
                    // Rückfall: wie bisher über das Setup anbieten.
                    log::line(&format!("update: Einspielen ohne Setup fehlgeschlagen: {e:#}"));
                    app.ui.set_update_status(trf("updates.available", &[("version", &info.latest)]).into());
                    app.ui.set_update_available(true);
                    *app.update.borrow_mut() = Some(info);
                }
            });
        });
    }

    fn restart_after_update(&self) {
        match updater::relaunch() {
            Ok(()) => self.quit(),
            Err(e) => self.ui.set_update_status(trf("updates.install_failed", &[("error", &format!("{e:#}"))]).into()),
        }
    }

    fn install_update(&self) {
        let Some(info) = self.update.borrow().clone() else { return };
        if !updater::is_installed() {
            self.ui.set_update_status(tr("updates.portable").into());
            let _ = open::that(updater::RELEASES_PAGE);
            return;
        }
        self.ui.set_update_busy(true);
        self.ui.set_update_progress(0.0);
        self.ui.set_update_status(tr("updates.downloading").into());
        self.runtime.spawn(async move {
            let progress = |percent: i32| {
                on_ui(move |app| app.ui.set_update_progress(percent as f32 / 100.0));
            };
            let result = updater::install(&info, progress).await;
            on_ui(move |app| match result {
                Ok(()) => {
                    app.ui.set_update_status(trf("updates.installing", &[("app", APP_NAME)]).into());
                    // Das Setup schließt Playtube selbst (/CLOSEAPPLICATIONS); kurz warten,
                    // damit Discord sauber beendet wird.
                    let timer = slint::Timer::default();
                    timer.start(slint::TimerMode::SingleShot, Duration::from_millis(600), || with_app(|app| app.quit()));
                    app.timers.borrow_mut().push(timer);
                }
                Err(e) => {
                    app.ui.set_update_busy(false);
                    app.ui.set_update_progress(-1.0);
                    app.ui.set_update_status(trf("updates.install_failed", &[("error", &format!("{e:#}"))]).into());
                }
            });
        });
    }

    // ------------------------------------------------------------------ Einstellungen

    fn load_settings_into_ui(&self) {
        let config = self.config.borrow();
        let ui = &self.ui;
        ui.set_discord_enabled(config.discord.enabled);
        ui.set_show_idle(config.discord.show_idle_presence);
        ui.set_updates_enabled(config.updates.enabled);
        ui.set_check_interval(config.updates.check_interval_hours as i32);
        ui.set_updates_auto(config.updates.auto_install);
        ui.set_phone_enabled(config.lan_remote.enabled);
        ui.set_diag_enabled(config.diagnostics.enabled);
        ui.set_audio_mode_index(if config.audio.stream_mode { 1 } else { 0 });
        ui.set_start_tab_index(if config.start_tab == "music" { 1 } else { 0 });
        ui.set_music_view_index(if config.music_view { 0 } else { 1 });
        ui.set_autostart(config.autostart);
        ui.set_autostart_tray(config.autostart_tray);
        let language = i18n::current();
        ui.set_language_index(i18n::LANGUAGES.iter().position(|(code, _)| *code == language).unwrap_or(0) as i32);
        drop(config);
        self.fill_audio_devices();
    }

    fn save_settings(&self) {
        let ui = &self.ui;
        let language_before = i18n::current();
        {
            let mut config = self.config.borrow_mut();
            config.discord.enabled = ui.get_discord_enabled();
            config.discord.show_idle_presence = ui.get_show_idle();
            config.updates.enabled = ui.get_updates_enabled();
            config.updates.check_interval_hours = ui.get_check_interval().clamp(1, 168) as u64;
            config.updates.auto_install = ui.get_updates_auto();
            let values = self.audio_values.borrow();
            config.audio.youtube_output = values.get(ui.get_youtube_output() as usize).cloned().unwrap_or_default();
            config.audio.music_output = values.get(ui.get_music_output() as usize).cloned().unwrap_or_default();
            config.lan_remote.enabled = ui.get_phone_enabled();
            config.diagnostics.enabled = ui.get_diag_enabled();
            config.audio.stream_mode = ui.get_audio_mode_index() == 1;
            config.start_tab = if ui.get_start_tab_index() == 1 { "music" } else { "youtube" }.into();
            config.music_view = ui.get_music_view_index() == 0;
            config.autostart = ui.get_autostart();
            config.autostart_tray = ui.get_autostart_tray();
            config.language = i18n::LANGUAGES.get(ui.get_language_index() as usize).map(|(c, _)| c.to_string()).unwrap_or_default();
            config::save(&config);
        }
        let language = i18n::set_language(&self.config.borrow().language);
        if language != language_before {
            apply_texts(&self.ui);
            self.rebuild_tray_texts();
            self.fill_audio_devices();
        }
        self.apply_audio();
        self.start_discord();
        self.apply_lan();
        self.setup_update_timer();
        autostart::apply(self.config.borrow().autostart);
        // Ansicht von YouTube Music sofort übernehmen
        self.select_tab(self.current.get());
        self.ui.set_saved_text(tr("settings.saved").into());
        self.saved_timer.start(slint::TimerMode::SingleShot, Duration::from_secs(4), || {
            with_app(|app| app.ui.set_saved_text(SharedString::new()))
        });
    }

    // ------------------------------------------------------------------ Tray

    fn build_tray(&self) {
        let ids = TrayIds {
            show: "show".into(),
            play_pause: "play_pause".into(),
            next: "next".into(),
            previous: "previous".into(),
            quit: "quit".into(),
        };
        let menu = Menu::new();
        let _ = menu.append_items(&[
            &MenuItem::with_id(ids.show.clone(), tr("tray.show"), true, None),
            &MenuItem::with_id(ids.play_pause.clone(), tr("tray.play_pause"), true, None),
            &MenuItem::with_id(ids.next.clone(), tr("tray.next"), true, None),
            &MenuItem::with_id(ids.previous.clone(), tr("tray.previous"), true, None),
            &PredefinedMenuItem::separator(),
            &MenuItem::with_id(ids.quit.clone(), tr("tray.quit"), true, None),
        ]);
        let icon = tray_icon::Icon::from_resource(1, None).ok();
        let mut builder = TrayIconBuilder::new()
            .with_tooltip(APP_NAME)
            .with_menu(Box::new(menu.clone()))
            .with_menu_on_left_click(false);
        if let Some(icon) = icon {
            builder = builder.with_icon(icon);
        }
        match builder.build() {
            Ok(tray) => *self.tray.borrow_mut() = Some((tray, menu, ids)),
            Err(e) => log::line(&format!("tray: {e}")),
        }
    }

    fn rebuild_tray_texts(&self) {
        self.tray.borrow_mut().take();
        self.build_tray();
    }

    fn poll_tray(&self) {
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            let action = {
                let tray = self.tray.borrow();
                let Some((_, _, ids)) = tray.as_ref() else { continue };
                if event.id == ids.show { "show" }
                else if event.id == ids.play_pause { "play_pause" }
                else if event.id == ids.next { "next" }
                else if event.id == ids.previous { "previous" }
                else if event.id == ids.quit { "quit" }
                else { continue }
            };
            match action {
                "show" => self.show_window(),
                "quit" => self.quit(),
                command => self.tray_command(command),
            }
        }
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                self.show_window();
            }
        }
    }
}

fn urlencode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            b' ' => "+".to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Holt das Fenster nach vorn (Windows erlaubt das nur dem Vordergrundprozess - ein kurzer
/// "TopMost"-Wechsel umgeht das für unser eigenes Fenster).
fn bring_to_front(ui: &AppWindow) {
    use slint::winit_030::WinitWindowAccessor;
    ui.window().with_winit_window(|w| {
        w.set_window_level(slint::winit_030::winit::window::WindowLevel::AlwaysOnTop);
        w.set_window_level(slint::winit_030::winit::window::WindowLevel::Normal);
        w.focus_window();
    });
}

fn apply_texts(ui: &AppWindow) {
    let app = [("app", APP_NAME)];
    ui.set_t(Texts {
        tab_settings: tr("tab.settings").into(),
        url_placeholder: tr("url.placeholder").into(),
        settings_title: trf("settings.title", &app).into(),
        version: trf("settings.version", &[("version", VERSION)]).into(),
        save: tr("settings.save").into(),
        section_discord: tr("section.discord").into(),
        section_updates: tr("section.updates").into(),
        section_audio: tr("section.audio").into(),
        section_phone: tr("section.phone").into(),
        section_general: tr("section.general").into(),
        field_enabled: tr("field.enabled").into(),
        field_check_interval: tr("field.check_interval").into(),
        field_youtube: tr("field.youtube").into(),
        field_music: tr("field.music").into(),
        field_start_tab: tr("field.start_tab").into(),
        field_language: tr("field.language").into(),
        field_music_view: tr("field.music_view").into(),
        field_autostart: tr("field.autostart").into(),
        field_autostart_tray: tr("field.autostart_tray").into(),
        music_view_playtube: tr("field.music_view.playtube").into(),
        music_view_original: tr("field.music_view.original").into(),
        discord_show_idle: tr("discord.show_idle").into(),
        updates_check_now: tr("updates.check_now").into(),
        updates_install: tr("updates.install").into(),
        updates_auto: tr("updates.auto").into(),
        updates_restart: tr("updates.restart").into(),
        audio_hint: tr("audio.hint").into(),
        audio_mode: tr("audio.mode").into(),
        audio_mode_normal: tr("audio.mode.normal").into(),
        audio_mode_stream: tr("audio.mode.stream").into(),
        audio_mode_hint: tr("audio.mode.hint").into(),
        phone_enabled: tr("phone.enabled").into(),
        phone_hint: trf("phone.hint", &app).into(),
        phone_pair: tr("phone.pair").into(),
        phone_forget: tr("phone.forget").into(),
        section_diag: tr("section.diag").into(),
        diag_enabled: tr("diag.enabled").into(),
        diag_hint: tr("diag.hint").into(),
        diag_send_now: tr("diag.send_now").into(),
    });
    jam_ui::apply_jam_texts(ui);
    music::apply_music_texts(ui);
}

fn show_error(message: &str) {
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    unsafe {
        MessageBoxW(None, &HSTRING::from(message), &HSTRING::from(APP_NAME), MB_OK | MB_ICONERROR);
    }
}

fn build_app(ui: &AppWindow, runtime: tokio::runtime::Runtime, config: Config) -> Result<Rc<App>, String> {
    let handle = ui.window().window_handle();
    let mut context = Box::new(wry::WebContext::new(Some(config::webview_dir())));
    let events: Rc<dyn TabEvents> = Rc::new(Events);
    let bounds = wry::Rect {
        position: wry::dpi::LogicalPosition::new(0, 88).into(),
        size: wry::dpi::LogicalSize::new(config.window.width.max(400), config.window.height.max(300)).into(),
    };
    let mut tabs = Vec::new();
    for (index, home) in [config.home_youtube.clone(), config.home_music.clone()].iter().enumerate() {
        let tab = Tab::new(&handle, &mut context, index, home, events.clone(), bounds, config.audio.stream_mode)
            .map_err(|e| format!("{}\n\n({e})", tr("error.webview")))?;
        tabs.push(tab);
    }

    let pairing = lan::Pairing::new(config.lan_remote.devices.clone(), |devices| {
        on_ui(move |app| app.on_devices_changed(devices));
    });
    let dispatch: Dispatch = Arc::new(|action, reply| on_ui(move |app| app.remote(action, reply)));

    Ok(Rc::new(App {
        ui: ui.clone_strong(),
        runtime,
        tabs,
        _web_context: context,
        config: RefCell::new(config),
        current: Cell::new(0),
        fullscreen: Cell::new(false),
        media: RefCell::new([None, None]),
        discord: RefCell::new(None),
        last_presence: RefCell::new(None),
        pairing,
        lan: RefCell::new(None),
        dispatch,
        audio_values: RefCell::new(Vec::new()),
        update: RefCell::new(None),
        update_staged: RefCell::new(None),
        tray: RefCell::new(None),
        timers: RefCell::new(Vec::new()),
        update_timer: slint::Timer::default(),
        code_timer: slint::Timer::default(),
        saved_timer: slint::Timer::default(),
        jam: RefCell::new(None),
        jam_timer: slint::Timer::default(),
        jam_results: RefCell::new(Vec::new()),
        music: RefCell::new(music::MusicState::default()),
        music_timer: slint::Timer::default(),
        music_login: Cell::new(false),
        relays: RefCell::new([None, None]),
    }))
}

fn start(ui: &AppWindow, runtime: tokio::runtime::Runtime, config: Config, jam_code: Option<String>) {
    let start_tab = if config.start_tab == "music" { 1 } else { 0 };
    let remote_enabled = config.remote_control.enabled;
    let updates_enabled = config.updates.enabled;
    let app = match build_app(ui, runtime, config) {
        Ok(app) => app,
        Err(message) => {
            log::line(&message);
            show_error(&message);
            let _ = slint::quit_event_loop();
            return;
        }
    };
    APP.with(|a| *a.borrow_mut() = Some(app.clone()));

    app.load_settings_into_ui();
    app.layout();
    app.select_tab(start_tab);
    app.apply_audio();
    app.start_discord();
    app.apply_lan();
    app.build_tray();
    app.setup_update_timer();
    app.jam_init_ui();
    // Eintrag mit dem aktuellen Pfad auffrischen; vom Autostart gestartet: auf Wunsch nur im Tray.
    let (autostart_on, tray_only) = {
        let config = app.config.borrow();
        (config.autostart, config.autostart_tray)
    };
    if autostart_on {
        autostart::apply(true);
    }
    if tray_only && autostart::started_by_autostart(&std::env::args().collect::<Vec<_>>()) {
        let _ = app.ui.hide();
    }

    // Ein zweiter Start holt dieses Fenster nach vorn (bzw. tritt einem Jam bei).
    instance::listen(app.runtime.handle(), |jam_code| {
        on_ui(move |app| match jam_code {
            Some(code) => app.jam_open_code(code),
            None => app.show_window(),
        })
    });
    // Gestartet über eine Einladung (playtube://jam/<code>).
    if let Some(code) = jam_code {
        app.jam_open_code(code);
    }
    // Stream-Dock-Plugin: eigener Pipe-Name pro Datenordner.
    if remote_enabled {
        let folder = config::data_dir().file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        app.runtime.spawn(remote::run_pipe_server(format!("{}{folder}", remote::PIPE_PREFIX), app.dispatch.clone()));
    }

    let media_timer = slint::Timer::default();
    media_timer.start(slint::TimerMode::Repeated, MEDIA_POLL, || with_app(|app| app.poll_media()));
    let tray_timer = slint::Timer::default();
    tray_timer.start(slint::TimerMode::Repeated, Duration::from_millis(100), || with_app(|app| app.poll_tray()));
    app.timers.borrow_mut().extend([media_timer, tray_timer]);

    // Diagnose: kurz nach dem Start (dann steht ein Absturz vom letzten Mal im Log) und alle 6 Stunden.
    let diag_timer = slint::Timer::default();
    diag_timer.start(slint::TimerMode::SingleShot, Duration::from_secs(30), || {
        with_app(|app| {
            app.send_diagnostics(false);
            let repeat = slint::Timer::default();
            repeat.start(slint::TimerMode::Repeated, Duration::from_secs(6 * 3600), || with_app(|app| app.send_diagnostics(false)));
            app.timers.borrow_mut().push(repeat);
        })
    });
    app.timers.borrow_mut().push(diag_timer);

    if updates_enabled {
        let timer = slint::Timer::default();
        timer.start(slint::TimerMode::SingleShot, Duration::from_secs(5), || with_app(|app| app.check_updates(false)));
        app.timers.borrow_mut().push(timer);
    }
}

fn main() {
    std::panic::set_hook(Box::new(|info| log::line(&format!("PANIC: {info}"))));

    let args: Vec<String> = std::env::args().collect();
    // Neustart nach einem Update: erst warten, bis die alte Instanz weg ist.
    updater::wait_for_previous(&args);
    updater::cleanup();
    let jam_code = jam::code_from_args(&args);
    if instance::notify_running(jam_code.as_deref()) {
        return;
    }
    let config = config::load();
    i18n::set_language(&config.language);

    let runtime = match tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            show_error(&format!("Tokio: {e}"));
            return;
        }
    };

    let ui = match AppWindow::new() {
        Ok(ui) => ui,
        Err(e) => {
            show_error(&format!("{e}"));
            return;
        }
    };
    apply_texts(&ui);
    ui.set_languages(ModelRc::new(VecModel::from(
        i18n::LANGUAGES.iter().map(|(_, name)| SharedString::from(*name)).collect::<Vec<_>>(),
    )));
    ui.window().set_size(slint::LogicalSize::new(config.window.width as f32, config.window.height as f32));

    ui.on_tab_selected(|index| with_app(|app| app.select_tab(index as usize)));
    ui.on_navigate(|text| with_app(|app| app.navigate(&text)));
    // In der Playtube-Ansicht bedienen Zurück/Vor/Neu laden/Start deren eigene Seiten.
    ui.on_go_back(|| with_app(|app| if app.music_active() { app.music_back() } else if let Some(t) = app.current_browser() { t.back() }));
    ui.on_go_forward(|| with_app(|app| if app.music_active() { app.music_forward() } else if let Some(t) = app.current_browser() { t.forward() }));
    ui.on_reload(|| with_app(|app| if app.music_active() { app.music_reload() } else if let Some(t) = app.current_browser() { t.reload() }));
    ui.on_go_home(|| with_app(|app| if app.music_active() { app.music_go(music::Target::Home) } else if let Some(t) = app.current_browser() { t.go_home() }));
    ui.on_geometry_changed(|| with_app(|app| app.layout()));
    ui.on_save_settings(|| with_app(|app| app.save_settings()));
    ui.on_check_updates(|| with_app(|app| app.check_updates(true)));
    ui.on_install_update(|| with_app(|app| app.install_update()));
    ui.on_restart_update(|| with_app(|app| app.restart_after_update()));
    ui.on_pair_phone(|| with_app(|app| app.pair_phone()));
    ui.on_forget_phones(|| with_app(|app| app.pairing.forget_all()));
    ui.on_send_diagnostics(|| with_app(|app| app.send_diagnostics(true)));
    jam_ui::wire(&ui);
    music::wire(&ui);

    // Schließen versteckt nur das Fenster (Musik läuft weiter); Beenden über das Tray-Icon.
    ui.window().on_close_requested(|| slint::CloseRequestResponse::HideWindow);

    if let Err(e) = ui.show() {
        show_error(&format!("{e}"));
        return;
    }
    // Die WebViews brauchen das native Fenster. Slint legt es erst an, wenn die Ereignisschleife
    // läuft - deshalb kurz warten, bis es ein gültiges Handle gibt.
    let ui_handle = ui.as_weak();
    let pending = RefCell::new(Some((runtime, config, jam_code)));
    let waiting = slint::Timer::default();
    let attempts = Cell::new(0);
    waiting.start(slint::TimerMode::Repeated, Duration::from_millis(30), move || {
        use wry::raw_window_handle::HasWindowHandle;
        if pending.borrow().is_none() {
            return;
        }
        let Some(ui) = ui_handle.upgrade() else { return };
        attempts.set(attempts.get() + 1);
        let ready = ui.window().window_handle().window_handle().is_ok();
        if !ready && attempts.get() < 200 {
            return;
        }
        if let Some((runtime, config, jam_code)) = pending.borrow_mut().take() {
            start(&ui, runtime, config, jam_code);
        }
    });

    let _ = slint::run_event_loop_until_quit();
    APP.with(|a| {
        if let Some(app) = a.borrow_mut().take() {
            app.quit();
        }
    });
}
