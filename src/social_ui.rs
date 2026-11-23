//! Tab „Sozial“: Playtube-Konto (anmelden, registrieren, Passwort vergessen, abmelden, löschen)
//! und Freunde (suchen, Anfragen, was sie hören, in den eigenen Sync einladen). Netzwerk und
//! Protokoll stehen in `social.rs`. Ohne Konto bleibt alles wie bisher.

use super::*;
use crate::social::{self, Event};
use serde_json::json;
use slint::{Rgba8Pixel, SharedPixelBuffer};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

/// Der Tab hat den Index 4 (nach YouTube, YouTube Music, Einstellungen, Sync).
/// So lange bleibt eine Einladung sichtbar (wie auf dem Server).
const INVITE_TTL: Duration = Duration::from_secs(15 * 60);
const PRIVACY_URL: &str = "https://playtube-ppp-eula.pages.dev/playtube/privacy/#konto";

#[derive(Default)]
pub struct SocialState {
    connection: Option<social::Connection>,
    user: Option<social::User>,
    connected: bool,
    friends: social::Friends,
    results: Vec<social::Person>,
    invites: Vec<(social::Invite, Instant)>,
    /// In den Sync mit diesem Code schon eingeladene Freunde
    invited: (String, HashSet<String>),
    /// Warten auf den eigenen Sync, der gerade startet
    pending_invites: Vec<String>,
    last_presence: Option<Value>,
    images: HashMap<String, SharedPixelBuffer<Rgba8Pixel>>,
    loading: HashSet<String>,
}

fn error_text(error: &str) -> String {
    let text = tr(&format!("social.error.{error}"));
    if text == "?" { tr("social.error.failed").to_string() } else { text.to_string() }
}

pub fn apply_social_texts(ui: &AppWindow) {
    ui.set_st(SocialTexts {
        tab: tr("social.tab").into(),
        title: tr("social.title").into(),
        intro: tr("social.intro").into(),
        login_title: tr("social.login_title").into(),
        register_title: tr("social.register_title").into(),
        reset_title: tr("social.reset_title").into(),
        login_field: tr("social.login_field").into(),
        name_field: tr("social.name_field").into(),
        email_field: tr("social.email_field").into(),
        password_field: tr("social.password_field").into(),
        password2_field: tr("social.password2_field").into(),
        login: tr("social.login").into(),
        register: tr("social.register").into(),
        send_reset: tr("social.send_reset").into(),
        to_register: tr("social.to_register").into(),
        to_login: tr("social.to_login").into(),
        forgot: tr("social.forgot").into(),
        optional_hint: tr("social.optional_hint").into(),
        signed_in_as: tr("social.signed_in_as").into(),
        logout: tr("social.logout").into(),
        share: tr("social.share").into(),
        share_hint: tr("social.share_hint").into(),
        delete_account: tr("social.delete_account").into(),
        delete_confirm: tr("social.delete_confirm").into(),
        delete_hint: tr("social.delete_hint").into(),
        cancel: tr("social.cancel").into(),
        friends: tr("social.friends").into(),
        no_friends: tr("social.no_friends").into(),
        search_placeholder: tr("social.search_placeholder").into(),
        search: tr("social.search").into(),
        add: tr("social.add").into(),
        accept: tr("social.accept").into(),
        decline: tr("social.decline").into(),
        withdraw: tr("social.withdraw").into(),
        remove_friend: tr("social.remove_friend").into(),
        incoming: tr("social.incoming").into(),
        outgoing: tr("social.outgoing").into(),
        invite: tr("social.invite").into(),
        invite_hint: tr("social.invite_hint").into(),
        invites: tr("social.invites").into(),
        join: tr("social.join").into(),
        dismiss: tr("social.dismiss").into(),
        offline: tr("social.offline").into(),
        privacy: tr("social.privacy").into(),
    });
}

pub fn wire(ui: &AppWindow) {
    ui.on_social_login(|| with_app(|app| app.social_login()));
    ui.on_social_register(|| with_app(|app| app.social_register()));
    ui.on_social_send_reset(|| with_app(|app| app.social_send_reset()));
    ui.on_social_logout(|| with_app(|app| app.social_logout()));
    ui.on_social_delete_account(|| with_app(|app| app.social_delete_account()));
    ui.on_social_set_share(|on| with_app(|app| app.social_set_share(on)));
    ui.on_social_search(|| with_app(|app| app.social_search()));
    ui.on_social_request(|id| with_app(|app| app.social_friend_action("request", id.to_string())));
    ui.on_social_accept(|id| with_app(|app| app.social_friend_action("accept", id.to_string())));
    ui.on_social_remove(|id| with_app(|app| app.social_friend_action("remove", id.to_string())));
    ui.on_social_invite(|id| with_app(|app| app.social_invite(id.to_string())));
    ui.on_social_join(|id| with_app(|app| app.social_join(id.as_str())));
    ui.on_social_dismiss(|id| with_app(|app| app.social_dismiss(id.as_str())));
    ui.on_social_open_privacy(|| {
        let _ = open::that(PRIVACY_URL);
    });
}

fn initial(name: &str) -> String {
    name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default()
}

impl App {
    fn social(&self) -> std::cell::RefMut<'_, SocialState> {
        self.social.borrow_mut()
    }

    fn social_server(&self) -> String {
        self.config.borrow().jam.server.clone()
    }

    fn social_token(&self) -> Option<String> {
        let token = self.config.borrow().account.token.clone();
        (!token.is_empty()).then_some(token)
    }

    fn social_set_status(&self, text: impl Into<SharedString>) {
        self.ui.set_social_status(text.into());
    }

    /// Beim Start: gespeicherte Anmeldung wieder aufnehmen.
    pub fn social_init(&self) {
        let (token, name) = {
            let config = self.config.borrow();
            (config.account.token.clone(), config.account.name.clone())
        };
        if token.is_empty() {
            self.ui.set_social_signed_in(false);
            return;
        }
        self.ui.set_social_signed_in(true);
        self.ui.set_social_user_name(name.into());
        self.social_connect(&token);
    }

    fn social_connect(&self, token: &str) {
        let server = self.social_server();
        let connection = social::Connection::start(self.runtime.handle(), &server, token, |event| {
            on_ui(move |app| app.social_event(event));
        });
        let mut state = self.social();
        state.connection = Some(connection);
        state.last_presence = None;
    }

    fn social_send(&self, msg: Value) {
        if let Some(connection) = self.social.borrow().connection.as_ref() {
            connection.send(msg);
        }
    }

    // ------------------------------------------------------------------ Konto

    /// Angemeldet (bzw. registriert): Token speichern und verbinden.
    fn social_signed_in(&self, token: String, user: social::User) {
        {
            let mut config = self.config.borrow_mut();
            config.account.token = token.clone();
            config.account.name = user.name.clone();
            config::save(&config);
        }
        self.ui.set_social_password_input(SharedString::new());
        self.ui.set_social_password2_input(SharedString::new());
        self.ui.set_social_signed_in(true);
        self.social_set_status("");
        self.social_apply_user(&user);
        self.social().user = Some(user);
        self.social_connect(&token);
    }

    fn social_apply_user(&self, user: &social::User) {
        self.ui.set_social_user_name(user.name.clone().into());
        self.ui.set_social_user_email(user.email.clone().into());
        self.ui.set_social_share(user.share);
    }

    /// Lokal abmelden (Token vergessen, Verbindung zu).
    fn social_forget(&self, status: String) {
        {
            let mut config = self.config.borrow_mut();
            config.account.token.clear();
            config::save(&config);
        }
        *self.social() = SocialState::default();
        self.ui.set_social_signed_in(false);
        self.ui.set_social_connected(false);
        self.ui.set_social_delete_input(SharedString::new());
        self.ui.set_social_mode(0);
        self.social_set_status(status);
        self.social_render();
    }

    fn social_busy(&self, busy: bool) {
        self.ui.set_social_busy(busy);
        if busy {
            self.social_set_status(tr("social.status.working"));
        }
    }

    /// REST-Aufruf im Hintergrund, Ergebnis im UI-Thread.
    fn social_call(
        &self,
        method: &'static str,
        path: &'static str,
        body: Option<Value>,
        with_token: bool,
        done: impl FnOnce(&Rc<App>, Result<Value, social::ApiError>) + Send + 'static,
    ) {
        let server = self.social_server();
        let token = if with_token { self.social_token() } else { None };
        self.runtime.spawn(async move {
            let result = social::call(&server, method, path, token.as_deref(), body).await;
            on_ui(move |app| done(app, result));
        });
    }

    pub fn social_login(&self) {
        let login = self.ui.get_social_login_input().trim().to_string();
        let password = self.ui.get_social_password_input().to_string();
        if login.is_empty() || password.is_empty() {
            self.social_set_status(tr("social.status.fill_in"));
            return;
        }
        self.social_busy(true);
        self.social_call("POST", "/api/account/login", Some(json!({ "login": login, "password": password, "client": "pc" })), false, |app, result| {
            app.ui.set_social_busy(false);
            match result.map(|v| social::session_of(&v)) {
                Ok(Some((token, user))) => app.social_signed_in(token, user),
                Ok(None) => app.social_set_status(error_text("failed")),
                Err(e) => app.social_set_status(error_text(&e.0)),
            }
        });
    }

    pub fn social_register(&self) {
        let name = self.ui.get_social_name_input().trim().to_string();
        let email = self.ui.get_social_email_input().trim().to_string();
        let password = self.ui.get_social_password_input().to_string();
        if password != self.ui.get_social_password2_input().as_str() {
            self.social_set_status(tr("social.status.mismatch"));
            return;
        }
        self.social_busy(true);
        let body = json!({ "name": name, "email": email, "password": password, "client": "pc" });
        self.social_call("POST", "/api/account/register", Some(body), false, |app, result| {
            app.ui.set_social_busy(false);
            match result.map(|v| social::session_of(&v)) {
                Ok(Some((token, user))) => app.social_signed_in(token, user),
                Ok(None) => app.social_set_status(error_text("failed")),
                Err(e) => app.social_set_status(error_text(&e.0)),
            }
        });
    }

    pub fn social_send_reset(&self) {
        let email = self.ui.get_social_email_input().trim().to_string();
        if email.is_empty() {
            self.social_set_status(tr("social.status.fill_in"));
            return;
        }
        self.social_busy(true);
        self.social_call("POST", "/api/account/reset-request", Some(json!({ "email": email })), false, |app, result| {
            app.ui.set_social_busy(false);
            match result {
                Ok(_) => {
                    app.ui.set_social_mode(0);
                    app.social_set_status(tr("social.status.reset_sent"));
                }
                Err(e) => app.social_set_status(error_text(&e.0)),
            }
        });
    }

    pub fn social_logout(&self) {
        // Erst lokal abmelden (geht auch offline), dann das Token beim Server ungültig machen.
        let token = self.social_token();
        let server = self.social_server();
        self.social_forget(tr("social.status.logged_out").to_string());
        if let Some(token) = token {
            self.runtime.spawn(async move {
                let _ = social::call(&server, "POST", "/api/account/logout", Some(&token), None).await;
            });
        }
    }

    pub fn social_delete_account(&self) {
        let password = self.ui.get_social_delete_input().to_string();
        if password.is_empty() {
            self.social_set_status(tr("social.status.fill_in"));
            return;
        }
        self.social_busy(true);
        self.social_call("DELETE", "/api/account", Some(json!({ "password": password })), true, |app, result| {
            app.ui.set_social_busy(false);
            match result {
                Ok(_) => app.social_forget(tr("social.status.deleted").to_string()),
                Err(e) => app.social_set_status(error_text(&e.0)),
            }
        });
    }

    pub fn social_set_share(&self, share: bool) {
        self.social_call("PATCH", "/api/account", Some(json!({ "share": share })), true, move |app, result| match result {
            Ok(value) => {
                if let Ok(user) = serde_json::from_value::<social::User>(value["user"].clone()) {
                    app.social_apply_user(&user);
                    app.social().user = Some(user);
                }
            }
            Err(e) => {
                app.ui.set_social_share(!share);
                app.social_set_status(error_text(&e.0));
            }
        });
    }

    // ------------------------------------------------------------------ Freunde

    pub fn social_search(&self) {
        let query = self.ui.get_social_search_input().trim().to_string();
        if query.chars().count() < 2 {
            self.social_set_status(tr("social.status.search_short"));
            return;
        }
        self.social_set_status("");
        let server = self.social_server();
        let token = self.social_token();
        self.runtime.spawn(async move {
            let path = format!("/api/friends/search?q={}", urlencode(&query));
            let result = social::call(&server, "GET", &path, token.as_deref(), None).await;
            on_ui(move |app| match result {
                Ok(value) => {
                    let results: Vec<social::Person> = serde_json::from_value(value["results"].clone()).unwrap_or_default();
                    if results.is_empty() {
                        app.social_set_status(tr("social.status.no_results"));
                    }
                    app.social().results = results;
                    app.social_render();
                }
                Err(e) => app.social_set_status(error_text(&e.0)),
            });
        });
    }

    /// Anfrage senden, annehmen oder entfernen (ablehnen, zurückziehen, Freundschaft beenden).
    pub fn social_friend_action(&self, action: &'static str, id: String) {
        let path = match action {
            "request" => "/api/friends/request",
            "accept" => "/api/friends/accept",
            _ => "/api/friends/remove",
        };
        self.social_call("POST", path, Some(json!({ "id": id })), true, move |app, result| match result {
            Ok(value) => {
                let status = value["status"].as_str().unwrap_or("");
                {
                    let mut state = app.social();
                    for person in state.results.iter_mut().filter(|p| p.id == id) {
                        person.relation = match status {
                            "requested" => "outgoing".into(),
                            "friends" => "friend".into(),
                            _ => "none".into(),
                        };
                    }
                }
                app.social_set_status(match status {
                    "requested" => tr("social.status.requested"),
                    "friends" => tr("social.status.now_friends"),
                    _ => "",
                });
                app.social_render();
            }
            Err(e) => app.social_set_status(error_text(&e.0)),
        });
    }

    /// Freund in den eigenen Sync einladen (läuft keiner, wird einer gestartet).
    pub fn social_invite(&self, friend_id: String) {
        match self.jam_code() {
            Some(code) => self.social_send_invite(&friend_id, &code),
            None => {
                let starting = {
                    let mut state = self.social();
                    let starting = !state.pending_invites.is_empty();
                    if !state.pending_invites.contains(&friend_id) {
                        state.pending_invites.push(friend_id);
                    }
                    starting
                };
                self.ui.set_social_friends_status(tr("social.status.starting_sync").into());
                if !starting {
                    self.jam_start();
                }
            }
        }
        self.social_render();
    }

    fn social_send_invite(&self, friend_id: &str, code: &str) {
        self.social_send(json!({ "t": "invite", "to": friend_id, "code": code }));
        let mut state = self.social();
        if state.invited.0 != code {
            state.invited = (code.to_string(), HashSet::new());
        }
        state.invited.1.insert(friend_id.to_string());
    }

    /// Der eigene Sync läuft jetzt (aus `jam_connect`): wartende Einladungen schicken.
    pub fn social_jam_started(&self, code: &str) {
        let pending = std::mem::take(&mut self.social().pending_invites);
        if pending.is_empty() {
            return;
        }
        for friend in &pending {
            self.social_send_invite(friend, code);
        }
        self.social_render();
    }

    /// Einladung annehmen: dem Sync des Freundes beitreten (einen anderen vorher verlassen).
    pub fn social_join(&self, invite_id: &str) {
        let invite = {
            let mut state = self.social();
            let index = state.invites.iter().position(|(i, _)| i.id == invite_id);
            index.map(|i| state.invites.remove(i).0)
        };
        self.social_render();
        let Some(invite) = invite else { return };
        self.show_window();
        self.select_tab(jam_ui::TAB_JAM);
        self.ui.set_jam_code_input(invite.code.clone().into());
        match self.jam_code() {
            Some(code) if code == invite.code => {}
            Some(_) => {
                // Erst den alten Sync sauber verlassen, dann beitreten.
                self.jam_leave();
                let code = invite.code.clone();
                let timer = slint::Timer::default();
                timer.start(slint::TimerMode::SingleShot, Duration::from_millis(1000), move || {
                    let code = code.clone();
                    with_app(move |app| app.jam_join_code(code));
                });
                self.timers.borrow_mut().push(timer);
            }
            None => self.jam_join_code(invite.code),
        }
    }

    pub fn social_dismiss(&self, invite_id: &str) {
        self.social().invites.retain(|(i, _)| i.id != invite_id);
        self.social_render();
    }

    // ------------------------------------------------------------------ Ereignisse

    pub fn social_event(&self, event: Event) {
        match event {
            Event::Connected => {}
            Event::Disconnected => {
                self.social().connected = false;
                self.ui.set_social_connected(false);
            }
            Event::Ready(user) => {
                {
                    let mut config = self.config.borrow_mut();
                    if config.account.name != user.name {
                        config.account.name = user.name.clone();
                        config::save(&config);
                    }
                }
                self.social_apply_user(&user);
                {
                    let mut state = self.social();
                    state.user = Some(user);
                    state.connected = true;
                    state.last_presence = None;
                }
                self.ui.set_social_connected(true);
                self.social_observe_media();
            }
            Event::Friends(list) => {
                self.social().friends = list;
                self.social_render();
            }
            Event::Invite(invite) => {
                log::line(&format!("social: Einladung in Sync {} erhalten", invite.code));
                {
                    let mut state = self.social();
                    state.invites.retain(|(i, _)| i.from.id != invite.from.id);
                    state.invites.push((invite, Instant::now()));
                }
                self.social_render();
                // Fenster auf sich aufmerksam machen, falls im Hintergrund.
                self.ui.window().request_redraw();
            }
            Event::Invited { delivered } => {
                self.ui.set_social_friends_status(tr(if delivered { "social.status.invited" } else { "social.status.invited_later" }).into());
            }
            Event::Error { error, context } => {
                log::line(&format!("social: Fehler {error} ({context})"));
                if context == "invite" {
                    self.ui.set_social_friends_status(error_text(&error).into());
                }
            }
            Event::SignedOut(reason) => {
                log::line(&format!("social: abgemeldet ({reason})"));
                self.social_forget(error_text(if reason == "unauthorized" { "session_expired" } else { &reason }));
            }
        }
    }

    /// Bei jeder Änderung des Players (alle 2 s aus on_media): eigenen Titel an die Freunde.
    pub fn social_observe_media(&self) {
        if !self.social.borrow().connected {
            return;
        }
        let info = self.active_media();
        let payload = social::presence_payload(info.as_ref());
        let mut state = self.social();
        if state.last_presence.as_ref() == Some(&payload) {
            return;
        }
        state.last_presence = Some(payload.clone());
        if let Some(connection) = state.connection.as_ref() {
            connection.send(payload);
        }
    }

    // ------------------------------------------------------------------ Anzeige

    fn social_image(&self, url: &str) -> Option<slint::Image> {
        let cached = self.social.borrow().images.get(url).cloned();
        if let Some(buffer) = cached {
            return Some(slint::Image::from_rgba8(buffer));
        }
        let start = self.social().loading.insert(url.to_string());
        if start {
            let url = url.to_string();
            self.runtime.spawn(async move {
                let buffer = music::download_image(&url).await;
                on_ui(move |app| {
                    let mut state = app.social();
                    state.loading.remove(&url);
                    if let Some(buffer) = buffer {
                        if state.images.len() > 200 {
                            state.images.clear();
                        }
                        state.images.insert(url, buffer);
                        drop(state);
                        app.social_render();
                    }
                });
            });
        }
        None
    }

    pub fn social_render(&self) {
        // Abgelaufene Einladungen entfernen.
        self.social().invites.retain(|(_, at)| at.elapsed() < INVITE_TTL);
        let state = self.social.borrow();
        let invited = if Some(&state.invited.0) == self.jam_code().as_ref() { state.invited.1.clone() } else { HashSet::new() };
        let friends: Vec<(social::Friend, bool)> = state
            .friends
            .friends
            .iter()
            .map(|f| (f.clone(), invited.contains(&f.id) || state.pending_invites.contains(&f.id)))
            .collect();
        let incoming = state.friends.incoming.clone();
        let outgoing = state.friends.outgoing.clone();
        let results = state.results.clone();
        let invites: Vec<social::Invite> = state.invites.iter().map(|(i, _)| i.clone()).collect();
        drop(state);

        let friend_rows: Vec<SocialFriend> = friends
            .into_iter()
            .map(|(f, invited)| {
                let (status, info) = match (f.status.as_str(), &f.track) {
                    ("listening", Some(track)) => {
                        let text = if track.artist.is_empty() { track.title.clone() } else { format!("{} – {}", track.title, track.artist) };
                        (2, format!("♪ {text}"))
                    }
                    ("offline", _) => (0, tr("social.state.offline").to_string()),
                    _ => (
                        1,
                        match f.client.as_deref() {
                            Some("android") => tr("social.state.online_android").to_string(),
                            Some("pc") => tr("social.state.online_pc").to_string(),
                            _ => tr("social.state.online").to_string(),
                        },
                    ),
                };
                let image = f.track.as_ref().and_then(|t| t.thumb.as_deref()).and_then(|url| self.social_image(url));
                SocialFriend {
                    id: f.id.clone().into(),
                    initial: initial(&f.name).into(),
                    name: f.name.into(),
                    status,
                    info: info.into(),
                    has_image: image.is_some(),
                    image: image.unwrap_or_default(),
                    invited,
                }
            })
            .collect();
        let person = |p: &social::Person| SocialPerson {
            id: p.id.clone().into(),
            name: p.name.clone().into(),
            info: match p.relation.as_str() {
                "friend" => tr("social.rel.friend"),
                "outgoing" => tr("social.rel.outgoing"),
                "incoming" => tr("social.rel.incoming"),
                _ => "",
            }
            .into(),
            can_add: p.relation == "none" || p.relation.is_empty(),
            can_accept: p.relation == "incoming",
        };
        let invite_rows: Vec<SocialInvite> = invites
            .iter()
            .map(|i| SocialInvite {
                id: i.id.clone().into(),
                text: trf("social.invite_from", &[("name", &i.from.name)]).into(),
                detail: i
                    .current
                    .as_ref()
                    .map(|c| trf("social.invite_playing", &[("title", &c.title)]))
                    .unwrap_or_default()
                    .into(),
            })
            .collect();

        self.ui.set_social_badge((incoming.len() + invite_rows.len()) as i32);
        self.ui.set_social_friends(ModelRc::new(VecModel::from(friend_rows)));
        self.ui.set_social_incoming(ModelRc::new(VecModel::from(incoming.iter().map(person).collect::<Vec<_>>())));
        self.ui.set_social_outgoing(ModelRc::new(VecModel::from(outgoing.iter().map(person).collect::<Vec<_>>())));
        self.ui.set_social_results(ModelRc::new(VecModel::from(results.iter().map(person).collect::<Vec<_>>())));
        self.ui.set_social_invites(ModelRc::new(VecModel::from(invite_rows)));
    }
}
