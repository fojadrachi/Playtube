//! Sprache der Oberfläche: Deutsch oder Englisch (US). Die Sprache kommt aus config.json
//! ("language": "de" / "en", leer = Windows-Sprache) und lässt sich ohne Neustart umschalten.

use std::sync::atomic::{AtomicBool, Ordering};

static ENGLISH: AtomicBool = AtomicBool::new(false);

pub const LANGUAGES: [(&str, &str); 2] = [("de", "Deutsch"), ("en", "English (US)")];

/// Gespeicherter Wert -> "de"/"en" (leer/unbekannt = Windows-Sprache).
pub fn resolve(code: &str) -> &'static str {
    match code {
        "de" => "de",
        "en" => "en",
        _ => system_language(),
    }
}

fn system_language() -> &'static str {
    use windows::Win32::Globalization::GetUserDefaultUILanguage;
    // Primäre Sprach-ID 0x07 = Deutsch.
    let lang = unsafe { GetUserDefaultUILanguage() };
    if lang & 0x3ff == 0x07 { "de" } else { "en" }
}

pub fn set_language(code: &str) -> &'static str {
    let resolved = resolve(code);
    ENGLISH.store(resolved == "en", Ordering::Relaxed);
    resolved
}

pub fn current() -> &'static str {
    if ENGLISH.load(Ordering::Relaxed) { "en" } else { "de" }
}

/// Text zum Schlüssel in der aktuellen Sprache. Platzhalter wie "{app}" ersetzt [`trf`].
pub fn tr(key: &str) -> &'static str {
    let en = ENGLISH.load(Ordering::Relaxed);
    for (k, de, english) in STRINGS {
        if *k == key {
            return if en { english } else { de };
        }
    }
    "?"
}

/// [`tr`] mit Platzhaltern: `trf("x", &[("app", "Playtube")])` ersetzt "{app}".
pub fn trf(key: &str, args: &[(&str, &str)]) -> String {
    let mut text = tr(key).to_string();
    for (name, value) in args {
        text = text.replace(&format!("{{{name}}}"), value);
    }
    text
}

const STRINGS: &[(&str, &str, &str)] = &[
    // Hauptfenster / Tray
    ("tab.settings", "Einstellungen", "Settings"),
    ("url.placeholder", "Suche oder URL eingeben …", "Search or enter URL …"),
    ("tray.show", "Anzeigen", "Show"),
    ("tray.play_pause", "Wiedergabe umschalten", "Play/Pause"),
    ("tray.next", "Nächster Titel", "Next track"),
    ("tray.previous", "Vorheriger Titel", "Previous track"),
    ("tray.quit", "Beenden", "Quit"),
    ("tray.background",
     "{app} läuft im Hintergrund weiter. Rechtsklick auf das Tray-Icon zum Beenden.",
     "{app} keeps running in the background. Right-click the tray icon to quit."),
    // Einstellungen
    ("settings.title", "{app}-Einstellungen", "{app} Settings"),
    ("settings.version", "Version {version}", "Version {version}"),
    ("settings.save", "Speichern", "Save"),
    ("settings.saved", "Gespeichert und übernommen.", "Saved and applied."),
    ("section.discord", "Discord Rich Presence", "Discord Rich Presence"),
    ("section.updates", "Automatische Updates", "Automatic updates"),
    ("section.audio", "Audioausgabe", "Audio output"),
    ("section.phone", "Steuerung vom Handy", "Control from your phone"),
    ("section.general", "Allgemein", "General"),
    ("field.enabled", "Aktiviert", "Enabled"),
    ("field.client_id", "Client-ID", "Client ID"),
    ("field.update_interval", "Update-Intervall (Sekunden)", "Update interval (seconds)"),
    ("field.check_interval", "Prüfintervall (Stunden)", "Check interval (hours)"),
    ("field.youtube", "YouTube", "YouTube"),
    ("field.music", "YouTube Music", "YouTube Music"),
    ("field.start_tab", "Beim Start öffnen", "Open on startup"),
    ("field.language", "Sprache", "Language"),
    ("discord.show_idle", "Status anzeigen, wenn gerade nichts läuft", "Show status when nothing is playing"),
    ("updates.check_now", "Jetzt nach Updates suchen", "Check for updates now"),
    ("updates.install", "Update installieren", "Install update"),
    ("updates.checking", "Suche nach Updates …", "Checking for updates …"),
    ("updates.available", "Neue Version verfügbar: {version}", "New version available: {version}"),
    ("updates.up_to_date", "Du hast bereits die neueste Version ({version}).", "You already have the latest version ({version})."),
    ("updates.failed", "Update-Prüfung fehlgeschlagen (keine Internetverbindung?).", "Update check failed (no internet connection?)."),
    ("updates.downloading", "Lade Update herunter …", "Downloading update …"),
    ("updates.installing", "Installiere Update – {app} startet gleich neu …", "Installing update – {app} restarts in a moment …"),
    ("updates.install_failed", "Update fehlgeschlagen: {error}", "Update failed: {error}"),
    ("updates.portable",
     "Diese Kopie ist nicht installiert – lade das Setup von der Releases-Seite.",
     "This copy is not installed – download the setup from the releases page."),
    ("audio.system_default", "Systemstandard", "System default"),
    ("audio.unavailable", "{name} (nicht verfügbar)", "{name} (not available)"),
    ("audio.hint",
     "Legt den Ton von YouTube und YouTube Music auf verschiedene Ausgabegeräte, z. B. getrennte Sonar-Kanäle. Damit die Seite das Gerät findet, darf sie die Namen der Audiogeräte sehen (es wird nichts aufgenommen).",
     "Sends the sound of YouTube and YouTube Music to different output devices, e.g. separate Sonar channels. So the page can find the device, it may read the names of your audio devices (nothing is recorded)."),
    ("phone.enabled", "Steuerung vom Handy im selben WLAN erlauben", "Allow control from a phone on the same Wi-Fi"),
    ("phone.hint",
     "Mit Playtube für Android steuerst du {app} auf diesem PC wie bei Spotify. Einschalten, speichern, „Handy koppeln“ wählen und den Code in der Android-App eingeben. Windows fragt beim ersten Mal nach der Firewall – für private Netzwerke erlauben.",
     "With Playtube for Android you control {app} on this PC like with Spotify. Turn it on, save, choose “Pair phone” and enter the code in the Android app. The first time, Windows asks about the firewall – allow it for private networks."),
    ("phone.pair", "Handy koppeln", "Pair phone"),
    ("phone.forget", "Alle Kopplungen entfernen", "Remove all pairings"),
    ("phone.code", "Code: {code}  (2 Minuten gültig)", "Code: {code}  (valid for 2 minutes)"),
    ("phone.off", "Ausgeschaltet.", "Turned off."),
    ("phone.not_running", "Konnte nicht starten (Port {port} belegt?).", "Could not start (port {port} in use?)."),
    ("phone.address", "Adresse dieses PCs: {addresses}", "This PC's address: {addresses}"),
    ("phone.devices", "Gekoppelt: {names}", "Paired: {names}"),
    ("phone.no_devices", "Noch kein Handy gekoppelt.", "No phone paired yet."),
    // Discord
    ("rpc.unknown_title", "Unbekannter Titel", "Unknown title"),
    ("rpc.playing", "Spielt", "Playing"),
    ("rpc.paused", "Pausiert", "Paused"),
    ("rpc.open_music", "In YouTube Music öffnen", "Open in YouTube Music"),
    ("rpc.watch_youtube", "Auf YouTube ansehen", "Watch on YouTube"),
    ("rpc.idle_details", "Bei {app}", "In {app}"),
    ("rpc.idle_state", "Stöbert gerade", "Browsing"),
    ("rpc.settings_details", "In den Einstellungen", "In the settings"),
    ("rpc.settings_state", "Schaut sich die Einstellungen an", "Looking at the settings"),
    // Fehler
    ("error.webview",
     "Die Microsoft Edge WebView2 Runtime fehlt oder startet nicht. Bitte von https://go.microsoft.com/fwlink/p/?LinkId=2124703 installieren.",
     "The Microsoft Edge WebView2 Runtime is missing or fails to start. Please install it from https://go.microsoft.com/fwlink/p/?LinkId=2124703."),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_is_unique_and_filled() {
        for (i, (key, de, en)) in STRINGS.iter().enumerate() {
            assert!(!de.is_empty() && !en.is_empty(), "{key}");
            assert!(STRINGS[i + 1..].iter().all(|(k, _, _)| k != key), "doppelt: {key}");
            // Platzhalter müssen in beiden Sprachen gleich sein.
            let holes = |s: &str| s.split('{').skip(1).filter_map(|p| p.split('}').next()).map(str::to_string).collect::<Vec<_>>();
            assert_eq!(holes(de), holes(en), "{key}");
        }
    }

    #[test]
    fn formats_placeholders() {
        set_language("de");
        assert_eq!(trf("settings.title", &[("app", "Playtube")]), "Playtube-Einstellungen");
        set_language("en");
        assert_eq!(trf("settings.title", &[("app", "Playtube")]), "Playtube Settings");
    }
}
