//! Playtube-Ansicht für YouTube Music (Spotify-Stil, ui/music.slint). Daten kommen über
//! [`crate::ytm`] mit dem Login des YouTube-Music-Tabs; abgespielt wird im echten
//! YouTube-Music-Player, der dabei versteckt im Hintergrund läuft (Premium bleibt erhalten).

use super::*;
use crate::ytm::{self, Entry, Nav, Page, PageType};
use slint::{Image, Model, Rgba8Pixel, SharedPixelBuffer};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

/// Der YouTube-Music-Tab.
pub const TAB_MUSIC: usize = 1;
/// Google-Anmeldung für YouTube Music; danach leitet Google zurück zu music.youtube.com.
const LOGIN_URL: &str = "https://accounts.google.com/ServiceLogin?service=youtube&passive=true&continue=https%3A%2F%2Fmusic.youtube.com%2F";
const CARD_PX: u32 = 226;
const ROW_PX: u32 = 120;
const HEADER_PX: u32 = 300;
const QUEUE_EVERY: Duration = Duration::from_secs(3);

/// Liest die Einstellungen des eingeloggten YouTube Music aus der Seite (für die Anfragen).
const YTCFG_JS: &str = "(function(){try{var g=function(k){return (window.ytcfg&&ytcfg.get)?ytcfg.get(k):null;};\
return JSON.stringify({INNERTUBE_CLIENT_VERSION:g('INNERTUBE_CLIENT_VERSION'),VISITOR_DATA:g('VISITOR_DATA'),\
HL:g('HL'),GL:g('GL'),SESSION_INDEX:g('SESSION_INDEX'),DELEGATED_SESSION_ID:g('DELEGATED_SESSION_ID'),\
DATASYNC_ID:g('DATASYNC_ID'),USER_SESSION_ID:g('USER_SESSION_ID')});}catch(e){return '{}';}})();";

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Home,
    Library,
    Search(String),
    Browse { id: String, params: Option<String>, page: PageType },
}

#[derive(Default)]
pub struct MusicState {
    auth: Option<ytm::Auth>,
    auth_pending: Option<Target>,
    history: Vec<Target>,
    position: usize,
    generation: u64,
    page: Option<Page>,
    entries: Vec<Entry>,
    sidebar: Vec<Entry>,
    sidebar_loaded: bool,
    /// Bild-Adresse je Karte (gleiche Reihenfolge wie die Modelle)
    shelf_models: Vec<Rc<VecModel<MusicCard>>>,
    shelf_urls: Vec<Vec<Option<String>>>,
    sidebar_model: Option<Rc<VecModel<MusicCard>>>,
    sidebar_urls: Vec<Option<String>>,
    queue_model: Option<Rc<VecModel<MusicCard>>>,
    queue_urls: Vec<Option<String>>,
    page_url: Option<String>,
    now_url: Option<String>,
    images: HashMap<String, SharedPixelBuffer<Rgba8Pixel>>,
    loading: HashSet<String>,
    /// Zuletzt gelesener Stand des Players (für flüssigen Fortschritt)
    now_video: Option<String>,
    position_s: f64,
    duration_s: f64,
    playing: bool,
    polled_at: Option<Instant>,
    last_queue: Option<Instant>,
}

fn texts() -> MusicTexts {
    MusicTexts {
        home: tr("music.home").into(),
        search: tr("music.search").into(),
        library: tr("music.library").into(),
        your_playlists: tr("music.your_playlists").into(),
        liked: tr("music.liked").into(),
        play: tr("music.play").into(),
        queue: tr("music.queue").into(),
        queue_empty: tr("music.queue_empty").into(),
        login: tr("music.login").into(),
        loading: tr("music.loading").into(),
        login_hint: tr("music.login_hint").into(),
        nothing: tr("music.nothing").into(),
        top_result: tr("music.top_result").into(),
        results: tr("music.results").into(),
        search_placeholder: tr("music.search_placeholder").into(),
    }
}

pub fn apply_music_texts(ui: &AppWindow) {
    ui.global::<Music>().set_t(texts());
    ui.set_music_view_label(tr("music.switch").into());
}

pub fn wire(ui: &AppWindow) {
    let music = ui.global::<Music>();
    music.on_open_home(|| with_app(|app| app.music_go(Target::Home)));
    music.on_open_library(|| with_app(|app| app.music_go(Target::Library)));
    music.on_search(|q| with_app(|app| app.music_search(q.as_str())));
    music.on_open_card(|id| with_app(|app| app.music_open_card(id as usize, false)));
    music.on_play_card(|id| with_app(|app| app.music_open_card(id as usize, true)));
    music.on_open_sidebar(|id| with_app(|app| app.music_open_sidebar(id as usize)));
    music.on_play_page(|| with_app(|app| app.music_play_page()));
    music.on_media(|command| with_app(|app| app.media_command(TAB_MUSIC, command.as_str(), 0.0)));
    music.on_seek(|fraction| with_app(|app| app.music_seek(fraction as f64)));
    music.on_set_volume(|v| with_app(|app| app.media_command(TAB_MUSIC, "set_volume", v as f64)));
    music.on_toggle_queue(|| with_app(|app| app.music_toggle_queue()));
    music.on_play_queue(|i| with_app(|app| app.music_play_queue(i as u32)));
    music.on_login(|| with_app(|app| app.music_start_login()));
    ui.on_toggle_music_view(|| with_app(|app| app.music_end_login()));
}

fn card(id: usize, entry: &Entry, image: Option<Image>) -> MusicCard {
    MusicCard {
        id: id as i32,
        title: entry.title.clone().into(),
        subtitle: entry.subtitle.clone().into(),
        duration: entry.duration.clone().unwrap_or_default().into(),
        has_image: image.is_some(),
        image: image.unwrap_or_default(),
        round: entry.round,
        playing: false,
    }
}

fn entry_video(entry: &Entry) -> Option<&str> {
    match entry.play.as_ref().or(entry.open.as_ref()) {
        Some(Nav::Play { video_id: Some(v), .. }) => Some(v.as_str()),
        _ => None,
    }
}

impl App {
    fn music(&self) -> std::cell::RefMut<'_, MusicState> {
        self.music.borrow_mut()
    }

    /// Ist die Playtube-Ansicht gerade zu sehen? (Im Tab YouTube Music immer - außer während der
    /// Google-Anmeldung.)
    pub fn music_active(&self) -> bool {
        self.current.get() == TAB_MUSIC && !self.music_login.get() && self.config.borrow().music_view
    }

    /// Google-Anmeldung im YouTube-Music-Tab zeigen; nach der Anmeldung geht es von selbst zurück.
    pub fn music_start_login(&self) {
        self.music_login.set(true);
        self.ui.set_music_login(true);
        self.tabs[TAB_MUSIC].load(LOGIN_URL);
        self.select_tab(TAB_MUSIC);
    }

    /// Zurück zur Playtube-Ansicht (nach der Anmeldung oder über "Fertig").
    pub fn music_end_login(&self) {
        if !self.music_login.replace(false) {
            return;
        }
        self.ui.set_music_login(false);
        {
            let mut music = self.music();
            music.auth = None;
            music.sidebar_loaded = false;
        }
        self.select_tab(TAB_MUSIC);
        self.music_reload();
    }

    /// YouTube Music ist fertig geladen: Wurde die Anmeldung vorher ohne die Angaben der Seite
    /// gebaut (direkt nach dem Start), jetzt vollständig neu holen und die Seiten neu laden.
    pub fn music_page_loaded(&self) {
        let incomplete = self.music.borrow().auth.as_ref().is_some_and(|a| !a.complete);
        if incomplete {
            self.music().sidebar_loaded = false;
            self.music_reload();
        }
    }

    /// Adresswechsel im YouTube-Music-Tab: Ist die Anmeldung fertig (wieder auf music.youtube.com),
    /// automatisch zurück zur Playtube-Ansicht.
    pub fn music_url_changed(&self, url: &str) {
        if self.music_login.get() && url.starts_with(ytm::ORIGIN) {
            self.music_end_login();
        }
    }

    /// Nach jedem Tabwechsel: WebView verstecken und Seite laden, falls nötig.
    pub fn music_on_select(&self) {
        let active = self.music_active();
        self.ui.set_music_view(active && self.current.get() == TAB_MUSIC);
        if !active {
            // zurück an die normale Stelle (Original-Ansicht)
            self.layout();
            return;
        }
        self.layout();
        self.ui.set_url_text(SharedString::new());
        let fresh = self.music.borrow().history.is_empty();
        if fresh {
            self.music_go(Target::Home);
        }
        if !self.music.borrow().sidebar_loaded {
            self.music_load_sidebar();
        }
        self.music_timer.start(slint::TimerMode::Repeated, Duration::from_millis(500), || with_app(|app| app.music_tick()));
    }

    // ------------------------------------------------------------------ Anmeldung

    /// Login aus der WebView übernehmen (Cookies + ytcfg) und danach `then` laden.
    fn music_refresh_auth(&self, then: Target) {
        if self.music.borrow().auth_pending.is_some() {
            self.music().auth_pending = Some(then);
            return;
        }
        self.music().auth_pending = Some(then);
        // Cookies ohne Warten lesen (sonst kann Playtube hängen), danach ytcfg aus der Seite.
        self.tabs[TAB_MUSIC].cookies_async(ytm::ORIGIN, |cookies| with_app(|app| app.music_auth_cookies(cookies)));
    }

    fn music_auth_cookies(&self, cookies: Vec<(String, String)>) {
        self.tabs[TAB_MUSIC].eval_with(YTCFG_JS, move |cfg| {
            let cfg = cfg.unwrap_or(Value::Null);
            let cookies = cookies.clone();
            on_ui(move |app| {
                let auth = ytm::Auth::from_parts(&cookies, &cfg);
                app.ui.global::<Music>().set_logged_in(auth.logged_in());
                let then = {
                    let mut music = app.music();
                    music.auth = Some(auth);
                    music.auth_pending.take()
                };
                if let Some(target) = then {
                    app.music_load(target);
                }
                if !app.music.borrow().sidebar_loaded {
                    app.music_load_sidebar();
                }
            });
        });
    }

    // ------------------------------------------------------------------ Navigation

    pub fn music_go(&self, target: Target) {
        {
            let mut music = self.music();
            let keep = (music.position + 1).min(music.history.len());
            music.history.truncate(keep);
            music.history.push(target.clone());
            music.position = music.history.len() - 1;
        }
        self.music_load(target);
    }

    pub fn music_search(&self, query: &str) {
        let query = query.trim();
        if !query.is_empty() {
            self.music_go(Target::Search(query.to_string()));
        }
    }

    pub fn music_back(&self) {
        let target = {
            let mut music = self.music();
            if music.position == 0 {
                return;
            }
            music.position -= 1;
            music.history[music.position].clone()
        };
        self.music_load(target);
    }

    pub fn music_forward(&self) {
        let target = {
            let mut music = self.music();
            if music.position + 1 >= music.history.len() {
                return;
            }
            music.position += 1;
            music.history[music.position].clone()
        };
        self.music_load(target);
    }

    pub fn music_reload(&self) {
        let target = self.music.borrow().history.get(self.music.borrow().position).cloned();
        if let Some(target) = target {
            self.music().auth = None; // frischer Login (z. B. nach dem Anmelden)
            self.music_load(target);
        }
    }

    fn music_load(&self, target: Target) {
        let Some(auth) = self.music.borrow().auth.clone() else {
            self.music_refresh_auth(target);
            return;
        };
        let generation = {
            let mut music = self.music();
            music.generation += 1;
            music.generation
        };
        let ui = self.ui.global::<Music>();
        ui.set_loading(true);
        ui.set_error(SharedString::new());
        ui.set_section(match target {
            Target::Home => 0,
            Target::Search(_) => 1,
            Target::Library => 2,
            Target::Browse { .. } => 3,
        });
        self.runtime.spawn(async move {
            let result = match &target {
                Target::Home => ytm::browse(&auth, ytm::HOME, None).await,
                Target::Library => ytm::browse(&auth, ytm::LIBRARY_PLAYLISTS, None).await,
                Target::Search(q) => ytm::search(&auth, q).await,
                Target::Browse { id, params, .. } => ytm::browse(&auth, id, params.as_deref()).await,
            };
            on_ui(move |app| {
                if app.music.borrow().generation != generation {
                    return;
                }
                let ui = app.ui.global::<Music>();
                ui.set_loading(false);
                match result {
                    Ok(page) => app.music_show(page, &target),
                    Err(e) => {
                        log::line(&format!("music: Laden fehlgeschlagen: {e:#}"));
                        ui.set_error(tr("music.error").into());
                    }
                }
            });
        });
    }

    fn music_show(&self, mut page: Page, target: &Target) {
        // Startseite: lange Songlisten (z. B. "Schnellauswahl") kürzen wie bei Spotify.
        if let Target::Home = target {
            for shelf in page.shelves.iter_mut().filter(|s| s.rows) {
                shelf.entries.truncate(8);
            }
        }
        if let Target::Library = target {
            page.title = tr("music.library").into();
            // Lieblingssongs als erste Karte, wie bei Spotify
            let liked = Entry {
                title: tr("music.liked").into(),
                subtitle: tr("music.liked_sub").into(),
                open: Some(Nav::Browse { browse_id: ytm::LIKED_SONGS.into(), params: None, page: PageType::Playlist }),
                ..Default::default()
            };
            match page.shelves.first_mut() {
                Some(shelf) if !shelf.rows => shelf.entries.insert(0, liked),
                _ => page.shelves.insert(0, ytm::Shelf { title: String::new(), entries: vec![liked], rows: false }),
            }
        }
        let ui = self.ui.global::<Music>();
        if page.logged_in == Some(false) {
            ui.set_logged_in(false);
        }
        let has_header = matches!(target, Target::Browse { .. } | Target::Library) && !page.title.is_empty();
        ui.set_has_header(has_header);
        ui.set_page_title(page.title.clone().into());
        ui.set_page_subtitle(page.subtitle.clone().into());
        ui.set_page_description(page.description.chars().take(300).collect::<String>().into());
        ui.set_page_can_play(page.play.is_some());
        ui.set_page_round(matches!(target, Target::Browse { page: PageType::Artist, .. }));
        ui.set_scroll_y(0.0);

        let mut entries = Vec::new();
        let mut models = Vec::new();
        let mut urls = Vec::new();
        let mut shelves = Vec::new();
        let mut wanted = Vec::new();
        for shelf in &page.shelves {
            let size = if shelf.rows { ROW_PX } else { CARD_PX };
            let mut cards = Vec::new();
            let mut card_urls = Vec::new();
            for entry in &shelf.entries {
                let url = entry.thumb.as_deref().map(|u| ytm::sized_thumb(u, size));
                let image = url.as_ref().and_then(|u| self.music_cached(u));
                if let Some(u) = &url {
                    wanted.push(u.clone());
                }
                cards.push(card(entries.len(), entry, image));
                card_urls.push(url);
                entries.push(entry.clone());
            }
            let model = Rc::new(VecModel::from(cards));
            shelves.push(MusicShelf { title: shelf.title.clone().into(), rows: shelf.rows, cards: ModelRc::from(model.clone()) });
            models.push(model);
            urls.push(card_urls);
        }
        let page_url = page.thumb.as_deref().map(|u| ytm::sized_thumb(u, HEADER_PX));
        match page_url.as_ref().and_then(|u| self.music_cached(u)) {
            Some(image) => {
                ui.set_page_image(image);
                ui.set_page_has_image(true);
            }
            None => ui.set_page_has_image(false),
        }
        if let Some(u) = &page_url {
            wanted.push(u.clone());
        }
        ui.set_shelves(ModelRc::new(VecModel::from(shelves)));
        {
            let mut music = self.music();
            music.entries = entries;
            music.shelf_models = models;
            music.shelf_urls = urls;
            music.page_url = page_url;
            music.page = Some(page);
        }
        self.music_mark_playing();
        for url in wanted {
            self.music_fetch_image(url);
        }
    }

    fn music_load_sidebar(&self) {
        let Some(auth) = self.music.borrow().auth.clone() else { return };
        self.music().sidebar_loaded = true;
        self.runtime.spawn(async move {
            let result = ytm::browse(&auth, ytm::LIBRARY_PLAYLISTS, None).await;
            on_ui(move |app| {
                let mut list = vec![Entry {
                    title: tr("music.liked").into(),
                    subtitle: tr("music.liked_sub").into(),
                    open: Some(Nav::Browse { browse_id: ytm::LIKED_SONGS.into(), params: None, page: PageType::Playlist }),
                    ..Default::default()
                }];
                if let Ok(page) = result {
                    list.extend(
                        page.shelves
                            .into_iter()
                            .flat_map(|s| s.entries)
                            .filter(|e| matches!(e.open, Some(Nav::Browse { page: PageType::Playlist | PageType::Album | PageType::Artist, .. }))),
                    );
                }
                let mut urls = Vec::new();
                let cards: Vec<MusicCard> = list
                    .iter()
                    .enumerate()
                    .map(|(i, e)| {
                        let url = e.thumb.as_deref().map(|u| ytm::sized_thumb(u, ROW_PX));
                        let image = url.as_ref().and_then(|u| app.music_cached(u));
                        urls.push(url);
                        card(i, e, image)
                    })
                    .collect();
                let model = Rc::new(VecModel::from(cards));
                app.ui.global::<Music>().set_sidebar(ModelRc::from(model.clone()));
                {
                    let mut music = app.music();
                    music.sidebar = list;
                    music.sidebar_model = Some(model);
                    music.sidebar_urls = urls.clone();
                }
                for url in urls.into_iter().flatten() {
                    app.music_fetch_image(url);
                }
            });
        });
    }

    // ------------------------------------------------------------------ Klicks

    fn music_open_card(&self, id: usize, play: bool) {
        let Some(entry) = self.music.borrow().entries.get(id).cloned() else { return };
        self.music_activate(&entry, play);
    }

    fn music_open_sidebar(&self, id: usize) {
        let Some(entry) = self.music.borrow().sidebar.get(id).cloned() else { return };
        self.music_activate(&entry, false);
    }

    fn music_activate(&self, entry: &Entry, play: bool) {
        let nav = if play { entry.play.as_ref().or(entry.open.as_ref()) } else { entry.open.as_ref() };
        match nav {
            Some(Nav::Browse { browse_id, params, page }) => {
                self.music_go(Target::Browse { id: browse_id.clone(), params: params.clone(), page: *page })
            }
            // Einfacher Klick auf einen Song: wie bei Spotify erst mit Doppelklick/Knopf abspielen.
            Some(nav @ Nav::Play { .. }) if play || entry.duration.is_none() => self.music_play(nav),
            _ => {}
        }
    }

    fn music_play_page(&self) {
        let nav = self.music.borrow().page.as_ref().and_then(|p| p.play.clone());
        if let Some(nav) = nav {
            self.music_play(&nav);
        }
    }

    fn music_play(&self, nav: &Nav) {
        if let Some(url) = ytm::watch_url(nav) {
            self.tabs[TAB_MUSIC].load(&url);
            if let Nav::Play { video_id: Some(v), .. } = nav {
                self.music().now_video = Some(v.clone());
                self.music_mark_playing();
            }
            // Warteschlange gleich neu lesen
            self.music().last_queue = None;
        }
    }

    fn music_seek(&self, fraction: f64) {
        let duration = self.music.borrow().duration_s;
        if duration > 0.0 {
            if let Some(js) = scripts::jam_seek(fraction.clamp(0.0, 1.0) * duration) {
                self.tabs[TAB_MUSIC].eval(&js);
            }
        }
    }

    fn music_toggle_queue(&self) {
        let ui = self.ui.global::<Music>();
        ui.set_queue_open(!ui.get_queue_open());
        self.music().last_queue = None;
    }

    fn music_play_queue(&self, index: u32) {
        self.tabs[TAB_MUSIC].eval(&scripts::play_queue_item(index));
        self.music().last_queue = None;
    }

    // ------------------------------------------------------------------ Player-Leiste

    /// Neuer Stand des YouTube-Music-Players (aus poll_media).
    pub fn music_on_media(&self, info: &Value) {
        if std::env::var_os("PLAYTUBE_JAM_DEBUG").is_some() {
            log::line(&format!("music-debug: {info}"));
        }
        let ui = self.ui.global::<Music>();
        let title = info["title"].as_str().unwrap_or("").to_string();
        ui.set_has_track(!title.is_empty() && info["hasVideo"].as_bool().unwrap_or(false));
        ui.set_now_title(title.into());
        ui.set_now_artist(info["subtitle"].as_str().unwrap_or("").into());
        ui.set_volume(info["volume"].as_f64().unwrap_or(100.0) as f32);
        ui.set_like_status(info["likeStatus"].as_str().unwrap_or("").into());
        ui.set_repeat_mode(info["repeatMode"].as_str().unwrap_or("").into());
        let video = info["url"].as_str().and_then(crate::jam::video_id_from_url);
        let thumb = info["thumbnail"].as_str().map(str::to_string);
        let changed_video = {
            let mut music = self.music();
            music.position_s = info["currentTime"].as_f64().unwrap_or(0.0);
            music.duration_s = info["duration"].as_f64().unwrap_or(0.0);
            music.playing = info["playing"].as_bool().unwrap_or(false);
            music.polled_at = Some(Instant::now());
            let changed = video.is_some() && music.now_video != video;
            if video.is_some() {
                music.now_video = video;
            }
            changed
        };
        ui.set_playing(self.music.borrow().playing);
        if let Some(url) = thumb {
            let url = ytm::sized_thumb(&url, ROW_PX);
            if self.music.borrow().now_url.as_deref() != Some(url.as_str()) {
                self.music().now_url = Some(url.clone());
                match self.music_cached(&url) {
                    Some(image) => {
                        ui.set_now_image(image);
                        ui.set_now_has_image(true);
                    }
                    None => {
                        ui.set_now_has_image(false);
                        self.music_fetch_image(url);
                    }
                }
            }
        }
        if changed_video {
            self.music_mark_playing();
            self.music().last_queue = None;
        }
        self.music_render_progress();
    }

    fn music_render_progress(&self) {
        let music = self.music.borrow();
        let mut pos = music.position_s;
        if music.playing {
            if let Some(at) = music.polled_at {
                pos += at.elapsed().as_secs_f64();
            }
        }
        let dur = music.duration_s;
        if dur > 0.0 {
            pos = pos.min(dur);
        }
        let ui = self.ui.global::<Music>();
        ui.set_position_text(crate::jam::format_time(pos).into());
        ui.set_duration_text(crate::jam::format_time(dur).into());
        ui.set_progress(if dur > 0.0 { (pos / dur) as f32 } else { 0.0 });
    }

    /// Alle 500 ms, solange die Ansicht sichtbar ist.
    fn music_tick(&self) {
        if !self.music_active() {
            self.music_timer.stop();
            return;
        }
        self.music_render_progress();
        let queue_due = self.ui.global::<Music>().get_queue_open()
            && self.music.borrow().last_queue.is_none_or(|t| t.elapsed() > QUEUE_EVERY);
        if queue_due {
            self.music().last_queue = Some(Instant::now());
            self.tabs[TAB_MUSIC].eval_with(scripts::QUEUE, |result| {
                if let Some(Value::Array(items)) = result {
                    on_ui(move |app| app.music_show_queue(items));
                }
            });
        }
    }

    fn music_show_queue(&self, items: Vec<Value>) {
        let mut urls = Vec::new();
        let cards: Vec<MusicCard> = items
            .iter()
            .take(200)
            .enumerate()
            .map(|(i, item)| {
                let url = item["thumbnail"].as_str().filter(|u| u.starts_with("https://")).map(|u| ytm::sized_thumb(u, ROW_PX));
                let image = url.as_ref().and_then(|u| self.music_cached(u));
                urls.push(url);
                MusicCard {
                    id: i as i32,
                    title: item["title"].as_str().unwrap_or("").into(),
                    subtitle: item["subtitle"].as_str().unwrap_or("").into(),
                    duration: SharedString::new(),
                    has_image: image.is_some(),
                    image: image.unwrap_or_default(),
                    round: false,
                    playing: item["current"].as_bool().unwrap_or(false),
                }
            })
            .collect();
        let model = Rc::new(VecModel::from(cards));
        self.ui.global::<Music>().set_queue(ModelRc::from(model.clone()));
        {
            let mut music = self.music();
            music.queue_model = Some(model);
            music.queue_urls = urls.clone();
        }
        for url in urls.into_iter().flatten() {
            self.music_fetch_image(url);
        }
    }

    /// Den gerade laufenden Song in den Listen hervorheben.
    fn music_mark_playing(&self) {
        let music = self.music.borrow();
        let now = music.now_video.as_deref();
        let mut id = 0;
        for model in &music.shelf_models {
            for row in 0..model.row_count() {
                let playing = now.is_some() && music.entries.get(id).and_then(entry_video) == now;
                if let Some(mut card) = model.row_data(row) {
                    if card.playing != playing {
                        card.playing = playing;
                        model.set_row_data(row, card);
                    }
                }
                id += 1;
            }
        }
    }

    // ------------------------------------------------------------------ Bilder

    fn music_cached(&self, url: &str) -> Option<Image> {
        self.music.borrow().images.get(url).map(|buffer| Image::from_rgba8(buffer.clone()))
    }

    fn music_fetch_image(&self, url: String) {
        {
            let mut music = self.music();
            if music.images.contains_key(&url) || !music.loading.insert(url.clone()) {
                return;
            }
        }
        self.runtime.spawn(async move {
            let buffer = download_image(&url).await;
            on_ui(move |app| app.music_image_ready(url, buffer));
        });
    }

    fn music_image_ready(&self, url: String, buffer: Option<SharedPixelBuffer<Rgba8Pixel>>) {
        let mut music = self.music();
        music.loading.remove(&url);
        let Some(buffer) = buffer else { return };
        if music.images.len() > 600 {
            music.images.clear();
        }
        music.images.insert(url.clone(), buffer.clone());
        let image = Image::from_rgba8(buffer);
        let set = |model: &Rc<VecModel<MusicCard>>, urls: &[Option<String>]| {
            for (row, u) in urls.iter().enumerate() {
                if u.as_deref() == Some(url.as_str()) {
                    if let Some(mut card) = model.row_data(row) {
                        card.image = image.clone();
                        card.has_image = true;
                        model.set_row_data(row, card);
                    }
                }
            }
        };
        for (model, urls) in music.shelf_models.iter().zip(&music.shelf_urls) {
            set(model, urls);
        }
        if let Some(model) = &music.sidebar_model {
            set(model, &music.sidebar_urls);
        }
        if let Some(model) = &music.queue_model {
            set(model, &music.queue_urls);
        }
        let ui = self.ui.global::<Music>();
        if music.page_url.as_deref() == Some(url.as_str()) {
            ui.set_page_image(image.clone());
            ui.set_page_has_image(true);
        }
        if music.now_url.as_deref() == Some(url.as_str()) {
            ui.set_now_image(image);
            ui.set_now_has_image(true);
        }
    }
}

/// Bild laden und in Pixel umwandeln (im tokio-Runtime).
async fn download_image(url: &str) -> Option<SharedPixelBuffer<Rgba8Pixel>> {
    if !url.starts_with("https://") {
        return None;
    }
    let client = reqwest::Client::builder().timeout(Duration::from_secs(15)).build().ok()?;
    let response = client.get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    if bytes.len() > 4_000_000 {
        return None;
    }
    tokio::task::spawn_blocking(move || {
        let image = image::load_from_memory(&bytes).ok()?.to_rgba8();
        let (w, h) = image.dimensions();
        Some(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(image.as_raw(), w, h))
    })
    .await
    .ok()
    .flatten()
}
