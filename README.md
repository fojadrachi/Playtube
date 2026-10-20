**Sprache / Language:** [Deutsch](#deutsch) · [English](#english)

<a id="deutsch"></a>

# Playtube (Deutsch)

Eigenständige Desktop-App für YouTube & YouTube Music unter Windows (kein Browser-Fenster, keine
Erweiterung). Seit Version 1.0.0 rendert Playtube mit der **Microsoft-Edge-Engine (WebView2)** –
damit laufen auch selbst hochgeladene Songs in YouTube Music (AAC/H.264).

- **Zwei Tabs** – YouTube und YouTube Music laufen parallel, Musik spielt im Hintergrund weiter,
  wenn du zu Videos wechselst.
- **Login** – eigenes, dauerhaftes Profil (`%APPDATA%\Playtube`), einmal bei Google anmelden
  reicht für beide Dienste.
- **Discord Rich Presence** – zeigt Titel, Kanal/Interpret, Fortschrittsbalken und einen
  Link-Button in deinem Discord-Profil. Ist der **Einstellungen-Tab** offen, steht dort
  „In den Einstellungen“ und welches Feld du gerade bearbeitest – nur der Feldname, nie der
  Inhalt.
- **Audioausgabe pro Tab** – YouTube und YouTube Music auf verschiedene Ausgabegeräte legen
  (z. B. getrennte Sonar-Kanäle).
- **Sprache** – Deutsch oder Englisch (US), im Einstellungen-Tab umschaltbar.
- **Fernsteuerung** – lokal per Named Pipe, z. B. über das Stream-Dock-Plugin.
- **System-Tray** – Fenster schließen minimiert nur (Musik läuft weiter), Rechtsklick aufs
  Tray-Icon zum Beenden, Play/Pause/Skip direkt aus dem Menü.
- **Automatische Updates** über die [GitHub Releases](https://github.com/fojadrachi/Playtube/releases).

## Installation (Windows)

Lade auf der [Releases-Seite](https://github.com/fojadrachi/Playtube/releases)
`Playtube-Setup-vX.Y.Z.exe` herunter und starte sie. Voraussetzung: Windows 10/11 mit
WebView2-Runtime (unter Windows 11 vorinstalliert). Der Installer

- installiert Playtube **pro Benutzer** nach `%LOCALAPPDATA%\Programs\Playtube` (keine
  Admin-Rechte nötig) und legt einen Startmenü-Eintrag an (optional Desktop-Verknüpfung),
- ersetzt bei jeder späteren Ausführung die vorhandene Installation **an Ort und Stelle**
  (feste Installer-ID) – auch eine ältere Playtube (Qt-Version bis 4.5.1),
- lässt Einstellungen (`%APPDATA%\Playtube`) unangetastet; beim Deinstallieren über
  „Apps & Features“ wird gefragt, ob sie mitgelöscht werden sollen.

Das ZIP (`Playtube-vX.Y.Z-win64.zip`) ist die portable Variante ohne Installation.

Wer bisher **Playtube Edge** benutzt hat: Playtube installieren und Playtube Edge danach über
„Apps & Features“ deinstallieren. Einmal neu bei Google anmelden.

## Schnellstart (Entwicklung)

```powershell
python -m venv .venv
.venv\Scripts\pip install -r requirements.txt
.venv\Scripts\python main.py
```

Der Entwicklungsmodus nutzt einen eigenen Datenordner (`%APPDATA%\PlaytubeDev`), damit er sich
nie mit einer installierten Playtube überschneidet. Die WebView2-DLLs liegen in
`vendor/webview2` (Microsoft, NuGet `Microsoft.Web.WebView2`).

Tests: `.venv\Scripts\python -m unittest discover -s tests -v`

## Sprache

Im **Einstellungen-Tab** wählst du unter „Allgemein → Sprache“ zwischen **Deutsch** und
**English (US)**. Nach „Speichern“ stellt die App sofort um (Tabs, Tray-Menü, Einstellungen,
Update-Meldungen, Discord-Texte). In `config.json` heißt die Einstellung `language`: `"de"`,
`"en"` oder leer (= automatisch nach der Windows-Sprache). Alle Texte stehen in
[playtube/i18n.py](playtube/i18n.py).

## Discord Rich Presence einrichten

Die Client-ID ist bereits in `config.json` eingetragen (`discord.client_id`). Für eine eigene
Anwendung:

1. Auf https://discord.com/developers/applications eine Application anlegen.
2. Die **Application ID** in `config.json` unter `discord.client_id` eintragen.
3. Optional unter *Rich Presence → Art Assets* zwei Bilder mit den Schlüsseln `youtube_logo`
   und `music_logo` hochladen.
4. Discord muss auf demselben Rechner laufen.

## Audioausgabe pro Tab

Im **Einstellungen-Tab** legst du unter „Audioausgabe“ getrennt fest, über welches Gerät
**YouTube** und **YouTube Musik** laufen (Standard: Systemstandard). Die Auswahl gilt nach
„Speichern“ sofort. Damit die Seite das Gerät per Namen findet, erteilt Playtube YouTube/YouTube
Musik dafür die Berechtigung für Audiogeräte – es wird **nichts aufgenommen**. Technik:
[playtube/audio_routing.py](playtube/audio_routing.py) (`HTMLMediaElement.setSinkId`).

## Fernsteuerung (Stream Dock)

Playtube lauscht auf der Named Pipe `\\.\pipe\Playtube.Remote.Playtube` (Entwicklungsmodus:
`...PlaytubeDev`), z. B. für das Stream-Dock-Plugin `com.fojadrachi.playtube.sdPlugin`. Kein
Netzwerk-Port, verbinden darf nur der angemeldete Benutzer.

- Protokoll: eine JSON-Nachricht pro Zeile, z. B. `{"id":1,"cmd":"play_pause","target":"auto"}`.
- Befehle: `status`, `show`, `switch_tab`, `play_pause`, `play`, `pause`, `next`, `previous`,
  `seek`, `volume_change`, `set_volume`, `mute_toggle`, `like`, `dislike`, `shuffle`, `repeat`,
  `list_playlists`, `play_playlist` (feste Whitelist).
- Abschalten: in `config.json` `"remote_control": {"enabled": false}`.

## Playtube.exe und Setup bauen

```powershell
.venv\Scripts\pip install pyinstaller
powershell -ExecutionPolicy Bypass -File packaging\build.ps1
```

Ergebnis: `dist\Playtube\Playtube.exe`. Mit [Inno Setup 6](https://jrsoftware.org/isinfo.php)
(`winget install --id JRSoftware.InnoSetup -e`) entsteht zusätzlich
`dist\installer\Playtube-Setup-vX.Y.Z.exe` (Skript: [packaging/playtube.iss](packaging/playtube.iss),
mit `-SkipInstaller` überspringbar).

## Automatische Updates

Playtube prüft beim Start und danach alle `updates.check_interval_hours` Stunden (Standard 6)
die [GitHub Releases](https://github.com/fojadrachi/Playtube/releases). Berücksichtigt werden nur
Releases mit dem Titel „Playtube vX.Y.Z“ – so kann der Updater nie zur alten Qt-Version
zurückspringen. Gibt es eine neuere Version, lädt Playtube nach Bestätigung den
**Setup-Installer** und führt ihn still aus; die App startet danach neu. Im Entwicklungsmodus
werden stattdessen `git pull` und `pip install -r requirements.txt` ausgeführt.

Abschalten: im Einstellungen-Tab oder in `config.json` unter `updates.enabled`.

## Eine neue Version veröffentlichen

```powershell
powershell -ExecutionPolicy Bypass -File packaging\release.ps1 -Version 1.1.0
```

Das Skript setzt die Versionsnummer, committet, erstellt den Tag `v1.1.0` und pusht zu `origin`
(https://github.com/fojadrachi/Playtube). Der Tag löst
[.github/workflows/release.yml](.github/workflows/release.yml) aus: GitHub Actions baut Setup und
ZIP und veröffentlicht beides als Release „Playtube v1.1.0“ mit den Notizen aus
[packaging/release_notes.md](packaging/release_notes.md).

## Hinweise

- **Fehlende Icons auf YouTube/YouTube Music:** Fast immer laufen zwei Playtube-Instanzen auf
  demselben Profil. Alle über das Tray-Icon (Rechtsklick → Beenden) schließen und nur eine
  starten.
- Icon/Branding liegen unter `assets/` (erzeugt mit `tools/generate_icon.py`).
- Playtube ist ein privates Projekt und steht in keiner Verbindung zu Google, YouTube oder
  Discord.

---

<a id="english"></a>

# Playtube (English)

A standalone desktop app for YouTube & YouTube Music on Windows (no browser window, no
extension). Since version 1.0.0 Playtube renders with the **Microsoft Edge engine (WebView2)** –
so your own uploads in YouTube Music play too (AAC/H.264).

- **Two tabs** – YouTube and YouTube Music run side by side; music keeps playing when you switch
  to videos.
- **Login** – its own persistent profile (`%APPDATA%\Playtube`); sign in to Google once for both
  services.
- **Discord Rich Presence** – shows title, channel/artist, a progress bar and a link button in
  your Discord profile. While the **Settings tab** is open it shows “In settings” and which field
  you are editing – only the field name, never its content.
- **Audio output per tab** – send YouTube and YouTube Music to different output devices (e.g.
  separate Sonar channels).
- **Language** – German or English (US), switchable in the Settings tab.
- **Remote control** – local named pipe, e.g. for the Stream Dock plugin.
- **System tray** – closing the window only minimizes it (music keeps playing); right-click the
  tray icon to quit; play/pause/skip from the menu.
- **Automatic updates** via [GitHub Releases](https://github.com/fojadrachi/Playtube/releases).

## Installation (Windows)

Download `Playtube-Setup-vX.Y.Z.exe` from the
[releases page](https://github.com/fojadrachi/Playtube/releases) and run it. Requires Windows
10/11 with the WebView2 runtime (preinstalled on Windows 11). The installer

- installs Playtube **per user** to `%LOCALAPPDATA%\Programs\Playtube` (no admin rights) and
  adds a Start menu entry (desktop shortcut optional),
- replaces an existing installation **in place** on every later run (fixed installer ID) – including
  an older Playtube (Qt version up to 4.5.1),
- keeps your settings (`%APPDATA%\Playtube`); uninstalling via “Apps & features” asks whether to
  delete them too.

The ZIP (`Playtube-vX.Y.Z-win64.zip`) is the portable version without installation.

Coming from **Playtube Edge**: install Playtube, then uninstall Playtube Edge via “Apps &
features”. Sign in to Google once more.

## Quick start (development)

```powershell
python -m venv .venv
.venv\Scripts\pip install -r requirements.txt
.venv\Scripts\python main.py
```

Development mode uses its own data folder (`%APPDATA%\PlaytubeDev`) so it never clashes with an
installed Playtube. The WebView2 DLLs live in `vendor/webview2` (Microsoft, NuGet
`Microsoft.Web.WebView2`).

Tests: `.venv\Scripts\python -m unittest discover -s tests -v`

## Language

In the **Settings tab**, pick **Deutsch** or **English (US)** under “General → Language”. After
“Save” the app switches immediately (tabs, tray menu, settings, update messages, Discord texts).
In `config.json` the setting is `language`: `"de"`, `"en"` or empty (= follow the Windows
language). All strings live in [playtube/i18n.py](playtube/i18n.py).

## Setting up Discord Rich Presence

The client ID is already set in `config.json` (`discord.client_id`). To use your own application:

1. Create an application at https://discord.com/developers/applications.
2. Put its **Application ID** into `config.json` under `discord.client_id`.
3. Optionally upload two images under *Rich Presence → Art Assets* with the keys `youtube_logo`
   and `music_logo`.
4. Discord must be running on the same computer.

## Audio output per tab

In the **Settings tab**, “Audio output” sets the device for **YouTube** and **YouTube Music**
separately (default: system default). The choice applies right after “Save”. So the page can find
the device by name, Playtube grants YouTube/YouTube Music access to audio devices – **nothing is
recorded**. Implementation: [playtube/audio_routing.py](playtube/audio_routing.py)
(`HTMLMediaElement.setSinkId`).

## Remote control (Stream Dock)

Playtube listens on the named pipe `\\.\pipe\Playtube.Remote.Playtube` (development mode:
`...PlaytubeDev`), e.g. for the Stream Dock plugin `com.fojadrachi.playtube.sdPlugin`. No network
port; only the signed-in user can connect.

- Protocol: one JSON message per line, e.g. `{"id":1,"cmd":"play_pause","target":"auto"}`.
- Commands: `status`, `show`, `switch_tab`, `play_pause`, `play`, `pause`, `next`, `previous`,
  `seek`, `volume_change`, `set_volume`, `mute_toggle`, `like`, `dislike`, `shuffle`, `repeat`,
  `list_playlists`, `play_playlist` (fixed whitelist).
- Disable: `"remote_control": {"enabled": false}` in `config.json`.

## Building Playtube.exe and the setup

```powershell
.venv\Scripts\pip install pyinstaller
powershell -ExecutionPolicy Bypass -File packaging\build.ps1
```

Result: `dist\Playtube\Playtube.exe`. With [Inno Setup 6](https://jrsoftware.org/isinfo.php)
(`winget install --id JRSoftware.InnoSetup -e`) it also builds
`dist\installer\Playtube-Setup-vX.Y.Z.exe` (script: [packaging/playtube.iss](packaging/playtube.iss),
skip with `-SkipInstaller`).

## Automatic updates

Playtube checks the [GitHub Releases](https://github.com/fojadrachi/Playtube/releases) at startup
and every `updates.check_interval_hours` hours (default 6). Only releases titled
“Playtube vX.Y.Z” count – so the updater can never jump back to the old Qt version. When a newer
version exists, Playtube downloads the **setup installer** after you confirm and runs it silently;
the app restarts afterwards. In development mode it runs `git pull` and
`pip install -r requirements.txt` instead.

Disable it in the Settings tab or via `updates.enabled` in `config.json`.

## Publishing a new version

```powershell
powershell -ExecutionPolicy Bypass -File packaging\release.ps1 -Version 1.1.0
```

The script sets the version, commits, creates tag `v1.1.0` and pushes to `origin`
(https://github.com/fojadrachi/Playtube). The tag triggers
[.github/workflows/release.yml](.github/workflows/release.yml): GitHub Actions builds the setup
and ZIP and publishes them as release “Playtube v1.1.0” with the notes from
[packaging/release_notes.md](packaging/release_notes.md).

## Notes

- **Missing icons on YouTube/YouTube Music:** almost always two Playtube instances are running on
  the same profile. Quit all via the tray icon (right-click → Quit) and start only one.
- Icon/branding live in `assets/` (generated with `tools/generate_icon.py`).
- Playtube is a private project and not affiliated with Google, YouTube or Discord.
