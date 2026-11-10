//! Ein WebView2-Tab (YouTube oder YouTube Music) als Kindfenster des Playtube-Fensters, mit
//! eigenem dauerhaftem Profil (Login), Audio-Ausgabe pro Tab und Vollbild-Unterstützung.

use crate::scripts;
use std::cell::RefCell;
use std::rc::Rc;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2, ICoreWebView2Profile4, ICoreWebView2_13, COREWEBVIEW2_PERMISSION_KIND_MICROPHONE,
    COREWEBVIEW2_PERMISSION_STATE_ALLOW, COREWEBVIEW2_PERMISSION_STATE_DEFAULT,
};
use webview2_com::{
    AddScriptToExecuteOnDocumentCreatedCompletedHandler, ContainsFullScreenElementChangedEventHandler,
    SetPermissionStateCompletedHandler,
};
use windows::core::{Interface, HSTRING};
use wry::dpi::{LogicalPosition, LogicalSize};
use wry::{BackgroundThrottlingPolicy, NewWindowResponse, PageLoadEvent, Rect, WebContext, WebView, WebViewBuilder, WebViewExtWindows};

const YOUTUBE_ORIGINS: [&str; 2] = ["https://www.youtube.com", "https://music.youtube.com"];

/// Ereignisse eines Tabs an die App (laufen im UI-Thread).
pub trait TabEvents: 'static {
    fn url_changed(&self, tab: usize, url: String);
    fn load_finished(&self, tab: usize);
    fn fullscreen_changed(&self, tab: usize, fullscreen: bool);
    /// Ein Link wollte ein neues Fenster öffnen - stattdessen im selben Tab laden.
    fn open_url(&self, tab: usize, url: String);
    /// Nachricht unserer Skripte in der Seite (window.ipc.postMessage), z. B. Ton für Playtube.
    fn ipc(&self, tab: usize, body: String);
}

pub struct Tab {
    pub index: usize,
    pub home: String,
    pub webview: WebView,
    pub url: RefCell<String>,
    router_script_id: Rc<RefCell<Option<String>>>,
    audio_device: RefCell<String>,
}

impl Tab {
    pub fn new<W: wry::raw_window_handle::HasWindowHandle>(
        parent: &W,
        context: &mut WebContext,
        index: usize,
        home: &str,
        events: Rc<dyn TabEvents>,
        bounds: Rect,
        audio_tap: bool,
    ) -> wry::Result<Rc<Self>> {
        let page_events = events.clone();
        let window_events = events.clone();
        let ipc_events = events.clone();
        let webview = WebViewBuilder::new_with_web_context(context)
            .with_url(home)
            .with_bounds(bounds)
            .with_autoplay(true)
            .with_devtools(cfg!(debug_assertions))
            // Musik soll auch bei minimiertem/verstecktem Fenster ungebremst weiterlaufen.
            .with_background_throttling(BackgroundThrottlingPolicy::Disabled)
            .with_initialization_script(scripts::in_document(&scripts::audio_router("")))
            // Keine "Website verlassen?"-Abfrage, wenn Playtube selbst eine neue Seite lädt.
            .with_initialization_script(scripts::NO_LEAVE_PROMPT)
            // Playtube-Ton (src/relay.rs): Ton der Seite abgreifen und über Playtube.exe ausgeben.
            .with_initialization_script(scripts::audio_tap(audio_tap))
            .with_ipc_handler(move |request| ipc_events.ipc(index, request.into_body()))
            // Links mit target=_blank im selben Tab öffnen statt in einem neuen Fenster.
            .with_new_window_req_handler(move |url, _features| {
                window_events.open_url(index, url);
                NewWindowResponse::Deny
            })
            .with_on_page_load_handler(move |event, url| match event {
                PageLoadEvent::Started => page_events.url_changed(index, url),
                PageLoadEvent::Finished => {
                    page_events.url_changed(index, url);
                    page_events.load_finished(index);
                }
            })
            .build_as_child(parent)?;

        let tab = Rc::new(Self {
            index,
            home: home.to_string(),
            webview,
            url: RefCell::new(home.to_string()),
            router_script_id: Rc::new(RefCell::new(None)),
            audio_device: RefCell::new(String::new()),
        });
        tab.watch_fullscreen(events);
        Ok(tab)
    }

    fn core(&self) -> ICoreWebView2 {
        self.webview.webview()
    }

    fn watch_fullscreen(&self, events: Rc<dyn TabEvents>) {
        let index = self.index;
        let handler = ContainsFullScreenElementChangedEventHandler::create(Box::new(move |sender, _| {
            if let Some(core) = sender {
                let mut full = windows::core::BOOL::default();
                unsafe { core.ContainsFullScreenElement(&mut full)? };
                events.fullscreen_changed(index, full.as_bool());
            }
            Ok(())
        }));
        let mut token = 0i64;
        unsafe {
            let _ = self.core().add_ContainsFullScreenElementChanged(&handler, &mut token);
        }
    }

    pub fn set_bounds(&self, x: f64, y: f64, width: f64, height: f64) {
        let _ = self.webview.set_bounds(Rect {
            position: LogicalPosition::new(x, y).into(),
            size: LogicalSize::new(width.max(1.0), height.max(1.0)).into(),
        });
    }

    pub fn set_visible(&self, visible: bool) {
        let _ = self.webview.set_visible(visible);
    }

    pub fn eval(&self, js: &str) {
        let _ = self.webview.evaluate_script(js);
    }

    /// Führt `js` aus und liefert das (dekodierte) Ergebnis über `callback` (im UI-Thread).
    pub fn eval_with(&self, js: &str, callback: impl Fn(Option<serde_json::Value>) + Send + 'static) {
        let _ = self.webview.evaluate_script_with_callback(js, move |raw| callback(scripts::decode_result(&raw)));
    }

    pub fn load(&self, url: &str) {
        let _ = self.webview.load_url(url);
    }

    pub fn back(&self) {
        let _ = self.webview.go_back();
    }

    pub fn forward(&self) {
        let _ = self.webview.go_forward();
    }

    pub fn reload(&self) {
        let _ = self.webview.reload();
    }

    pub fn go_home(&self) {
        self.load(&self.home.clone());
    }

    /// Legt den Ton dieses Tabs auf `device` (leer = Systemstandard): sofort auf der offenen
    /// Seite und für jede weitere Seite schon beim Dokumentstart.
    pub fn set_audio_output(&self, device: &str) {
        let changed = *self.audio_device.borrow() != device;
        if !changed {
            return;
        }
        *self.audio_device.borrow_mut() = device.to_string();
        let core = self.core();
        if let Some(old) = self.router_script_id.borrow_mut().take() {
            unsafe {
                let _ = core.RemoveScriptToExecuteOnDocumentCreated(&HSTRING::from(old));
            }
        }
        let source = scripts::audio_router(device);
        if !device.is_empty() {
            let slot = self.router_script_id.clone();
            let handler = AddScriptToExecuteOnDocumentCreatedCompletedHandler::create(Box::new(move |result, id| {
                if result.is_ok() {
                    *slot.borrow_mut() = Some(id);
                }
                Ok(())
            }));
            unsafe {
                let _ = core.AddScriptToExecuteOnDocumentCreated(&HSTRING::from(scripts::in_document(&source)), &handler);
            }
        }
        // Bereits geladene Seite sofort umstellen (auch zurück auf den Systemstandard).
        self.eval(&source);
    }

    /// Erlaubt der Seite, die Namen der Audiogeräte zu sehen (Voraussetzung für die Audio-
    /// Ausgabe pro Tab). Es wird nichts aufgenommen.
    pub fn set_device_names_allowed(&self, allowed: bool) {
        let state = if allowed { COREWEBVIEW2_PERMISSION_STATE_ALLOW } else { COREWEBVIEW2_PERMISSION_STATE_DEFAULT };
        let Ok(core13) = self.core().cast::<ICoreWebView2_13>() else { return };
        let Ok(profile) = (unsafe { core13.Profile() }) else { return };
        let Ok(profile4) = profile.cast::<ICoreWebView2Profile4>() else { return };
        for origin in YOUTUBE_ORIGINS {
            let handler = SetPermissionStateCompletedHandler::create(Box::new(|_| Ok(())));
            unsafe {
                let _ = profile4.SetPermissionState(COREWEBVIEW2_PERMISSION_KIND_MICROPHONE, &HSTRING::from(origin), state, &handler);
            }
        }
    }
}
