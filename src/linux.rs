//! Linux-Besonderheiten: X11 erzwingen, GTK starten und dessen Ereignisse in der Slint-Schleife
//! mitverarbeiten.
//!
//! wry kann die WebKitGTK-Ansicht nur unter X11 als Kindfenster in ein fremdes Fenster (das
//! Slint-Fenster) setzen. Unter Wayland läuft Playtube deshalb über XWayland. WebKitGTK und das
//! Tray-Menü (tray-icon) brauchen außerdem eine laufende GTK-Schleife; Slint (winit) hat eine
//! eigene - also werden die GTK-Ereignisse regelmäßig aus einem Slint-Timer abgearbeitet.

use std::time::Duration;

/// Eingebettete Symbolschrift (ui/fonts.slint) statt der Windows-Schrift "Segoe MDL2 Assets".
pub const ICON_FONT: &str = "Playtube Icons";

/// Vor allem anderen in `main` aufrufen (noch ein einziger Thread).
pub fn prepare() {
    // SAFETY: Ganz am Anfang von main, bevor weitere Threads laufen.
    unsafe {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() && std::env::var_os("DISPLAY").is_some() {
            std::env::remove_var("WAYLAND_DISPLAY");
        }
        std::env::set_var("GDK_BACKEND", "x11");
        // WebKitGTK zeigt in Kindfenstern unter manchen Grafiktreibern sonst nur eine weiße Fläche.
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }
    if let Err(e) = gtk::init() {
        crate::log::line(&format!("linux: GTK nicht gestartet: {e}"));
    }
}

/// Arbeitet ausstehende GTK-Ereignisse ab (alle 8 ms), solange der Timer lebt.
pub fn start_gtk_pump() -> slint::Timer {
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(8), pump);
    timer
}

/// Alle ausstehenden GTK-Ereignisse abarbeiten (auch für Wartestellen, z. B. beim Beenden).
pub fn pump() {
    while gtk::events_pending() {
        gtk::main_iteration_do(false);
    }
}

/// Laufzeitordner für Sockets: $XDG_RUNTIME_DIR, sonst /tmp.
pub fn runtime_dir() -> std::path::PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_dir())
        .unwrap_or_else(std::env::temp_dir)
}
