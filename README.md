**Sprache / Language:** [Deutsch](#deutsch) · [English](#english)

<a id="deutsch"></a>

# Playtube (Deutsch)

Eigenständige Desktop-App für YouTube & YouTube Music unter Windows. Seit Version 2.0.0 in
**Rust** (Oberfläche mit [Slint](https://slint.dev)); die Seiten zeigt die **Microsoft-Edge-Engine
(WebView2)** – damit laufen auch selbst hochgeladene Songs in YouTube Music.

- **Zwei Tabs** – YouTube und YouTube Music parallel, Musik spielt im Hintergrund weiter.
- **Playtube-Ansicht** – YouTube Music mit eigener Oberfläche im Spotify-Stil (Startseite, Suche, Bibliothek, Player-Leiste, Warteschlange); Premium bleibt erhalten.
- **Login** – eigenes, dauerhaftes Profil (`%APPDATA%\Playtube`), einmal bei Google anmelden.
- **Discord Rich Presence** – Titel, Kanal/Interpret, Fortschritt und Link-Button im Discord-Profil.
- **Audioausgabe pro Tab** – YouTube und YouTube Music auf verschiedene Geräte (z. B. Sonar-Kanäle).
- **Steuerung vom Handy** – mit Playtube für Android im selben WLAN, wie Spotify Connect.
- **Playtube Jam** – gemeinsam hören wie bei Spotify Jam: Freunde per Link/QR einladen, jeder hört synchron auf dem eigenen Gerät.
- **Fernsteuerung** – lokal per Named Pipe, z. B. über das Stream-Dock-Plugin.
- **System-Tray** – Schließen versteckt nur das Fenster, Play/Pause/Skip und Beenden im Tray-Menü.
- **Sprache** – Deutsch oder Englisch (US), im Einstellungen-Tab umschaltbar.
- **Automatische Updates** über die [GitHub Releases](https://github.com/fojadrachi/Playtube/releases).

## Installation

`Playtube-Setup-<Version>.exe` von der [Releases-Seite](https://github.com/fojadrachi/Playtube/releases)
laden und starten. Voraussetzung: Windows 10/11 mit WebView2-Runtime (unter Windows 11 vorhanden).
Das Setup installiert pro Benutzer nach `%LOCALAPPDATA%\Programs\Playtube` (keine Adminrechte) und
ersetzt jede ältere Playtube – auch die früheren Python-Versionen. Einstellungen und Login bleiben
erhalten.

## Updates

Wie beim NNN Launcher: Playtube fragt beim Start (nach 5 s) und danach alle
`updates.check_interval_hours` Stunden `releases/latest` von GitHub ab. Gibt es eine neuere Version,
erscheint im Einstellungen-Tab „Update installieren“. Playtube lädt dann
`Playtube-Setup-<Version>.exe`, prüft die SHA-256-Prüfsumme, die GitHub mitliefert, startet das
Setup still (`/SILENT`) und beendet sich; das Setup startet Playtube danach neu.

## Steuerung vom Handy

1. Im Einstellungen-Tab „Steuerung vom Handy im selben WLAN erlauben“ einschalten und speichern.
   Windows fragt einmal nach der Firewall – für private Netzwerke erlauben.
2. „Handy koppeln“: Playtube zeigt einen 6-stelligen Code (2 Minuten gültig).
3. In der Android-App auf den runden Knopf am rechten Rand tippen, den PC wählen, Code eingeben.

Technik ([src/lan.rs](src/lan.rs)): TCP-Port 47811, Suche per UDP-Broadcast an Port 47812, dasselbe
Zeilenprotokoll wie die Stream-Dock-Steuerung, davor `pair`/`hello` mit Token. Standardmäßig aus;
nur Geräte aus privaten Netzen und nur nach Kopplung; gespeichert wird nur ein SHA-256 des Tokens.

## Playtube-Ansicht (YouTube Music im Spotify-Stil)

Im Tab YouTube Music zeigt Playtube standardmäßig eine eigene Oberfläche: Seitenleiste (Startseite,
Suchen, Bibliothek mit Lieblingssongs und Playlists), Regale mit Karten, Album-/Playlist-Seiten mit
Songliste, Player-Leiste und Warteschlange. Die Daten kommen über dieselbe Schnittstelle, die
music.youtube.com selbst nutzt (InnerTube, [src/ytm.rs](src/ytm.rs)), mit dem Login des Tabs
(Cookies der WebView, `SAPISIDHASH`). Abgespielt wird weiterhin im echten YouTube-Music-Player
– er liegt dabei außerhalb des sichtbaren Bereichs –, deshalb bleiben Premium, eigene Uploads und
Verlauf erhalten. Oberfläche: [ui/music.slint](ui/music.slint), Anbindung: [src/music.rs](src/music.rs).
Umschalten über „Original YouTube Music“ bzw. „Playtube-Ansicht“ (gespeichert als `music_view` in der
`config.json`). Angemeldet wird man in der Original-Ansicht.

## Playtube Jam

Tab **Jam**: Namen eingeben, **Jam starten** oder einen Code/Einladungslink eingeben und
**Beitreten**. Einladen per Link (kopieren) oder QR-Code; Freunde machen mit Playtube am PC,
Playtube für Android oder im Browser mit. Gespielt wird im YouTube-Music-Tab, den Playtube mit dem
Jam synchron hält (Position, Pause, Titelwechsel). Host bzw. alle mit Steuerrecht steuern auch über
den Tab selbst, das Tray, das Stream Dock und das Handy. Songs kommen per Link/Video-ID, als Video
aus dem YouTube-Tab oder – falls am Server freigeschaltet – über die Suche. Einladungslinks
`playtube://jam/<CODE>` öffnen Playtube direkt (das Setup registriert das Protokoll).

Technik: [src/jam.rs](src/jam.rs) (Protokoll, Sync-Logik mit Tests, WebSocket über
tokio-tungstenite/rustls) und [src/jam_ui.rs](src/jam_ui.rs); der Dienst selbst liegt im eigenen
Projekt `playtube-jam` (Cloudflare Worker). Server änderbar über `"jam": {"server": "…"}` in der
`config.json` (Standard `https://jam.fojadrachi.de`).

## Fernsteuerung (Stream Dock)

Named Pipe `\\.\pipe\Playtube.Remote.Playtube`, eine JSON-Nachricht pro Zeile, z. B.
`{"id":1,"cmd":"play_pause","target":"auto"}`. Befehle: `status`, `show`, `switch_tab`, `play_pause`,
`play`, `pause`, `next`, `previous`, `seek`, `volume_change`, `set_volume`, `mute_toggle`, `like`,
`dislike`, `shuffle`, `repeat`, `list_playlists`, `play_playlist`, `queue` (Warteschlange als Liste), `play_queue_item` (Titel per Index 0–199 abspielen) (feste Whitelist, siehe
[src/remote.rs](src/remote.rs)). Abschalten: `"remote_control": {"enabled": false}` in `config.json`.

## Entwicklung

```powershell
cargo run                       # Debug-Start
cargo test                      # Unit-Tests (Protokoll, Kopplung, Pipe, TCP, Updater, Skripte)
.\build-release.ps1             # dist\Playtube-Setup-<v>.exe (braucht Inno Setup 6)
```

Mit `PLAYTUBE_DATA_DIR=<Ordner>` nutzt Playtube einen eigenen Datenordner (getrennt von der
installierten App, auch eigene Pipe-Namen) – zum Testen. Diagnose-Log: `<Datenordner>\logs\playtube.log`.

| Datei | Aufgabe |
|---|---|
| `src/main.rs` | Fenster, Tabs, Tray, Einstellungen, Verbindung aller Teile |
| `src/browser.rs` | WebView2-Tab (Profil, Vollbild, Audio-Ausgabe, Gerätenamen-Freigabe) |
| `src/scripts.rs`, `src/js/` | JavaScript für Wiedergabestand, Steuerung, Playlists, Audio |
| `src/remote.rs` | Fernsteuerungsprotokoll + Named Pipe |
| `src/lan.rs` | Steuerung vom Handy (Kopplung, TCP, UDP-Suche) |
| `src/discord.rs` | Discord Rich Presence |
| `src/updater.rs` | Updates über GitHub-Releases |
| `src/config.rs`, `src/i18n.rs` | `config.json`, Texte Deutsch/Englisch |
| `ui/app.slint` | Oberfläche |
| `installer/Playtube.iss` | Setup (feste AppId) |

## Eine neue Version veröffentlichen

Version in `Cargo.toml` erhöhen, `release_notes.md` anpassen, committen, Tag `vX.Y.Z` pushen.
[.github/workflows/release.yml](.github/workflows/release.yml) testet, baut das Setup und
veröffentlicht es als Release „Playtube vX.Y.Z“.

Playtube ist ein privates Projekt und steht in keiner Verbindung zu Google, YouTube oder Discord.

---

<a id="english"></a>

# Playtube (English)

A standalone desktop app for YouTube & YouTube Music on Windows. Since version 2.0.0 written in
**Rust** (UI with [Slint](https://slint.dev)); pages are rendered by the **Microsoft Edge engine
(WebView2)** – so your own uploads in YouTube Music play too.

- **Two tabs** – YouTube and YouTube Music side by side, music keeps playing in the background.
- **Login** – its own persistent profile (`%APPDATA%\Playtube`), sign in to Google once.
- **Discord Rich Presence** – title, channel/artist, progress and a link button in your profile.
- **Audio output per tab** – send YouTube and YouTube Music to different devices.
- **Control from your phone** – with Playtube for Android on the same Wi-Fi, like Spotify Connect.
- **Playtube view** – YouTube Music with its own Spotify-like interface (home, search, library, player bar, queue); Premium is kept.
- **Playtube Jam** – listen together like Spotify Jam: invite friends via link/QR, everyone listens in sync on their own device.
- **Remote control** – local named pipe, e.g. for the Stream Dock plugin.
- **System tray** – closing only hides the window; play/pause/skip and quit in the tray menu.
- **Language** – German or English (US), switchable in the Settings tab.
- **Automatic updates** via [GitHub Releases](https://github.com/fojadrachi/Playtube/releases).

## Installation

Download and run `Playtube-Setup-<version>.exe` from the
[releases page](https://github.com/fojadrachi/Playtube/releases). Requires Windows 10/11 with the
WebView2 runtime (included in Windows 11). It installs per user to `%LOCALAPPDATA%\Programs\Playtube`
(no admin rights) and replaces any older Playtube, including the former Python versions. Settings and
login are kept.

## Updates

Like the NNN Launcher: Playtube checks GitHub's `releases/latest` at startup (after 5 s) and every
`updates.check_interval_hours` hours. When a newer version exists, the Settings tab shows “Install
update”: Playtube downloads `Playtube-Setup-<version>.exe`, verifies the SHA-256 checksum provided by
GitHub, runs the setup silently and quits; the setup restarts Playtube afterwards.

## Control from your phone

1. In the Settings tab, turn on “Allow control from a phone on the same Wi-Fi” and save. Windows asks
   once about the firewall – allow it for private networks.
2. “Pair phone”: Playtube shows a 6-digit code (valid for 2 minutes).
3. In the Android app, tap the round button on the right edge, pick the PC and enter the code.

Implementation ([src/lan.rs](src/lan.rs)): TCP port 47811, discovery via UDP broadcast to port 47812,
the same line protocol as the Stream Dock control, preceded by `pair`/`hello` with a token. Off by
default; only private networks and only after pairing; only a SHA-256 of the token is stored.

## Playtube Jam

**Jam** tab: enter your name, **Start jam** or enter a code/invite link and **Join**. Invite via
link (copy) or QR code; friends join with Playtube for PC, Playtube for Android or in the browser.
Playback happens in the YouTube Music tab, which Playtube keeps in sync with the jam. With control
rights, the tab itself, the tray, the Stream Dock and the phone control the jam. `playtube://jam/<CODE>`
links open Playtube directly. Code: [src/jam.rs](src/jam.rs), [src/jam_ui.rs](src/jam_ui.rs); the
service lives in the separate `playtube-jam` project.

## Remote control (Stream Dock)

Named pipe `\\.\pipe\Playtube.Remote.Playtube`, one JSON message per line, e.g.
`{"id":1,"cmd":"play_pause","target":"auto"}`. Commands are a fixed whitelist (including `list_playlists`, `play_playlist`, `queue`, `play_queue_item`), see
[src/remote.rs](src/remote.rs). Disable with `"remote_control": {"enabled": false}` in `config.json`.

## Development

```powershell
cargo run                       # debug start
cargo test                      # unit tests
.\build-release.ps1             # dist\Playtube-Setup-<v>.exe (needs Inno Setup 6)
```

`PLAYTUBE_DATA_DIR=<folder>` makes Playtube use its own data folder (and pipe names) for testing.
Diagnostic log: `<data folder>\logs\playtube.log`.

## Publishing a new version

Bump the version in `Cargo.toml`, update `release_notes.md`, commit, push tag `vX.Y.Z`.
[.github/workflows/release.yml](.github/workflows/release.yml) tests, builds the setup
and publishes it as release “Playtube vX.Y.Z”.

Playtube is a private project and not affiliated with Google, YouTube or Discord.
