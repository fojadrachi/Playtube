"""Sprache der Oberflaeche (Deutsch / Englisch (US)).

Alle sichtbaren Texte stehen in STRINGS und werden ueber tr("schluessel", platzhalter=...)
geholt. Die Sprache kommt aus config.json ("language": "de" / "en"; leer = automatisch nach
der Windows-Sprache) und laesst sich im Einstellungen-Tab umschalten - MainWindow und
SettingsTab setzen ihre Texte dann per retranslate() neu, ohne Neustart.

Entwickler-Ausgaben (print, Debug-Log, Fernsteuerungs-Protokoll) bleiben bewusst deutsch.
"""
from __future__ import annotations

from PySide6.QtCore import QLibraryInfo, QLocale, QTranslator
from PySide6.QtWidgets import QApplication

# Code -> Anzeigename im Dropdown (jeweils in der eigenen Sprache).
LANGUAGES: dict[str, str] = {
    "de": "Deutsch",
    "en": "English (US)",
}
DEFAULT_LANGUAGE = "de"

# Sprache fuer die WebView2-Runtime (Kontextmenues, Accept-Language der Webseite).
_WEBVIEW_LANGUAGE = {"de": "de-DE", "en": "en-US"}

STRINGS: dict[str, dict[str, str]] = {
    # ------------------------------------------------------------ Hauptfenster / Tray
    "tab.settings": {"de": "Einstellungen", "en": "Settings"},
    "toolbar.navigation": {"de": "Navigation", "en": "Navigation"},
    "url.placeholder": {"de": "Suche oder URL eingeben …", "en": "Search or enter URL …"},
    "tray.show": {"de": "Anzeigen", "en": "Show"},
    "tray.play_pause": {"de": "Wiedergabe umschalten", "en": "Play/Pause"},
    "tray.next": {"de": "Nächster Titel", "en": "Next track"},
    "tray.previous": {"de": "Vorheriger Titel", "en": "Previous track"},
    "tray.quit": {"de": "Beenden", "en": "Quit"},
    "tray.background": {
        "de": "{app} läuft im Hintergrund weiter. Rechtsklick auf das Tray-Icon zum Beenden.",
        "en": "{app} keeps running in the background. Right-click the tray icon to quit.",
    },
    # ------------------------------------------------------------ Einstellungen-Tab
    "settings.title": {"de": "{app}-Einstellungen", "en": "{app} Settings"},
    "settings.version": {"de": "Version {version}", "en": "Version {version}"},
    "settings.save": {"de": "Speichern", "en": "Save"},
    "settings.saved_title": {"de": "Gespeichert", "en": "Saved"},
    "settings.saved_text": {
        "de": "Einstellungen gespeichert und übernommen (Discord-Verbindung wurde mit den "
              "neuen Werten neu gestartet).",
        "en": "Settings saved and applied (the Discord connection was restarted with the "
              "new values).",
    },
    "settings.saved_language": {
        "de": "Die Sprache von {app} ist sofort umgestellt. Die Menüs der Webseiten "
              "(z. B. Rechtsklick) folgen nach einem Neustart von {app}.",
        "en": "{app} switched its language right away. Website menus (e.g. right-click) "
              "follow after restarting {app}.",
    },
    "section.discord": {"de": "Discord Rich Presence", "en": "Discord Rich Presence"},
    "section.updates": {"de": "Automatische Updates", "en": "Automatic updates"},
    "section.audio": {"de": "Audioausgabe", "en": "Audio output"},
    "section.general": {"de": "Allgemein", "en": "General"},
    # Feldnamen ohne Doppelpunkt - werden fuer Formular-Beschriftung UND die
    # Discord-Anzeige ("Bearbeitet: Abschnitt › Feld") benutzt.
    "field.enabled": {"de": "Aktiviert", "en": "Enabled"},
    "field.client_id": {"de": "Client-ID", "en": "Client ID"},
    "field.update_interval": {"de": "Update-Intervall", "en": "Update interval"},
    "field.idle_status": {"de": "Status im Leerlauf", "en": "Idle status"},
    "field.check_interval": {"de": "Prüfintervall", "en": "Check interval"},
    "field.youtube": {"de": "YouTube", "en": "YouTube"},
    "field.music": {"de": "YouTube Musik", "en": "YouTube Music"},
    "field.start_tab": {"de": "Beim Start öffnen", "en": "Open on startup"},
    "field.language": {"de": "Sprache", "en": "Language"},
    "discord.show_idle": {
        "de": "Status anzeigen, wenn gerade nichts läuft",
        "en": "Show status when nothing is playing",
    },
    "discord.client_id_placeholder": {
        "de": "Discord Application Client-ID",
        "en": "Discord application client ID",
    },
    "updates.check_now": {"de": "Jetzt nach Updates suchen", "en": "Check for updates now"},
    "updates.install": {"de": "Update installieren", "en": "Install update"},
    "updates.checking": {"de": "Suche nach Updates …", "en": "Checking for updates …"},
    "updates.available": {
        "de": "Neue Version verfügbar: {version}",
        "en": "New version available: {version}",
    },
    "audio.hint": {
        "de": "Legt den Ton von YouTube und YouTube Musik auf verschiedene Ausgabegeräte, "
              "z. B. auf getrennte Sonar-Kanäle. Windows zeigt beide unter „{app}“ - getrennt "
              "wird über das Gerät. Damit die Seite das Gerät finden kann, erlaubt {app} ihr "
              "dafür den Zugriff auf die Audiogeräte-Namen (es wird nichts aufgenommen); die "
              "Freigabe gilt nur, solange hier ein eigenes Gerät gewählt ist.",
        "en": "Sends the sound of YouTube and YouTube Music to different output devices, "
              "e.g. separate Sonar channels. Windows lists both under “{app}” - they "
              "are separated by device. So the page can find the device, {app} lets it read "
              "the names of your audio devices (nothing is recorded); this is only allowed "
              "while a specific device is selected here.",
    },
    "audio.system_default": {"de": "Systemstandard", "en": "System default"},
    "audio.unavailable": {"de": "{name} (nicht verfügbar)", "en": "{name} (not available)"},
    "activity.editing": {"de": "Bearbeitet: {section} › {field}", "en": "Editing: {section} › {field}"},
    "activity.checking_updates": {"de": "Sucht nach Updates", "en": "Checking for updates"},
    "activity.installing_update": {"de": "Installiert ein Update", "en": "Installing an update"},
    "activity.saving": {"de": "Speichert die Einstellungen", "en": "Saving the settings"},
    # ------------------------------------------------------------ Update (MainWindow)
    "update.up_to_date": {
        "de": "Du hast bereits die neueste Version (v{version}).",
        "en": "You already have the latest version (v{version}).",
    },
    "update.check_failed": {
        "de": "Update-Prüfung fehlgeschlagen (keine Internetverbindung oder Update-Server "
              "nicht erreichbar).",
        "en": "Update check failed (no internet connection or update server unreachable).",
    },
    "update.dialog_title": {"de": "{app}-Update verfügbar", "en": "{app} update available"},
    "update.dialog_text": {
        "de": "Eine neue Version von {app} ist verfügbar: {version}\n(aktuell installiert: {current})",
        "en": "A new version of {app} is available: {version}\n(currently installed: {current})",
    },
    "update.install_now": {"de": "Jetzt installieren?", "en": "Install now?"},
    "update.installing_local": {"de": "Installiere lokale Patch-Datei …", "en": "Installing local patch file …"},
    "update.downloading": {"de": "Lade Update herunter …", "en": "Downloading update …"},
    "update.tray_installing": {"de": "{app} – Update wird installiert …", "en": "{app} – installing update …"},
    "update.done_restarting": {
        "de": "Installation abgeschlossen. {app} wird neu gestartet …",
        "en": "Installation complete. Restarting {app} …",
    },
    "update.tray_restarting": {"de": "{app} – wird neu gestartet …", "en": "{app} – restarting …"},
    "update.dev_done": {
        "de": "Update installiert. Die App wird jetzt neu gestartet.",
        "en": "Update installed. The app will now restart.",
    },
    "update.failed_status": {"de": "Update fehlgeschlagen: {error}", "en": "Update failed: {error}"},
    "update.failed_title": {"de": "{app}-Update fehlgeschlagen", "en": "{app} update failed"},
    # ------------------------------------------------------------ Updater (Thread/Skripte)
    "updater.patch_not_found": {"de": "Patch-Datei nicht gefunden: {path}", "en": "Patch file not found: {path}"},
    "updater.using_local": {"de": "Verwende lokale Patch-Datei {name} …", "en": "Using local patch file {name} …"},
    "updater.no_package": {
        "de": "Kein passendes Release-Paket für dieses Betriebssystem gefunden.",
        "en": "No matching release package found for this operating system.",
    },
    "updater.starting_installer": {"de": "Starte Installer …", "en": "Starting installer …"},
    "updater.extracting": {"de": "Entpacke Update …", "en": "Extracting update …"},
    "updater.preparing": {"de": "Bereite Installation vor …", "en": "Preparing installation …"},
    "updater.patch_missing_binary": {
        "de": "Patch-Paket enthielt kein '{binary}'. Bitte erneut versuchen.",
        "en": "Patch package did not contain '{binary}'. Please try again.",
    },
    "updater.window_title": {"de": "{app}-Update", "en": "{app} update"},
    "updater.updating": {"de": "{app} wird aktualisiert – bitte warten …", "en": "Updating {app} – please wait …"},
    "updater.copying_file": {"de": "Kopiere aktualisierte Datei …", "en": "Copying updated file …"},
    "updater.copying_files": {
        "de": "Kopiere Programmdateien … (kann etwas dauern)",
        "en": "Copying program files … (this may take a while)",
    },
    "updater.done_restarting": {"de": "Fertig – {app} wird neu gestartet …", "en": "Done – restarting {app} …"},
    "updater.dev_no_git": {
        "de": "Kein Git-Repository gefunden - im Entwicklungsmodus bitte manuell 'git pull' "
              "im Projektordner ausführen.",
        "en": "No Git repository found - in development mode, run 'git pull' manually in "
              "the project folder.",
    },
    "updater.dev_pulling": {"de": "Lade Änderungen per git pull …", "en": "Pulling changes via git pull …"},
    "updater.dev_pull_failed": {"de": "git pull fehlgeschlagen:\n{error}", "en": "git pull failed:\n{error}"},
    "updater.dev_installing_deps": {"de": "Installiere Abhängigkeiten …", "en": "Installing dependencies …"},
    "updater.dev_pip_failed": {"de": "pip install fehlgeschlagen:\n{error}", "en": "pip install failed:\n{error}"},
    # ------------------------------------------------------------ Discord Rich Presence
    "rpc.unknown_title": {"de": "Unbekannter Titel", "en": "Unknown title"},
    "rpc.playing": {"de": "Spielt", "en": "Playing"},
    "rpc.paused": {"de": "Pausiert", "en": "Paused"},
    "rpc.open_music": {"de": "In YouTube Music öffnen", "en": "Open in YouTube Music"},
    "rpc.watch_youtube": {"de": "Auf YouTube ansehen", "en": "Watch on YouTube"},
    "rpc.idle_details": {"de": "Bei {app}", "en": "In {app}"},
    "rpc.idle_state": {"de": "Stöbert gerade", "en": "Browsing"},
    "rpc.settings_details": {"de": "In den Einstellungen", "en": "In the settings"},
    "rpc.settings_default": {"de": "Schaut sich die Einstellungen an", "en": "Looking at the settings"},
}

_current = DEFAULT_LANGUAGE
_qt_translator: QTranslator | None = None


def system_language() -> str:
    """Windows-/Systemsprache auf eine unterstuetzte Sprache abgebildet."""
    return "de" if QLocale.system().language() == QLocale.Language.German else "en"


def resolve_language(code: str | None) -> str:
    """Gespeicherter Wert -> gueltiger Sprachcode (leer/unbekannt = Systemsprache)."""
    return code if code in LANGUAGES else system_language()


def get_language() -> str:
    return _current


def set_language(code: str | None) -> str:
    """Setzt die Oberflaechensprache und laedt passende Qt-Standardtexte (z.B. "Ja"/"Nein"
    in Dialogen). Liefert den tatsaechlich gesetzten Code zurueck."""
    global _current
    _current = resolve_language(code)
    _install_qt_translator(_current)
    return _current


def webview_language() -> str:
    return _WEBVIEW_LANGUAGE[_current]


def tr(key: str, **kwargs) -> str:
    entry = STRINGS.get(key)
    if entry is None:
        return key
    text = entry.get(_current) or entry[DEFAULT_LANGUAGE]
    return text.format(**kwargs) if kwargs else text


def _install_qt_translator(code: str) -> None:
    """Qt bringt eigene Uebersetzungen fuer Standard-Buttons mit (qtbase_de.qm). Fehlen die
    Dateien (z.B. im gepackten Build), bleiben die englischen Qt-Texte - kein Fehler."""
    global _qt_translator
    app = QApplication.instance()
    if app is None:
        return
    if _qt_translator is not None:
        app.removeTranslator(_qt_translator)
        _qt_translator = None
    if code == "en":
        return
    translator = QTranslator(app)
    path = QLibraryInfo.path(QLibraryInfo.LibraryPath.TranslationsPath)
    if translator.load(f"qtbase_{code}", path):
        app.installTranslator(translator)
        _qt_translator = translator
