# Playtube

Eigenständige Desktop-App für YouTube & YouTube Music (kein Browser-Fenster, keine
Erweiterung) mit:

- **Zwei Tabs** – YouTube und YouTube Music laufen parallel, Musik spielt im
  Hintergrund weiter wenn du zu Videos wechselst.
- **Login/Premium** – eigenes, persistentes Profil (`%APPDATA%\Playtube`, einmal bei
  Google anmelden reicht für beide Dienste.
- **Discord Rich Presence** – zeigt Titel, Kanal/Interpret, Fortschrittsbalken und
  einen Link-Button in deinem Discord-Profil, sobald etwas läuft.
- **System-Tray** – Fenster schliessen minimiert nur (Musik läuft weiter), Rechtsklick
  aufs Tray-Icon zum Beenden, Play/Pause/Skip direkt aus dem Menü.
- **Eigener Name** – erscheint als "Playtube" im Taskmanager, Fenstertitel, Alt-Tab
  und (nach dem Packaging, siehe unten) im Lautstärkemixer statt als "python".

## Installation (Windows)

Lade auf der [Releases-Seite](https://github.com/fojadrachi/Playtube/releases)
`Playtube-Setup-vX.Y.Z.exe` herunter und starte sie. Der Installer

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
Windows-Version eventüll weiterhin "QtWebEngineProcess" statt "Playtube" (rein
kosmetisch - Namensgebung von Chromium-Hilfsprozessen ist ein bekanntes,
Windows-versionsabhängiges Verhalten, das selbst grosse Electron-Apps nur mit rcedit
o.ä. umgehen).

## Automatische Updates

Playtube prüft beim Start und danach alle `updates.check_interval_hours` Stunden
(Standard 6, im **Einstellungen-Tab** oder in `config.json` einstellbar) die
[GitHub Releases](https://github.com/fojadrachi/Playtube/releases) des Projekts. Im
Einstellungen-Tab gibt es zusätzlich einen "Jetzt nach Updates suchen"-Button mit
Status-Anzeige und Fortschrittsbalken fü den Download. Gibt es eine neuere Version,
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
[Releases-Seite](https://github.com/fojadrachi/Playtube/releases)) lässt sich also
einfach per Doppelklick installieren, ohne dass Playtube selbst etwas herunterladen
muss.

### Eine neue Version veröffentlichen

```powershell
powershell -ExecutionPolicy Bypass -File packaging\release.ps1 -Version 1.1.0
```

Das Skript setzt die Versionsnummer, committet, erstellt Git-Tag `v1.1.0` und pusht zu
`origin` (dein Repo unter https://github.com/fojadrachi/Playtube). Der gepushte Tag
löst automatisch die GitHub-Actions-Pipeline
([.github/workflows/release.yml](.github/workflows/release.yml)) aus, die **sowohl
eine Windows- als auch eine Linux-Version baut** und beide als Assets an einem GitHub
Release veröffentlicht - Fortschritt unter
https://github.com/fojadrachi/Playtube/actions.

Für Windows entstehen dabei drei Dateien: `Playtube-Setup-vX.Y.Z.exe` (Installer, für
Neueinsteiger und volle Updates), `Playtube-vX.Y.Z-win64.zip` (portabel) und das kleine
`...-pyside<Version>-patch.play` (schnelles Update). Der Installer bekommt seine
Versionsnummer aus `playtube/__init__.py` (das `release.ps1` vor dem Tag setzt).

Alle Nutzer mit einer laufenden Playtube-Installation (Windows oder Linux) bekommen die
neue Version danach automatisch angeboten.

## Native Linux-Version

Playtube läuft genauso unter Linux (gleicher Code, gleiches PySide6/QtWebEngine) und
wird bei jedem Release automatisch als `Playtube-vX.Y.Z-linux-x86_64.tar.gz` unter
https://github.com/fojadrachi/Playtube/releases mitgebaut.

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
`sudo apt install libxkbcommon0 libegl1 libnss3 libxcomposite1 libxdamage1 libxrandr2 libgbm1 libasound2t64 libatk-bridge2.0-0 libcups2` (siehe auch die vollstaendige Liste in
[.github/workflows/release.yml](.github/workflows/release.yml)).

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
