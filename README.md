**Sprache / Language:** [Deutsch](#deutsch) · [English](#english)

<a id="deutsch"></a>

# Playtube Edge (Deutsch)

> **Das ist die „Edge“-Variante von Playtube (Branch `edge`).**
> **Diese Version brauchst du, wenn du selbst hochgeladene Musik in YouTube Music hören
> willst.** Die normale Playtube (Branch `main`) zeigt bei eigenen Uploads „Dieses
> Videoformat wird nicht unterstützt“, weil ihre Browser-Engine (QtWebEngine) kein AAC und
> kein H.264 abspielen kann. Playtube Edge rendert stattdessen mit der **Microsoft-Edge-Engine
> (WebView2)** – damit laufen auch Uploads.
>
> - Download: [Releases](https://git.fojadrachi.de/Fojadrachi/Playtube/releases) → Eintrag „Playtube
>   Edge vX.Y.Z-edge“ → `PlaytubeEdge-Setup-vX.Y.Z.exe`. Voraussetzung: Windows 10/11 mit
>   WebView2-Runtime (Windows 11: vorinstalliert).
> - Läuft **neben** der normalen Playtube (eigener Ordner `%APPDATA%\PlaytubeEdge`, eigenes
>   Login – beim ersten Start einmal bei Google anmelden).
> - Updates kommen nur über die `-edge`-Releases (die normale Playtube sieht sie nie).
> - Entwicklung: `pip install -r requirements.txt`, dann `python main.py`. Die
>   WebView2-DLLs liegen in `vendor/webview2` (Microsoft, NuGet `Microsoft.Web.WebView2`).
>
> Der Rest dieser README beschreibt die gemeinsame Basis mit der normalen Playtube; wo dort
> „QtWebEngine“ steht, ist in dieser Variante WebView2 gemeint.

Eigenständige Desktop-App für YouTube & YouTube Music (kein Browser-Fenster, keine
Erweiterung) mit:

- **Zwei Tabs** – YouTube und YouTube Music laufen parallel, Musik spielt im
  Hintergrund weiter wenn du zu Videos wechselst.
- **Login/Premium** – eigenes, persistentes Profil (`%APPDATA%\Playtube`), einmal bei
  Google anmelden reicht für beide Dienste.
- **Discord Rich Presence** – zeigt Titel, Kanal/Interpret, Fortschrittsbalken und
  einen Link-Button in deinem Discord-Profil, sobald etwas läuft. Ist der
  **Einstellungen-Tab** offen, steht dort stattdessen „In den Einstellungen“ und welches
  Feld du gerade bearbeitest (z.B. „Discord Rich Presence › Client-ID“) - nur der
  Feldname, nie der Inhalt. Das folgt der Option „Status anzeigen, wenn gerade nichts
  läuft“ (aus = auch kein Einstellungs-Status). Discord übernimmt Änderungen höchstens
  alle 15 Sekunden, die Anzeige hinkt dem Klicken also etwas hinterher.
- **Audioausgabe pro Tab** – YouTube und YouTube Musik lassen sich in den Einstellungen
  auf verschiedene Ausgabegeräte legen (z.B. getrennte Sonar-Kanäle), siehe unten.
- **Sprache** – Deutsch oder Englisch (US), im Einstellungen-Tab wählbar (siehe unten).
- **System-Tray** – Fenster schliessen minimiert nur (Musik läuft weiter), Rechtsklick
  aufs Tray-Icon zum Beenden, Play/Pause/Skip direkt aus dem Menü.
- **Eigener Name** – erscheint als "Playtube" im Taskmanager, Fenstertitel, Alt-Tab
  und (nach dem Packaging, siehe unten) im Lautstärkemixer statt als "python".

## Installation (Windows)

Lade auf der [Releases-Seite](https://git.fojadrachi.de/Fojadrachi/Playtube/releases)
`PlaytubeEdge-Setup-vX.Y.Z.exe` herunter und starte sie. Der Installer

- installiert Playtube **pro Benutzer** nach `%LOCALAPPDATA%\Programs\Playtube` (keine
  Admin-Rechte nötig) und legt Startmenü-Eintrag (optional Desktop-Verknüpfung) an,
- ersetzt bei jeder späteren Ausführung die vorhandene Installation **an Ort und
  Stelle** (feste Installer-ID) - es gibt also immer genau EINE installierte Version, nie
  mehrere nebeneinander,
- beendet dafür ein noch laufendes Playtube selbst und räumt den alten Programmordner
  auf, damit keine Reste alter Versionen übrig bleiben,
- lässt Login und Einstellungen (`%APPDATA%\Playtube`) unangetastet; beim Deinstallieren
  über "Apps & Features" wird gefragt, ob sie mitgelöscht werden sollen.

Ab dann aktualisiert sich Playtube selbst (siehe "Automatische Updates") - die
Setup-Datei wird dafür nicht erneut von Hand gebraucht. Das ZIP-Paket
(`Playtube-vX.Y.Z-win64.zip`) bleibt als portable Variante ohne Installation erhalten;
Playtube bietet einer portablen Kopie beim nächsten Update automatisch an, in die
reguläre Installation zu wechseln (danach kann der alte Ordner gelöscht werden).

## Schnellstart (Entwicklung)

```powershell
python -m venv .venv
.venv\Scripts\pip install -r requirements.txt
.venv\Scripts\python main.py
```

Beim ersten Start meldest du dich einmal in einem der beiden Tabs mit deinem
Google-Konto an (Symbol oben rechts auf youtube.com bzw. music.youtube.com) - der
Login bleibt danach dauerhaft gespeichert.

## Sprache

Im **Einstellungen-Tab** wählst du unter „Allgemein → Sprache“ zwischen **Deutsch** und
**English (US)**. Nach „Speichern“ stellt die App sofort um (Tabs, Tray-Menü, Einstellungen,
Update-Meldungen und die Texte der Discord Rich Presence); ein Neustart ist nicht nötig. Nur die
Menüs der Webseiten selbst (z. B. Rechtsklick) folgen erst nach dem nächsten Start von Playtube
Edge. Die Sprache von YouTube selbst stellst du in deinem YouTube-/Google-Konto ein.

In `config.json` heisst die Einstellung `language`: `"de"`, `"en"` oder leer (= automatisch
nach der Windows-Sprache). Alle Texte stehen in [playtube/i18n.py](playtube/i18n.py); ein Test
([tests/test_i18n.py](tests/test_i18n.py)) prüft, dass jeder Text in beiden Sprachen mit denselben
Platzhaltern vorhanden ist.

## Discord Rich Presence einrichten

Die Client-ID ist bereits in `config.json` eingetragen (`discord.client_id`). Falls du
sie ändern oder eine eigene Anwendung nutzen willst:

1. Auf https://discord.com/developers/applications eine neue Application anlegen.
2. Die **Application ID** in `config.json` unter `discord.client_id` eintragen.
3. Optional, für eigene Icons: unter *Rich Presence → Art Assets* zwei Bilder mit den
   exakten Schlüsseln `youtube_logo` und `music_logo` hochladen (Playtube nutzt genau
   diese Keys automatisch). Ohne hochgeladene Assets funktioniert die Presence trotzdem,
   nur ohne Bild.
4. Discord muss auf demselben Rechner laufen, damit die Presence angezeigt wird.

In `config.json` lassen sich zudem `update_interval_seconds` (Mindestabstand zwischen
Updates) und `show_idle_presence` (Status anzeigen, wenn gerade nichts läuft) anpassen.

## Audioausgabe pro Tab (YouTube / YouTube Musik)

Im **Einstellungen-Tab** legst du unter „Audioausgabe“ getrennt fest, über welches
Ausgabegerät der Ton von **YouTube** und von **YouTube Musik** läuft (Standard:
Systemstandard). So kannst du z.B. YouTube auf „Sonar - Media“ und YouTube Musik auf
„Sonar - Aux“ legen und beide getrennt regeln. Nach „Speichern“ gilt die Auswahl sofort,
auch für bereits geöffnete Seiten (kein Neuladen nötig).

Wichtig zu wissen:

- **Windows zeigt weiterhin einen Eintrag „Playtube“.** Chromium (QtWebEngine) spielt den
  Ton beider Tabs über *einen* gemeinsamen Audio-Prozess ab, deshalb kann Windows (auch
  „App-Lautstärke und Geräteeinstellungen“) die Tabs nicht einzeln benennen oder routen.
  Getrennt wird stattdessen über das **Gerät** - in Sonar & Co. also über den gewählten
  Kanal/das gewählte Gerät.
- **Freigabe der Gerätenamen:** Chromium blendet Gerätenamen aus, solange die Seite keine
  Mikrofon-Berechtigung hat. Damit die Seite das gewählte Gerät findet, erteilt Playtube
  YouTube/YouTube Musik diese Freigabe - nur, solange mindestens ein Tab ein eigenes Gerät
  nutzt, nur für diese beiden Seiten und nur für die laufende Sitzung (sie wird nicht
  gespeichert und bei jedem Start neu erteilt). Es wird nichts aufgenommen.
- Ist das gewählte Gerät gerade nicht angeschlossen, bleibt die Auswahl (als „nicht
  verfügbar“) gespeichert und der Ton läuft solange über den Systemstandard.
- Technik: [playtube/audio_routing.py](playtube/audio_routing.py) legt per
  `HTMLMediaElement.setSinkId` die Medienelemente der Seite auf das Gerät (Suche über den
  Gerätenamen). In der Konfiguration stehen die Namen unter `audio.youtube_output` und
  `audio.music_output` (leer = Systemstandard).

## Fernsteuerung (Stream Dock)

Playtube lässt sich lokal fernsteuern, z.B. über das Stream-Dock-Plugin
`com.fojadrachi.playtube.sdPlugin` (Ajazz/Mirabox AKP153E & Co.). Dafür lauscht Playtube
auf einer Named Pipe (`\\.\pipe\Playtube.Remote.Playtube`, im Entwicklungsmodus
`...PlaytubeDev`). Es gibt keinen Netzwerk-Port, verbinden darf nur der angemeldete
Benutzer.

- Protokoll: eine JSON-Nachricht pro Zeile, z.B. `{"id":1,"cmd":"play_pause","target":"auto"}`,
  Antwort `{"id":1,"ok":true}`. Details in [playtube/remote_control.py](playtube/remote_control.py).
- Befehle: `status`, `show`, `switch_tab`, `play_pause`, `play`, `pause`, `next`,
  `previous`, `seek`, `volume_change`, `set_volume`, `mute_toggle`, `like`, `dislike`,
  `shuffle`, `repeat`, `list_playlists`, `play_playlist`. Es gibt eine feste Whitelist;
  beliebiges JavaScript oder URLs lassen sich nicht senden.
- `target: "auto"` steuert den Tab, der gerade abspielt, sonst den sichtbaren.
- Abschalten: in `config.json` `"remote_control": {"enabled": false}`.
- Tests: `.venv\Scripts\python -m unittest discover -s tests -v`

## App als eigenständige Playtube.exe packen

Für die volle Taskmanager-/Audiomixer-Markierung wird die App als eigene .exe gebaut
(ein via `python main.py` gestarteter Prozess heisst in Windows immer "python.exe" -
nur eine kompilierte .exe mit eigenem Namen und eigener Versionsinfo kann das ändern):

```powershell
.venv\Scripts\pip install pyinstaller
powershell -ExecutionPolicy Bypass -File packaging\build.ps1
```

Ergebnis liegt danach unter `dist\Playtube\Playtube.exe`. Ist
[Inno Setup 6](https://jrsoftware.org/isinfo.php) installiert
(`winget install --id JRSoftware.InnoSetup -e`), baut das Skript daraus ausserdem den
Installer `dist\installer\Playtube-Setup-vX.Y.Z.exe` (Skript:
[packaging/playtube.iss](packaging/playtube.iss); mit `-SkipInstaller` überspringbar).
Eine portable, nicht per Setup installierte `.exe` legt beim ersten Start automatisch eine
Verknüpfung im Windows-Startmenü an - eine per Setup installierte Kopie hat den
Eintrag bereits vom Installer.

Das Build-Skript benennt
zusätzlich den QtWebEngine-Hilfsprozess (der den eigentlichen Ton ausgibt) zu
`PlaytubeHelper.exe` um, damit er im Taskmanager nicht als `QtWebEngineProcess`
auftaucht. Für eine vollständige Umbenennung inkl. Icon/Versionsinfo dieses
Hilfsprozesses (relevant für den Lautstärkemixer) zusätzlich
[rcedit](https://github.com/electron/rcedit/releases) als `packaging\rcedit.exe`
ablegen - das Skript nutzt es automatisch, wenn vorhanden. Ohne rcedit funktioniert
alles genauso, nur zeigt der Lautstärkemixer für den Ton-Unterprozess je nach
Windows-Version eventuell weiterhin "QtWebEngineProcess" statt "Playtube" (rein
kosmetisch - Namensgebung von Chromium-Hilfsprozessen ist ein bekanntes,
Windows-versionsabhängiges Verhalten, das selbst grosse Electron-Apps nur mit rcedit
o.ä. umgehen).

## Automatische Updates

Playtube prüft beim Start und danach alle `updates.check_interval_hours` Stunden
(Standard 6, im **Einstellungen-Tab** oder in `config.json` einstellbar) die
[Releases](https://git.fojadrachi.de/Fojadrachi/Playtube/releases) auf dem eigenen Gitea-Server (nur Tags `-edge`). Im
Einstellungen-Tab gibt es zusätzlich einen "Jetzt nach Updates suchen"-Button mit
Status-Anzeige und Fortschrittsbalken für den Download. Gibt es eine neuere Version,
fragt ein Dialog, ob sie installiert werden soll:

- **Windows, per Setup installiert** (siehe "Installation"): lädt bevorzugt das kleine
  **Patch-Paket** herunter (nur die `Playtube.exe` mit unserem Anwendungscode, ca. 2-3 MB)
  und überschreibt damit die Datei im Installationsordner - der riesige PySide6/
  QtWebEngine-Laufzeitordner (`_internal/`) bleibt unangetastet, da er sich zwischen
  normalen Patch-Releases nicht ändert. Die PySide6-Version steht im Dateinamen des
  Patches (`...-pyside6.11.2-patch.play`); weicht sie von der installierten Laufzeit ab,
  ist ein reines .exe-Patch nicht mehr passend und Playtube nimmt stattdessen den
  **Setup-Installer**, der alles (auch die Laufzeit) an Ort und Stelle erneuert und
  Playtube danach wieder startet. Der Versionseintrag in "Apps & Features" wird nach
  einem Patch ebenfalls nachgezogen.
- **Windows, portable Kopie** (aus dem ZIP entpackt, z.B. in Downloads): lädt den
  **Setup-Installer** und führt ihn still aus. Playtube liegt danach in der regulären
  Installation - Startmenü-Eintrag und Dateizuordnung zeigen dorthin, die alte portable
  Kopie ist nicht mehr nötig (und kann gelöscht werden). So entstehen nicht länger
  versionierte Ordner nebeneinander. Gibt es (bei älteren Releases) keinen Installer,
  wird wie früher das volle ZIP über den vorhandenen Ordner kopiert.
- **Linux** (`Playtube`-Binary): Patch-Paket (nur das Binary) bzw. volles `.tar.gz`.
  Die App startet sich in allen Fällen danach selbst neu.
- **Entwicklungsmodus** (`python main.py`): führt `git pull` + `pip install -r
  requirements.txt` aus und startet den Python-Prozess neu.

Auto-Update lässt sich im Einstellungen-Tab oder in `config.json` unter
`updates.enabled` deaktivieren.

Nach einem erkannten Update wird beim nächsten Start automatisch der QtWebEngine-
HTTP-Cache geleert (`webprofile/cache`) - alte Cache-Einträge können sonst nicht mehr
zum neuen Code passen (frühere Ursache für fehlende Icons). Der Login bleibt davon
unberührt, da Cookies/LocalStorage in einem komplett getrennten Ordner
(`webprofile/storage`) liegen.

### Patch-Dateien manuell installieren (`.play`)

Windows-Patch-Pakete tragen die eigene Dateiendung `.play` statt `.zip` (technisch
weiterhin ein ganz normales ZIP-Archiv). Playtube registriert `.play` beim ersten Start
automatisch als Windows-Dateizuordnung - eine manuell heruntergeladene
`Playtube-vX.Y.Z-win64-patch.play` (z.B. von der
[Releases-Seite](https://git.fojadrachi.de/Fojadrachi/Playtube/releases)) lässt sich also
einfach per Doppelklick installieren, ohne dass Playtube selbst etwas herunterladen
muss.

### Eine neue Version veröffentlichen

```powershell
powershell -ExecutionPolicy Bypass -File packaging\release.ps1 -Version 1.1.0
```

Das Skript setzt die Versionsnummer, committet, erstellt Git-Tag `v1.1.0` und pusht zu
`origin` (dein Gitea unter https://git.fojadrachi.de/Fojadrachi/Playtube). Für Playtube Edge
trägt der Tag den Zusatz `-edge` (z. B. `v1.0.1-edge`). Der gepushte Tag löst automatisch die
Gitea-Actions-Pipeline ([.gitea/workflows/release.yml](.gitea/workflows/release.yml)) aus, die
Playtube Edge für Windows baut und als **Vorabversion** auf dem Gitea veröffentlicht -
Fortschritt unter https://git.fojadrachi.de/Fojadrachi/Playtube/actions.

#### Releases bauen (einmalige Einrichtung)

GitHub wird nirgends mehr benutzt. Die Pipeline braucht:

1. **Einen Gitea-Actions-Runner unter Windows** (`act_runner`, Label `windows`, Host-Modus)
   mit Python 3.12, git, PowerShell und [Inno Setup 6](https://jrsoftware.org/isinfo.php).
   Registrierung z. B.:
   `act_runner register --instance https://git.fojadrachi.de --token <Runner-Token> --labels windows:host`
2. **Ein Secret `RELEASE_TOKEN`** im Repo (Einstellungen → Actions → Secrets): ein
   Zugriffstoken (Gitea → Einstellungen → Anwendungen) mit Schreibrecht auf das Repo.
3. In Gitea **Actions für das Repo aktivieren** (Einstellungen → Erweitert → Repository-Einheiten).

**Ohne Runner** kannst du selbst bauen und veröffentlichen:

```powershell
powershell -ExecutionPolicy Bypass -File packaging\build.ps1
$env:GITEA_TOKEN = "<Zugriffstoken>"
powershell -ExecutionPolicy Bypass -File packaging\publish_release.ps1 -Tag v1.0.1-edge -Prerelease `
    -Files dist\installer\PlaytubeEdge-Setup-v1.0.1.exe -NotesFile packaging\release_notes.md
```

Für Windows entstehen dabei drei Dateien: `Playtube-Setup-vX.Y.Z.exe` (Installer, für
Neueinsteiger und volle Updates), `Playtube-vX.Y.Z-win64.zip` (portabel) und das kleine
`...-pyside<Version>-patch.play` (schnelles Update). Der Installer bekommt seine
Versionsnummer aus `playtube/__init__.py` (das `release.ps1` vor dem Tag setzt).

Alle Nutzer mit einer laufenden Playtube-Installation (Windows oder Linux) bekommen die
neue Version danach automatisch angeboten.

## Native Linux-Version

**Hinweis:** Playtube Edge nutzt WebView2 und läuft nur unter Windows. Die Linux-Beschreibung
unten gehört zur normalen Playtube (Branch `main`, QtWebEngine); für Edge entstehen keine
Linux-Pakete.

**Fertiges Release installieren** (richtet Startmenü-Eintrag + Icon ein):

```sh
tar -xzf Playtube-vX.Y.Z-linux-x86_64.tar.gz
cd Playtube
sh install-linux.sh
```

Danach ist Playtube über das Anwendungsmenü oder den Befehl `playtube` startbar.

**Aus dem Quellcode starten:**

```sh
python3 -m venv .venv
.venv/bin/pip install -r requirements.txt
.venv/bin/python main.py
```

QtWebEngine benötigt unter Linux ein paar System-Bibliotheken (auf Debian/Ubuntu):
`sudo apt install libxkbcommon0 libegl1 libnss3 libxcomposite1 libxdamage1 libxrandr2 libgbm1 libasound2t64 libatk-bridge2.0-0 libcups2`.

## Hinweise

- **"Dieser Browser ist unter Umständen nicht sicher" beim Google-Login:** Playtube
  setzt einen echten Chrome-User-Agent inkl. passender `Sec-CH-UA`-Header, damit Google
  das eingebettete Chromium (QtWebEngine) nicht als unsicheres WebView erkennt. Sollte
  die Meldung dennoch erscheinen: alle Playtube-Fenster/-Prozesse schliessen und neu
  starten (die Header greifen erst ab dem nächsten Prozessstart), notfalls einmal den
  Profilordner `%APPDATA%\Playtube\webprofile` (bzw. `PlaytubeDev` im
  Entwicklungsmodus) löschen und neu anmelden.
- **Auf YouTube/YouTube Music fehlen alle Icons (Play/Pause, Suche, Menü ...):** Fast
  sicher laufen zwei Playtube-Prozesse gleichzeitig auf demselben Browser-Profil - die
  zweite Instanz rendert dann keine Icons mehr. Passiert leicht, weil das Schliessen des
  Fensters Playtube nur in den Tray minimiert: eine ältere Version läuft unsichtbar
  weiter, während eine neu heruntergeladene zusätzlich gestartet wird. Abhilfe: alle
  Playtube-Instanzen über das Tray-Icon (Rechtsklick -> Beenden) bzw. im Taskmanager
  beenden und nur EINE neu starten. Ab der Version mit
  [playtube/single_instance.py](playtube/single_instance.py) verhindert Playtube den
  Doppelstart selbst (ein zweiter Start holt das laufende Fenster nach vorn); eine
  ältere Instanz ohne diesen Schutz wird dabei nicht erkannt und muss einmalig manuell
  beendet werden.
- 4K/Premium-Videoqualität kann eingeschränkt sein, da die Open-Source-Variante von
  QtWebEngine kein Widevine-DRM mitbringt (Standard-Qualitäten funktionieren normal).
- Icon/Branding-Bilder liegen unter `assets/` und wurden mit `tools/generate_icon.py`
  erzeugt (Pillow) - bei Bedarf einfach eigenes `icon.ico`/`icon.png` dort ersetzen.

---

<a id="english"></a>

# Playtube Edge (English)

> **This is the "Edge" variant of Playtube (branch `edge`).**
> **You need this version if you want to listen to music you uploaded yourself in YouTube
> Music.** The regular Playtube (branch `main`) shows "This video format is not supported"
> for your own uploads, because its browser engine (QtWebEngine) cannot play AAC or H.264.
> Playtube Edge renders with the **Microsoft Edge engine (WebView2)** instead – so uploads
> work too.
>
> - Download: [Releases](https://git.fojadrachi.de/Fojadrachi/Playtube/releases) → entry
>   "Playtube Edge vX.Y.Z-edge" → `PlaytubeEdge-Setup-vX.Y.Z.exe`. Requirement: Windows 10/11
>   with the WebView2 Runtime (preinstalled on Windows 11).
> - Runs **alongside** the regular Playtube (its own folder `%APPDATA%\PlaytubeEdge`, its own
>   login – sign in to Google once on first start).
> - Updates only come through the `-edge` releases (the regular Playtube never sees them).
> - Development: `pip install -r requirements.txt`, then `python main.py`. The WebView2 DLLs
>   live in `vendor/webview2` (Microsoft, NuGet `Microsoft.Web.WebView2`).
>
> The rest of this README describes the base shared with the regular Playtube; where it says
> "QtWebEngine", this variant means WebView2.

A standalone desktop app for YouTube & YouTube Music (no browser window, no extension)
with:

- **Two tabs** – YouTube and YouTube Music run side by side, music keeps playing in the
  background when you switch to videos.
- **Login/Premium** – its own persistent profile (`%APPDATA%\Playtube`); signing in to
  Google once covers both services.
- **Discord Rich Presence** – shows title, channel/artist, a progress bar and a link button
  in your Discord profile as soon as something is playing. While the **Settings tab** is
  open, it shows "In the settings" and which field you are editing instead (e.g.
  "Discord Rich Presence › Client ID") - only the field name, never its content. This
  follows the option "Show status when nothing is playing" (off = no settings status
  either). Discord accepts changes at most every 15 seconds, so the display lags slightly
  behind your clicks.
- **Audio output per tab** – YouTube and YouTube Music can be sent to different output
  devices in the settings (e.g. separate Sonar channels), see below.
- **Language** – German or English (US), selectable in the Settings tab (see below).
- **System tray** – closing the window only minimizes it (music keeps playing), right-click
  the tray icon to quit, play/pause/skip right from the menu.
- **Its own name** – shows up as "Playtube" in Task Manager, the window title, Alt-Tab and
  (after packaging, see below) in the volume mixer instead of "python".

## Installation (Windows)

Download `PlaytubeEdge-Setup-vX.Y.Z.exe` from the
[releases page](https://git.fojadrachi.de/Fojadrachi/Playtube/releases) and run it. The
installer

- installs Playtube **per user** to `%LOCALAPPDATA%\Programs\Playtube` (no admin rights
  needed) and creates a Start menu entry (optionally a desktop shortcut),
- replaces the existing installation **in place** on every later run (fixed installer
  ID) - so there is always exactly ONE installed version, never several side by side,
- closes a running Playtube by itself and cleans up the old program folder so no leftovers
  of old versions remain,
- leaves login and settings (`%APPDATA%\Playtube`) untouched; when uninstalling via
  "Apps & Features" you are asked whether to delete them too.

From then on Playtube updates itself (see "Automatic updates") - you don't need the setup
file again. The ZIP package (`Playtube-vX.Y.Z-win64.zip`) remains as a portable variant
without installation; on the next update Playtube automatically offers a portable copy to
switch to the regular installation (afterwards the old folder can be deleted).

## Quick start (development)

```powershell
python -m venv .venv
.venv\Scripts\pip install -r requirements.txt
.venv\Scripts\python main.py
```

On first start, sign in once with your Google account in one of the two tabs (icon at the
top right of youtube.com or music.youtube.com) - the login is saved permanently after that.

## Language

In the **Settings tab** under "General → Language" you choose between **Deutsch** and
**English (US)**. After "Save" the app switches immediately (tabs, tray menu, settings,
update messages and the Discord Rich Presence texts); no restart needed. Only the menus of
the web pages themselves (e.g. right-click) follow after the next start of Playtube Edge.
The language of YouTube itself is set in your YouTube/Google account.

In `config.json` the setting is `language`: `"de"`, `"en"` or empty (= automatic based on
the Windows language). All texts are in [playtube/i18n.py](playtube/i18n.py); a test
([tests/test_i18n.py](tests/test_i18n.py)) checks that every text exists in both languages
with the same placeholders.

## Setting up Discord Rich Presence

The client ID is already set in `config.json` (`discord.client_id`). If you want to change
it or use your own application:

1. Create a new application at https://discord.com/developers/applications.
2. Enter the **Application ID** in `config.json` under `discord.client_id`.
3. Optional, for your own icons: under *Rich Presence → Art Assets*, upload two images with
   the exact keys `youtube_logo` and `music_logo` (Playtube uses exactly these keys
   automatically). Without uploaded assets the presence still works, just without an image.
4. Discord must be running on the same computer for the presence to show.

In `config.json` you can also adjust `update_interval_seconds` (minimum interval between
updates) and `show_idle_presence` (show status when nothing is playing).

## Audio output per tab (YouTube / YouTube Music)

In the **Settings tab** under "Audio output" you set separately which output device the
sound of **YouTube** and of **YouTube Music** goes to (default: system default). This way
you can e.g. put YouTube on "Sonar - Media" and YouTube Music on "Sonar - Aux" and control
them separately. After "Save" the selection applies immediately, also to pages that are
already open (no reload needed).

Good to know:

- **Windows still shows one "Playtube" entry.** Chromium (QtWebEngine) plays the sound of
  both tabs through *one* shared audio process, so Windows (including "App volume and
  device preferences") cannot name or route the tabs individually. They are separated by
  **device** instead - in Sonar & co. by the selected channel/device.
- **Access to device names:** Chromium hides device names as long as the page has no
  microphone permission. So the page can find the selected device, Playtube grants
  YouTube/YouTube Music this permission - only while at least one tab uses a specific
  device, only for these two sites and only for the current session (it is not saved and is
  granted again on every start). Nothing is recorded.
- If the selected device is currently not connected, the selection stays saved (as "not
  available") and the sound goes through the system default until then.
- Technical details: [playtube/audio_routing.py](playtube/audio_routing.py) routes the
  page's media elements to the device via `HTMLMediaElement.setSinkId` (looked up by device
  name). In the configuration the names are stored under `audio.youtube_output` and
  `audio.music_output` (empty = system default).

## Remote control (Stream Dock)

Playtube can be controlled locally, e.g. via the Stream Dock plugin
`com.fojadrachi.playtube.sdPlugin` (Ajazz/Mirabox AKP153E & co.). For this Playtube listens
on a named pipe (`\\.\pipe\Playtube.Remote.Playtube`, in development mode
`...PlaytubeDev`). There is no network port; only the signed-in user may connect.

- Protocol: one JSON message per line, e.g. `{"id":1,"cmd":"play_pause","target":"auto"}`,
  response `{"id":1,"ok":true}`. Details in [playtube/remote_control.py](playtube/remote_control.py).
- Commands: `status`, `show`, `switch_tab`, `play_pause`, `play`, `pause`, `next`,
  `previous`, `seek`, `volume_change`, `set_volume`, `mute_toggle`, `like`, `dislike`,
  `shuffle`, `repeat`, `list_playlists`, `play_playlist`. There is a fixed whitelist;
  arbitrary JavaScript or URLs cannot be sent.
- `target: "auto"` controls the tab that is currently playing, otherwise the visible one.
- Turn off: in `config.json` `"remote_control": {"enabled": false}`.
- Tests: `.venv\Scripts\python -m unittest discover -s tests -v`

## Packaging the app as a standalone Playtube.exe

For full Task Manager/volume mixer labeling the app is built as its own .exe (a process
started via `python main.py` is always called "python.exe" in Windows - only a compiled
.exe with its own name and version info can change that):

```powershell
.venv\Scripts\pip install pyinstaller
powershell -ExecutionPolicy Bypass -File packaging\build.ps1
```

The result is then at `dist\Playtube\Playtube.exe`. If
[Inno Setup 6](https://jrsoftware.org/isinfo.php) is installed
(`winget install --id JRSoftware.InnoSetup -e`), the script also builds the installer
`dist\installer\Playtube-Setup-vX.Y.Z.exe` from it (script:
[packaging/playtube.iss](packaging/playtube.iss); skip with `-SkipInstaller`). A portable
`.exe` not installed via setup automatically creates a shortcut in the Windows Start menu on
first start - a copy installed via setup already has the entry from the installer.

The build script additionally renames the QtWebEngine helper process (which actually
outputs the sound) to `PlaytubeHelper.exe`, so it doesn't show up as `QtWebEngineProcess` in
Task Manager. For a complete rename including icon/version info of this helper process
(relevant for the volume mixer), also place
[rcedit](https://github.com/electron/rcedit/releases) as `packaging\rcedit.exe` - the
script uses it automatically if present. Without rcedit everything works the same, the
volume mixer may just keep showing "QtWebEngineProcess" instead of "Playtube" for the sound
subprocess depending on the Windows version (purely cosmetic - naming of Chromium helper
processes is a known, Windows-version-dependent behavior that even large Electron apps only
work around with rcedit or similar).

## Automatic updates

Playtube checks the [releases](https://git.fojadrachi.de/Fojadrachi/Playtube/releases) on
its own Gitea server (only `-edge` tags) at start and then every
`updates.check_interval_hours` hours (default 6, adjustable in the **Settings tab** or in
`config.json`). The Settings tab also has a "Check for updates now" button with a status
display and a progress bar for the download. If there is a newer version, a dialog asks
whether to install it:

- **Windows, installed via setup** (see "Installation"): preferably downloads the small
  **patch package** (only the `Playtube.exe` with our application code, approx. 2-3 MB) and
  overwrites the file in the installation folder with it - the huge PySide6/QtWebEngine
  runtime folder (`_internal/`) stays untouched, since it doesn't change between normal
  patch releases. The PySide6 version is in the patch's file name
  (`...-pyside6.11.2-patch.play`); if it differs from the installed runtime, a pure .exe
  patch no longer fits and Playtube uses the **setup installer** instead, which renews
  everything (including the runtime) in place and starts Playtube again afterwards. The
  version entry in "Apps & Features" is updated after a patch as well.
- **Windows, portable copy** (extracted from the ZIP, e.g. in Downloads): downloads the
  **setup installer** and runs it silently. Playtube is then in the regular installation -
  Start menu entry and file association point there, the old portable copy is no longer
  needed (and can be deleted). This way versioned folders no longer pile up side by side.
  If there is no installer (for older releases), the full ZIP is copied over the existing
  folder as before.
- **Linux** (`Playtube` binary): patch package (only the binary) or full `.tar.gz`.
  In all cases the app restarts itself afterwards.
- **Development mode** (`python main.py`): runs `git pull` + `pip install -r
  requirements.txt` and restarts the Python process.

Auto-update can be disabled in the Settings tab or in `config.json` under
`updates.enabled`.

After a detected update, the QtWebEngine HTTP cache (`webprofile/cache`) is cleared
automatically on the next start - old cache entries may otherwise no longer match the new
code (earlier cause of missing icons). The login is not affected, since cookies/LocalStorage
are stored in a completely separate folder (`webprofile/storage`).

### Installing patch files manually (`.play`)

Windows patch packages use their own file extension `.play` instead of `.zip` (technically
still a completely normal ZIP archive). Playtube registers `.play` as a Windows file
association automatically on first start - so a manually downloaded
`Playtube-vX.Y.Z-win64-patch.play` (e.g. from the
[releases page](https://git.fojadrachi.de/Fojadrachi/Playtube/releases)) can simply be
installed by double-clicking, without Playtube having to download anything itself.

### Publishing a new version

```powershell
powershell -ExecutionPolicy Bypass -File packaging\release.ps1 -Version 1.1.0
```

The script sets the version number, commits, creates the Git tag `v1.1.0` and pushes to
`origin` (your Gitea at https://git.fojadrachi.de/Fojadrachi/Playtube). For Playtube Edge
the tag carries the suffix `-edge` (e.g. `v1.0.1-edge`). The pushed tag automatically
triggers the Gitea Actions pipeline ([.gitea/workflows/release.yml](.gitea/workflows/release.yml)),
which builds Playtube Edge for Windows and publishes it as a **pre-release** on the Gitea -
progress at https://git.fojadrachi.de/Fojadrachi/Playtube/actions.

#### Building releases (one-time setup)

GitHub is no longer used anywhere. The pipeline needs:

1. **A Gitea Actions runner on Windows** (`act_runner`, label `windows`, host mode) with
   Python 3.12, git, PowerShell and [Inno Setup 6](https://jrsoftware.org/isinfo.php).
   Registration e.g.:
   `act_runner register --instance https://git.fojadrachi.de --token <runner token> --labels windows:host`
2. **A secret `RELEASE_TOKEN`** in the repo (Settings → Actions → Secrets): an access token
   (Gitea → Settings → Applications) with write access to the repo.
3. In Gitea, **enable Actions for the repo** (Settings → Advanced → Repository units).

**Without a runner** you can build and publish yourself:

```powershell
powershell -ExecutionPolicy Bypass -File packaging\build.ps1
$env:GITEA_TOKEN = "<access token>"
powershell -ExecutionPolicy Bypass -File packaging\publish_release.ps1 -Tag v1.0.1-edge -Prerelease `
    -Files dist\installer\PlaytubeEdge-Setup-v1.0.1.exe -NotesFile packaging\release_notes.md
```

For Windows this produces three files: `Playtube-Setup-vX.Y.Z.exe` (installer, for new
users and full updates), `Playtube-vX.Y.Z-win64.zip` (portable) and the small
`...-pyside<version>-patch.play` (quick update). The installer gets its version number from
`playtube/__init__.py` (which `release.ps1` sets before the tag).

All users with a running Playtube installation (Windows or Linux) are then offered the new
version automatically.

## Native Linux version

**Note:** Playtube Edge uses WebView2 and only runs on Windows. The Linux description below
belongs to the regular Playtube (branch `main`, QtWebEngine); no Linux packages are built
for Edge.

**Install a finished release** (sets up Start menu entry + icon):

```sh
tar -xzf Playtube-vX.Y.Z-linux-x86_64.tar.gz
cd Playtube
sh install-linux.sh
```

After that Playtube can be started from the application menu or with the command
`playtube`.

**Run from source:**

```sh
python3 -m venv .venv
.venv/bin/pip install -r requirements.txt
.venv/bin/python main.py
```

On Linux, QtWebEngine needs a few system libraries (on Debian/Ubuntu):
`sudo apt install libxkbcommon0 libegl1 libnss3 libxcomposite1 libxdamage1 libxrandr2 libgbm1 libasound2t64 libatk-bridge2.0-0 libcups2`.

## Notes

- **"This browser or app may not be secure" during Google login:** Playtube sets a real
  Chrome user agent including matching `Sec-CH-UA` headers, so Google doesn't flag the
  embedded Chromium (QtWebEngine) as an insecure WebView. If the message still appears:
  close all Playtube windows/processes and restart (the headers only take effect from the
  next process start), if necessary delete the profile folder
  `%APPDATA%\Playtube\webprofile` once (or `PlaytubeDev` in development mode) and sign in
  again.
- **All icons are missing on YouTube/YouTube Music (play/pause, search, menu ...):** Almost
  certainly two Playtube processes are running at the same time on the same browser
  profile - the second instance then stops rendering icons. This happens easily because
  closing the window only minimizes Playtube to the tray: an older version keeps running
  invisibly while a newly downloaded one is started in addition. Fix: quit all Playtube
  instances via the tray icon (right-click -> Quit) or in Task Manager and start only ONE
  again. Since the version with [playtube/single_instance.py](playtube/single_instance.py)
  Playtube prevents the double start itself (a second start brings the running window to
  the front); an older instance without this protection is not detected and has to be
  quit manually once.
- 4K/Premium video quality may be limited, since the open-source variant of QtWebEngine
  doesn't include Widevine DRM (standard qualities work normally).
- Icon/branding images are in `assets/` and were generated with `tools/generate_icon.py`
  (Pillow) - simply replace `icon.ico`/`icon.png` there with your own if needed.
