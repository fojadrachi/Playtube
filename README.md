# Playtube

Eigenstaendige Desktop-App für YouTube & YouTube Music (kein Browser-Fenster, keine
Erweiterung) mit:

- **Zwei Tabs** – YouTube und YouTube Music laufen parallel, Musik spielt im
  Hintergrund weiter wenn du zu Videos wechselst.
- **Login/Premium** – eigenes, persistentes Profil (`%APPDATA%\Playtube`), einmal bei
  Google anmelden reicht fuer beide Dienste.
- **Discord Rich Presence** – zeigt Titel, Kanal/Interpret, Fortschrittsbalken und
  einen Link-Button in deinem Discord-Profil, sobald etwas laeuft.
- **System-Tray** – Fenster schliessen minimiert nur (Musik laeuft weiter), Rechtsklick
  aufs Tray-Icon zum Beenden, Play/Pause/Skip direkt aus dem Menue.
- **Eigener Name** – erscheint als "Playtube" im Taskmanager, Fenstertitel, Alt-Tab
  und (nach dem Packaging, siehe unten) im Lautstaerkemixer statt als "python".

## Schnellstart (Entwicklung)

```powershell
python -m venv .venv
.venv\Scripts\pip install -r requirements.txt
.venv\Scripts\python main.py
```

Beim ersten Start meldest du dich einmal in einem der beiden Tabs mit deinem
Google-Konto an (Symbol oben rechts auf youtube.com bzw. music.youtube.com) - der
Login bleibt danach dauerhaft gespeichert.

## Discord Rich Presence einrichten

Die Client-ID ist bereits in `config.json` eingetragen (`discord.client_id`). Falls du
sie aendern oder eine eigene Anwendung nutzen willst:

1. Auf https://discord.com/developers/applications eine neue Application anlegen.
2. Die **Application ID** in `config.json` unter `discord.client_id` eintragen.
3. Optional, fuer eigene Icons: unter *Rich Presence → Art Assets* zwei Bilder mit den
   exakten Schluesseln `youtube_logo` und `music_logo` hochladen (Playtube nutzt genau
   diese Keys automatisch). Ohne hochgeladene Assets funktioniert die Presence trotzdem,
   nur ohne Bild.
4. Discord muss auf demselben Rechner laufen, damit die Presence angezeigt wird.

In `config.json` lassen sich zudem `update_interval_seconds` (Mindestabstand zwischen
Updates) und `show_idle_presence` (Status anzeigen, wenn gerade nichts laeuft) anpassen.

## App als eigenstaendige Playtube.exe packen

Fuer die volle Taskmanager-/Audiomixer-Markierung wird die App als eigene .exe gebaut
(ein via `python main.py` gestarteter Prozess heisst in Windows immer "python.exe" -
nur eine kompilierte .exe mit eigenem Namen und eigener Versionsinfo kann das aendern):

```powershell
.venv\Scripts\pip install pyinstaller
powershell -ExecutionPolicy Bypass -File packaging\build.ps1
```

Ergebnis liegt danach unter `dist\Playtube\Playtube.exe`. Das Build-Skript benennt
zusaetzlich den QtWebEngine-Hilfsprozess (der den eigentlichen Ton ausgibt) zu
`PlaytubeHelper.exe` um, damit er im Taskmanager nicht als `QtWebEngineProcess`
auftaucht. Fuer eine vollstaendige Umbenennung inkl. Icon/Versionsinfo dieses
Hilfsprozesses (relevant fuer den Lautstaerkemixer) zusaetzlich
[rcedit](https://github.com/electron/rcedit/releases) als `packaging\rcedit.exe`
ablegen - das Skript nutzt es automatisch, wenn vorhanden. Ohne rcedit funktioniert
alles genauso, nur zeigt der Lautstaerkemixer fuer den Ton-Unterprozess je nach
Windows-Version eventuell weiterhin "QtWebEngineProcess" statt "Playtube" (rein
kosmetisch - Namensgebung von Chromium-Hilfsprozessen ist ein bekanntes,
Windows-versionsabhaengiges Verhalten, das selbst grosse Electron-Apps nur mit rcedit
o.ae. umgehen).

## Automatische Updates

Playtube prueft beim Start und danach alle `updates.check_interval_hours` Stunden
(Standard 6, in `config.json` einstellbar) die [GitHub Releases](https://github.com/fojadrachi/Playtube/releases)
des Projekts. Gibt es eine neuere Version als die laufende, fragt ein Dialog, ob sie
installiert werden soll:

- **Gepackte `Playtube.exe`**: laedt das `.zip`-Asset des Release herunter und ersetzt
  nach dem Beenden automatisch alle Dateien im Installationsordner, startet die App
  danach selbst neu.
- **Entwicklungsmodus** (`python main.py`): fuehrt `git pull` + `pip install -r
  requirements.txt` aus und startet den Python-Prozess neu.

Auto-Update laesst sich in `config.json` unter `updates.enabled` deaktivieren.

### Eine neue Version veroeffentlichen

```powershell
powershell -ExecutionPolicy Bypass -File packaging\release.ps1 -Version 1.1.0
```

Das Skript setzt die Versionsnummer, baut die `.exe`, packt sie als `.zip`, erstellt
Git-Tag `v1.1.0`, pusht zu `origin` (dein Repo unter
https://github.com/fojadrachi/Playtube) und legt automatisch ein GitHub Release mit
dem `.zip` als Anhang an - vorausgesetzt, entweder ist die
[GitHub CLI](https://cli.github.com/) installiert (`gh auth login` einmalig) oder die
Umgebungsvariable `GITHUB_TOKEN` enthaelt ein Personal Access Token mit `repo`-Rechten.
Ohne beides wird nur getaggt/gepusht und du legst das Release manuell unter
https://github.com/fojadrachi/Playtube/releases/new an (Zip-Datei aus `dist\` anhaengen).

Alle Nutzer mit einer laufenden Playtube-Installation bekommen die neue Version dann
automatisch angeboten.

## Hinweise

- **"Dieser Browser ist unter Umstaenden nicht sicher" beim Google-Login:** Playtube
  setzt einen echten Chrome-User-Agent inkl. passender `Sec-CH-UA`-Header, damit Google
  das eingebettete Chromium (QtWebEngine) nicht als unsicheres WebView erkennt. Sollte
  die Meldung dennoch erscheinen: alle Playtube-Fenster/-Prozesse schliessen und neu
  starten (die Header greifen erst ab dem naechsten Prozessstart), notfalls einmal den
  Profilordner `%APPDATA%\Playtube\webprofile` loeschen und neu anmelden.
- 4K/Premium-Videoqualitaet kann eingeschraenkt sein, da die Open-Source-Variante von
  QtWebEngine kein Widevine-DRM mitbringt (Standard-Qualitaeten funktionieren normal).
- Icon/Branding-Bilder liegen unter `assets/` und wurden mit `tools/generate_icon.py`
  erzeugt (Pillow) - bei Bedarf einfach eigenes `icon.ico`/`icon.png` dort ersetzen.
