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

#[cfg(not(windows))]
fn system_language() -> &'static str {
    // Wie gettext: LC_ALL, dann LC_MESSAGES, dann LANG (z. B. "de_DE.UTF-8").
    let value = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .find(|v| !v.is_empty())
        .unwrap_or_default();
    if value.starts_with("de") { "de" } else { "en" }
}

#[cfg(windows)]
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
    // Verlauf, Schlaf-Timer, Mini-Player (src/extras.rs)
    ("music.history", "Verlauf", "History"),
    ("music.history_placeholder", "Im Verlauf suchen …", "Search your history …"),
    ("music.history_empty", "Noch nichts gehört – Songs erscheinen hier nach 30 Sekunden.", "Nothing played yet – songs show up here after 30 seconds."),
    ("history.today", "Heute", "Today"),
    ("history.yesterday", "Gestern", "Yesterday"),
    ("history.clear", "Verlauf löschen", "Clear history"),
    ("history.cleared", "Verlauf gelöscht", "History cleared"),
    ("field.history", "Gehörte Songs im Verlauf speichern", "Keep a history of played songs"),
    ("sleep.title", "Schlaf-Timer", "Sleep timer"),
    ("sleep.off", "Aus", "Off"),
    ("sleep.end", "Ende des Songs", "End of song"),
    ("sleep.end_short", "Songende", "End of song"),
    ("sleep.minutes", "Minuten", "minutes"),
    ("mini.open", "Mini-Player", "Mini player"),
    ("mini.idle", "Gerade läuft nichts", "Nothing playing"),
    // Discord-Bot im Sync (src/botaudio.rs)
    ("bot.listening", "🎧 {bot} spielt den Sync im Discord-Sprachkanal (Ton von diesem PC)", "🎧 {bot} is playing the sync in a Discord voice channel (sound from this PC)"),
    ("bot.stopped", "Discord-Bot bekommt keinen Ton mehr", "Discord bot no longer receives sound"),
    ("bot.setting", "Discord-Bot im Sync darf den Ton von diesem PC bekommen", "A Discord bot in the sync may receive the sound from this PC"),
    ("bot.setting_hint", "Mit /jam connect <Code> spielt ein Discord-Bot den Sync im Sprachkanal ab. Den Ton schickt Playtube von hier – solange das läuft, kommt er über Playtube.exe (wie beim Modus „Für Discord-Streams“).", "With /jam connect <code> a Discord bot plays the jam in a voice channel. Playtube sends the sound from here – while it does, the sound comes through Playtube.exe (like the \"For Discord streams\" mode)."),
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
    ("section.diag", "Diagnose", "Diagnostics"),
    ("diag.enabled", "Diagnosedaten senden (hilft, Fehler zu finden)", "Send diagnostic data (helps to find bugs)"),
    ("diag.hint",
     "Gesendet werden Fehlermeldungen und Auszüge aus dem Playtube-Log, Version und Windows-Version mit einer zufälligen Kennung – keine Cookies, Anmeldedaten, Suchanfragen, Songtitel oder Video-IDs. Gespeichert 30 Tage. Details: playtube-ppp-eula.pages.dev/playtube/privacy/#diag",
     "Sent are error messages and excerpts from the Playtube log, version and Windows version with a random ID – no cookies, sign-in data, searches, song titles or video IDs. Stored for 30 days. Details: playtube-ppp-eula.pages.dev/playtube/privacy/#diag"),
    ("diag.send_now", "Jetzt senden", "Send now"),
    ("diag.sending", "Sende …", "Sending …"),
    ("diag.sent", "Gesendet. Kennung dieser Installation: {id}", "Sent. ID of this installation: {id}"),
    ("diag.failed", "Senden fehlgeschlagen: {error}", "Sending failed: {error}"),
    ("field.enabled", "Aktiviert", "Enabled"),
    ("field.check_interval", "Prüfintervall (Stunden)", "Check interval (hours)"),
    ("field.youtube", "YouTube", "YouTube"),
    ("field.music", "YouTube Music", "YouTube Music"),
    ("field.start_tab", "Beim Start öffnen", "Open on startup"),
    ("field.language", "Sprache", "Language"),
    ("field.music_view", "YouTube Music anzeigen als", "Show YouTube Music as"),
    ("field.media_keys",
     "Medientasten der Tastatur steuern Playtube (Play/Pause, Weiter, Zurück – auch im Hintergrund)",
     "Keyboard media keys control Playtube (play/pause, next, previous – also in the background)"),
    ("field.autostart", "Mit Windows starten", "Start with Windows"),
    ("field.autostart.linux", "Mit dem System starten", "Start with the system"),
    ("field.autostart_tray", "Dabei nur im Tray starten (ohne Fenster)", "Start in the tray only (no window)"),
    ("field.music_view.playtube", "Playtube-Ansicht", "Playtube view"),
    ("field.music_view.original", "Original YouTube Music", "Original YouTube Music"),
    ("discord.show_idle", "Status anzeigen, wenn gerade nichts läuft", "Show status when nothing is playing"),
    ("updates.check_now", "Jetzt nach Updates suchen", "Check for updates now"),
    ("updates.install", "Update installieren", "Install update"),
    ("updates.auto", "Kleine Updates automatisch im Hintergrund installieren", "Install small updates automatically in the background"),
    ("updates.staged", "Update {version} ist installiert – aktiv beim nächsten Start.", "Update {version} is installed – active on next start."),
    ("updates.restart", "Jetzt neu starten", "Restart now"),
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
    ("audio.mode.auto", "Automatisch (Discord-Modus im Discord-Sprachkanal)", "Automatic (Discord mode while in a Discord voice channel)"),
    ("audio.mode.hint",
     "„Normal“ spielt den Ton direkt wie in früheren Versionen. „Für Discord-Streams“ gibt ihn über Playtube.exe aus: Dann hören Zuschauer die Musik, wenn du in Discord nur das Playtube-Fenster teilst, und Mixer wie SteelSeries Sonar zeigen „Playtube“ statt „msedgewebview2“. Der Ton kommt dabei etwa 0,15 s später. „Automatisch“ schaltet von selbst in den Discord-Modus, sobald du in Discord in einem Sprachkanal bist (nur von dort aus kann man streamen), und danach wieder zurück.",
     "“Normal” plays the sound directly like earlier versions. “For Discord streams” outputs it through Playtube.exe: viewers then hear the music when you share only the Playtube window in Discord, and mixers like SteelSeries Sonar show “Playtube” instead of “msedgewebview2”. The sound is about 0.15 s later. “Automatic” switches to Discord mode by itself as soon as you are in a Discord voice channel (the only place you can stream from) and back afterwards."),
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
    // Sozial: Konto und Freunde (src/social_ui.rs)
    ("social.tab", "Sozial", "Social"),
    ("social.title", "Sozial", "Social"),
    ("social.intro", "Mit einem Playtube-Konto siehst du, was deine Freunde gerade hören, und holst sie mit einem Klick in deinen Sync.", "With a Playtube account you can see what your friends are listening to and bring them into your Sync with one click."),
    ("social.login_title", "Anmelden", "Sign in"),
    ("social.register_title", "Konto erstellen", "Create account"),
    ("social.reset_title", "Passwort vergessen", "Forgot password"),
    ("social.login_field", "Benutzername oder E-Mail", "Username or email"),
    ("social.name_field", "Benutzername (3–20 Zeichen: A–Z, 0–9, _ . -)", "Username (3–20 characters: A–Z, 0–9, _ . -)"),
    ("social.email_field", "E-Mail-Adresse", "Email address"),
    ("social.password_field", "Passwort", "Password"),
    ("social.password2_field", "Passwort wiederholen", "Repeat password"),
    ("social.login", "Anmelden", "Sign in"),
    ("social.register", "Konto erstellen", "Create account"),
    ("social.send_reset", "Link schicken", "Send link"),
    ("social.to_register", "Neues Konto", "New account"),
    ("social.to_login", "Zur Anmeldung", "Back to sign in"),
    ("social.forgot", "Passwort vergessen?", "Forgot password?"),
    ("social.optional_hint", "Ein Konto ist freiwillig – Sync per Code oder Link geht weiterhin ohne. Passwort mindestens 8 Zeichen.", "An account is optional – Sync by code or link still works without one. Password at least 8 characters."),
    ("social.signed_in_as", "Angemeldet als", "Signed in as"),
    ("social.logout", "Abmelden", "Sign out"),
    ("social.share", "Freunde sehen, was ich höre", "Friends can see what I'm listening to"),
    ("social.share_hint", "Aus: Freunde sehen nur, ob du online bist.", "Off: friends only see whether you're online."),
    ("social.delete_account", "Konto löschen", "Delete account"),
    ("social.delete_confirm", "Endgültig löschen", "Delete for good"),
    ("social.delete_hint", "Löscht dein Konto, alle Freundschaften und Anfragen sofort. Zur Bestätigung dein Passwort:", "Deletes your account, all friendships and requests right away. Enter your password to confirm:"),
    ("social.cancel", "Abbrechen", "Cancel"),
    ("social.friends", "Freunde", "Friends"),
    ("social.no_friends", "Noch keine Freunde – such rechts nach ihrem Benutzernamen.", "No friends yet – search for their username on the right."),
    ("social.search_placeholder", "Benutzername", "Username"),
    ("social.search", "Freunde finden", "Find friends"),
    ("social.add", "Hinzufügen", "Add"),
    ("social.accept", "Annehmen", "Accept"),
    ("social.decline", "Ablehnen", "Decline"),
    ("social.withdraw", "Zurückziehen", "Withdraw"),
    ("social.remove_friend", "Freund entfernen", "Remove friend"),
    ("social.incoming", "Anfragen", "Requests"),
    ("social.outgoing", "Gesendete Anfragen", "Sent requests"),
    ("social.invite", "In Sync einladen", "Invite to Sync"),
    ("social.invite_hint", "Läuft noch kein Sync, startet Playtube beim Einladen einen.", "If no Sync is running, Playtube starts one when you invite."),
    ("social.invites", "Einladungen", "Invitations"),
    ("social.join", "Beitreten", "Join"),
    ("social.dismiss", "Ausblenden", "Dismiss"),
    ("social.offline", "Keine Verbindung zum Server – versuche es gleich wieder …", "No connection to the server – retrying …"),
    ("social.privacy", "Datenschutz", "Privacy"),
    ("social.invite_from", "{name} lädt dich in den Sync ein", "{name} invites you to their Sync"),
    ("social.invite_playing", "Läuft gerade: {title}", "Now playing: {title}"),
    ("social.state.offline", "Offline", "Offline"),
    ("social.state.online", "Online", "Online"),
    ("social.state.online_pc", "Online · PC", "Online · PC"),
    ("social.state.online_android", "Online · Android", "Online · Android"),
    ("social.rel.friend", "Schon befreundet", "Already friends"),
    ("social.rel.outgoing", "Anfrage gesendet", "Request sent"),
    ("social.rel.incoming", "Hat dir eine Anfrage geschickt", "Sent you a request"),
    ("social.status.working", "Einen Moment …", "One moment …"),
    ("social.status.fill_in", "Bitte alle Felder ausfüllen.", "Please fill in all fields."),
    ("social.status.mismatch", "Die beiden Passwörter sind nicht gleich.", "The two passwords don't match."),
    ("social.status.reset_sent", "Falls es ein Konto mit dieser Adresse gibt, ist ein Link unterwegs (60 Minuten gültig). Schau auch im Spam-Ordner.", "If there is an account with this address, a link is on its way (valid for 60 minutes). Check your spam folder too."),
    ("social.status.logged_out", "Abgemeldet.", "Signed out."),
    ("social.status.deleted", "Dein Konto ist gelöscht.", "Your account has been deleted."),
    ("social.status.search_short", "Mindestens 2 Zeichen eingeben.", "Enter at least 2 characters."),
    ("social.status.no_results", "Niemanden gefunden.", "Nobody found."),
    ("social.status.requested", "Anfrage gesendet.", "Request sent."),
    ("social.status.now_friends", "Ihr seid jetzt befreundet.", "You're friends now."),
    ("social.status.starting_sync", "Sync wird gestartet, dann geht die Einladung raus …", "Starting a Sync, then the invitation goes out …"),
    ("social.status.invited", "Einladung gesendet.", "Invitation sent."),
    ("social.status.invited_later", "Gerade nicht online – die Einladung wartet 15 Minuten auf deinen Freund.", "Not online right now – the invitation waits 15 minutes for your friend."),
    ("social.error.failed", "Das hat nicht geklappt. Bitte versuch es noch einmal.", "That didn't work. Please try again."),
    ("social.error.network", "Keine Verbindung zum Server.", "No connection to the server."),
    ("social.error.wrong_login", "Benutzername/E-Mail oder Passwort stimmt nicht.", "Username/email or password is wrong."),
    ("social.error.wrong_password", "Das Passwort stimmt nicht.", "The password is wrong."),
    ("social.error.locked", "Zu viele Fehlversuche. Das Konto ist 15 Minuten gesperrt – oder setze das Passwort zurück.", "Too many failed attempts. The account is locked for 15 minutes – or reset your password."),
    ("social.error.rate_limited", "Zu viele Versuche. Bitte warte eine Minute.", "Too many attempts. Please wait a minute."),
    ("social.error.bad_name", "Benutzername: 3–20 Zeichen, nur A–Z, 0–9, _ . -", "Username: 3–20 characters, only A–Z, 0–9, _ . -"),
    ("social.error.bad_email", "Das ist keine gültige E-Mail-Adresse.", "That's not a valid email address."),
    ("social.error.password_short", "Das Passwort ist zu kurz (mindestens 8 Zeichen).", "The password is too short (at least 8 characters)."),
    ("social.error.password_long", "Das Passwort ist zu lang.", "The password is too long."),
    ("social.error.name_taken", "Dieser Benutzername ist schon vergeben.", "This username is already taken."),
    ("social.error.email_taken", "Mit dieser E-Mail gibt es schon ein Konto – melde dich an oder setze das Passwort zurück.", "There's already an account with this email – sign in or reset your password."),
    ("social.error.mail_disabled", "Der Mailversand ist noch nicht eingerichtet. Bitte später noch einmal versuchen.", "Sending email isn't set up yet. Please try again later."),
    ("social.error.mail_failed", "Die E-Mail konnte nicht verschickt werden. Bitte später noch einmal versuchen.", "The email couldn't be sent. Please try again later."),
    ("social.error.unauthorized", "Bitte melde dich neu an.", "Please sign in again."),
    ("social.error.session_expired", "Deine Anmeldung ist abgelaufen. Bitte melde dich neu an.", "Your session has expired. Please sign in again."),
    ("social.error.password_reset", "Dein Passwort wurde geändert. Bitte melde dich mit dem neuen an.", "Your password was changed. Please sign in with the new one."),
    ("social.error.deleted", "Dein Konto ist gelöscht.", "Your account has been deleted."),
    ("social.error.logout", "Abgemeldet.", "Signed out."),
    ("social.error.user_not_found", "Diesen Benutzer gibt es nicht.", "This user doesn't exist."),
    ("social.error.already_friends", "Ihr seid schon befreundet.", "You're already friends."),
    ("social.error.already_requested", "Anfrage ist schon unterwegs.", "Request already sent."),
    ("social.error.no_request", "Diese Anfrage gibt es nicht mehr.", "This request no longer exists."),
    ("social.error.too_many", "Zu viele Freunde bzw. offene Anfragen.", "Too many friends or open requests."),
    ("social.error.not_friends", "Ihr seid (nicht mehr) befreundet.", "You're not friends (anymore)."),
    ("social.error.jam_not_found", "Dein Sync läuft nicht mehr – starte einen neuen und lade noch einmal ein.", "Your Sync isn't running anymore – start a new one and invite again."),
    ("social.error.bad_code", "Ungültiger Sync-Code.", "Invalid Sync code."),
    // Playtube Sync
    ("tab.jam", "Sync", "Sync"),
    ("jam.title", "Playtube Sync", "Playtube Sync"),
    ("jam.intro",
     "Hört gemeinsam wie bei Spotify Jam: jeder auf seinem eigenen Gerät, synchron, und alle wünschen sich Songs. Freunde ohne Playtube machen über den Link im Browser mit.",
     "Listen together like Spotify Jam: everyone on their own device, in sync, and everyone adds songs. Friends without Playtube join in the browser via the link."),
    ("jam.name", "Dein Name", "Your name"),
    ("jam.start", "Sync starten", "Start jam"),
    ("jam.join", "Beitreten", "Join"),
    ("jam.code_placeholder", "Code oder Einladungslink", "Code or invite link"),
    ("jam.invite", "Freunde einladen", "Invite friends"),
    ("jam.copy", "Link kopieren", "Copy link"),
    ("jam.copied", "Link kopiert.", "Link copied."),
    ("jam.all_control", "Alle dürfen steuern", "Everyone can control"),
    ("jam.end", "Sync beenden", "End jam"),
    ("jam.leave", "Sync verlassen", "Leave jam"),
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
     "Die Musik läuft im YouTube-Music-Tab. Dort kannst du weiter stöbern: Tippst du einen Song an, landet er wie bei Spotify im Sync. Mit Steuerrecht wirken Play/Pause und Weiter dort, im Tray, am Stream Dock und vom Handy für alle.",
     "The music plays in the YouTube Music tab. You can keep browsing there: tap a song and it joins the sync, like on Spotify. With control rights, play/pause and skip there, in the tray, on the Stream Dock and from your phone apply to everyone."),
    ("jam.kick", "Entfernen", "Remove"),
    ("jam.make_host", "Zum Host machen", "Make host"),
    ("jam.like", "Zu Lieblingssongs", "Add to liked songs"),
    ("jam.like_need_login", "Dafür musst du bei YouTube Music angemeldet sein.", "You need to be signed in to YouTube Music for this."),
    ("jam.like_failed", "Hat nicht geklappt – noch einmal versuchen.", "That did not work – try again."),
    ("jam.make_host_confirm", "Nochmal klicken, um {name} zum Host zu machen – du gibst damit die Host-Rechte ab.", "Click again to make {name} the host – you will give up your host rights."),
    ("jam.make_host_done", "{name} ist jetzt Host.", "{name} is now the host."),
    ("jam.error.offline", "Diese Person ist gerade nicht verbunden.", "This person is not connected right now."),
    ("jam.play_now", "Jetzt spielen", "Play now"),
    ("jam.remove", "Entfernen", "Remove"),
    ("jam.add_next", "Als Nächstes", "Play next"),
    ("jam.client.pc", "Playtube PC", "Playtube PC"),
    ("jam.client.android", "Android", "Android"),
    ("jam.client.web", "Browser", "Browser"),
    ("jam.status.creating", "Sync wird gestartet …", "Starting jam …"),
    ("jam.status.joining", "Verbinde …", "Connecting …"),
    ("jam.status.connected", "Verbunden.", "Connected."),
    ("jam.status.reconnecting", "Verbindung unterbrochen – verbinde neu …", "Connection lost – reconnecting …"),
    ("jam.status.bad_code", "Das ist kein gültiger Code (6 Zeichen).", "That is not a valid code (6 characters)."),
    ("jam.status.not_found", "Diesen Sync gibt es nicht (mehr).", "This jam does not exist (anymore)."),
    ("jam.status.failed", "Keine Verbindung zum Sync-Server.", "Can't reach the jam server."),
    ("jam.status.need_name", "Bitte gib zuerst deinen Namen ein.", "Please enter your name first."),
    ("jam.status.in_other", "Du bist schon in einem Sync – verlasse ihn zuerst.", "You are already in a jam – leave it first."),
    ("jam.ended.host_ended", "Der Host hat den Sync beendet.", "The host ended the jam."),
    ("jam.ended.host_left", "Der Sync ist zu Ende.", "The jam is over."),
    ("jam.ended.expired", "Der Sync ist abgelaufen.", "The jam has expired."),
    ("jam.ended.kicked", "Du wurdest aus dem Sync entfernt.", "You were removed from the jam."),
    ("jam.ended.left", "Du hast den Sync verlassen.", "You left the jam."),
    ("jam.ended.full", "Der Sync ist voll.", "The jam is full."),
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
    ("music.jam_add", "Sync", "Sync"),
    ("music.jam_queue", "Sync-Warteschlange", "Sync queue"),
    ("music.jam_played", "Gespielt", "Played"),
    ("music.jam_added_one", "Zum Sync hinzugefügt: {title}", "Added to the sync: {title}"),
    ("music.jam_added_many", "{count} Songs zum Sync hinzugefügt", "Added {count} songs to the sync"),
    ("music.jam_nothing", "Hier sind keine Songs für den Sync.", "There are no songs for the sync here."),
    ("jam.playlist_adding", "Playlist wird hinzugefügt …", "Adding playlist …"),
    ("jam.error.playlist_not_found", "Diese Playlist ist nicht öffentlich oder existiert nicht.", "This playlist is not public or does not exist."),
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
