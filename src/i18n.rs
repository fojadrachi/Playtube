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
    ("field.music_view", "YouTube Music anzeigen als", "Show YouTube Music as"),
    ("field.music_view.playtube", "Playtube-Ansicht", "Playtube view"),
    ("field.music_view.original", "Original YouTube Music", "Original YouTube Music"),
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
    ("audio.mode", "Tonausgabe", "Sound output"),
    ("audio.mode.normal", "Normal (beste Qualität)", "Normal (best quality)"),
    ("audio.mode.stream", "Für Discord-Streams (Ton über Playtube)", "For Discord streams (sound through Playtube)"),
    ("audio.mode.hint",
     "„Normal“ spielt den Ton direkt wie in früheren Versionen. „Für Discord-Streams“ gibt ihn über Playtube.exe aus: Dann hören Zuschauer die Musik, wenn du in Discord nur das Playtube-Fenster teilst, und Mixer wie SteelSeries Sonar zeigen „Playtube“ statt „msedgewebview2“. Der Ton kommt dabei etwa 0,15 s später.",
     "“Normal” plays the sound directly like earlier versions. “For Discord streams” outputs it through Playtube.exe: viewers then hear the music when you share only the Playtube window in Discord, and mixers like SteelSeries Sonar show “Playtube” instead of “msedgewebview2”. The sound is about 0.15 s later."),
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
    // Playtube Jam
    ("tab.jam", "Jam", "Jam"),
    ("jam.title", "Playtube Jam", "Playtube Jam"),
    ("jam.intro",
     "Hört gemeinsam wie bei Spotify Jam: jeder auf seinem eigenen Gerät, synchron, und alle wünschen sich Songs. Freunde ohne Playtube machen über den Link im Browser mit.",
     "Listen together like Spotify Jam: everyone on their own device, in sync, and everyone adds songs. Friends without Playtube join in the browser via the link."),
    ("jam.name", "Dein Name", "Your name"),
    ("jam.start", "Jam starten", "Start jam"),
    ("jam.join", "Beitreten", "Join"),
    ("jam.code_placeholder", "Code oder Einladungslink", "Code or invite link"),
    ("jam.invite", "Freunde einladen", "Invite friends"),
    ("jam.copy", "Link kopieren", "Copy link"),
    ("jam.copied", "Link kopiert.", "Link copied."),
    ("jam.all_control", "Alle dürfen steuern", "Everyone can control"),
    ("jam.end", "Jam beenden", "End jam"),
    ("jam.leave", "Jam verlassen", "Leave jam"),
    ("jam.members", "Dabei", "Listening"),
    ("jam.queue", "Als Nächstes", "Up next"),
    ("jam.queue_empty", "Die Warteschlange ist leer.", "The queue is empty."),
    ("jam.add", "Hinzufügen", "Add"),
    ("jam.search", "Suchen", "Search"),
    ("jam.add_placeholder", "YouTube-Link oder Video-ID", "YouTube link or video ID"),
    ("jam.search_placeholder", "Song suchen oder Link einfügen", "Search a song or paste a link"),
    ("jam.add_current", "Video aus dem YouTube-Tab hinzufügen", "Add the video from the YouTube tab"),
    ("jam.results", "Suchergebnisse", "Search results"),
    ("jam.nothing", "Noch läuft nichts – füge den ersten Song hinzu.", "Nothing is playing yet – add the first song."),
    ("jam.by", "von {name}", "by {name}"),
    ("jam.host", "Host", "Host"),
    ("jam.you", "du", "you"),
    ("jam.offline", "offline", "offline"),
    ("jam.cant_play", "kann diesen Titel nicht abspielen", "can't play this track"),
    ("jam.cant_play_here",
     "Dieser Titel lässt sich hier nicht abspielen (z. B. Region) – er läuft bei den anderen weiter.",
     "This track can't be played here (e.g. region lock) – it keeps playing for the others."),
    ("jam.hint",
     "Die Musik läuft im YouTube-Music-Tab. Dort kannst du weiter stöbern: Tippst du einen Song an, landet er wie bei Spotify im Jam. Mit Steuerrecht wirken Play/Pause und Weiter dort, im Tray, am Stream Dock und vom Handy für alle.",
     "The music plays in the YouTube Music tab. You can keep browsing there: tap a song and it joins the jam, like on Spotify. With control rights, play/pause and skip there, in the tray, on the Stream Dock and from your phone apply to everyone."),
    ("jam.kick", "Entfernen", "Remove"),
    ("jam.play_now", "Jetzt spielen", "Play now"),
    ("jam.remove", "Entfernen", "Remove"),
    ("jam.add_next", "Als Nächstes", "Play next"),
    ("jam.client.pc", "Playtube PC", "Playtube PC"),
    ("jam.client.android", "Android", "Android"),
    ("jam.client.web", "Browser", "Browser"),
    ("jam.status.creating", "Jam wird gestartet …", "Starting jam …"),
    ("jam.status.joining", "Verbinde …", "Connecting …"),
    ("jam.status.connected", "Verbunden.", "Connected."),
    ("jam.status.reconnecting", "Verbindung unterbrochen – verbinde neu …", "Connection lost – reconnecting …"),
    ("jam.status.bad_code", "Das ist kein gültiger Code (6 Zeichen).", "That is not a valid code (6 characters)."),
    ("jam.status.not_found", "Diesen Jam gibt es nicht (mehr).", "This jam does not exist (anymore)."),
    ("jam.status.failed", "Keine Verbindung zum Jam-Server.", "Can't reach the jam server."),
    ("jam.status.need_name", "Bitte gib zuerst deinen Namen ein.", "Please enter your name first."),
    ("jam.status.in_other", "Du bist schon in einem Jam – verlasse ihn zuerst.", "You are already in a jam – leave it first."),
    ("jam.ended.host_ended", "Der Host hat den Jam beendet.", "The host ended the jam."),
    ("jam.ended.host_left", "Der Jam ist zu Ende.", "The jam is over."),
    ("jam.ended.expired", "Der Jam ist abgelaufen.", "The jam has expired."),
    ("jam.ended.kicked", "Du wurdest aus dem Jam entfernt.", "You were removed from the jam."),
    ("jam.ended.left", "Du hast den Jam verlassen.", "You left the jam."),
    ("jam.ended.full", "Der Jam ist voll.", "The jam is full."),
    ("jam.error.not_allowed", "Das darf nur der Host.", "Only the host can do that."),
    ("jam.error.queue_full", "Die Warteschlange ist voll.", "The queue is full."),
    ("jam.error.bad_video", "Das ist kein YouTube-Link.", "That is not a YouTube link."),
    ("jam.error.no_current", "Im YouTube-Tab läuft gerade kein Video.", "No video is open in the YouTube tab."),
    ("jam.error.search", "Die Suche geht gerade nicht.", "Search is not working right now."),
    ("jam.error.rate_limited", "Langsam – zu viele Aktionen auf einmal.", "Slow down – too many actions at once."),
    ("jam.error.other", "Fehler: {error}", "Error: {error}"),
    ("jam.added", "Hinzugefügt: {title}", "Added: {title}"),
    // Playtube-Ansicht (YouTube Music im Spotify-Stil)
    ("music.home", "Startseite", "Home"),
    ("music.search", "Suchen", "Search"),
    ("music.library", "Bibliothek", "Your Library"),
    ("music.your_playlists", "Deine Playlists", "Your playlists"),
    ("music.liked", "Lieblingssongs", "Liked songs"),
    ("music.liked_sub", "Playlist • Mag ich", "Playlist • Liked"),
    ("music.play", "Abspielen", "Play"),
    ("music.queue", "Warteschlange", "Queue"),
    ("music.queue_empty", "Spiel etwas ab – dann steht hier, was als Nächstes kommt.", "Play something – then you will see what's up next."),
    ("music.login", "Anmelden", "Sign in"),
    ("music.switch", "Fertig – zurück zu Playtube", "Done – back to Playtube"),
    ("music.loading", "Lädt …", "Loading …"),
    ("music.login_hint",
     "Nicht angemeldet – melde dich an, um deine Bibliothek, Playlists und Empfehlungen zu sehen.",
     "Not signed in – sign in to see your library, playlists and recommendations."),
    ("music.nothing", "Hier gibt es nichts.", "Nothing here."),
    ("music.top_result", "Top-Ergebnis", "Top result"),
    ("music.results", "Ergebnisse", "Results"),
    ("music.search_placeholder", "Was möchtest du hören?", "What do you want to listen to?"),
    ("music.error", "YouTube Music hat nicht geantwortet. Neu laden versuchen.", "YouTube Music did not respond. Try reloading."),
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
