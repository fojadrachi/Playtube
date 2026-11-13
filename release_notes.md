## Playtube 2.7.0

- **Jam deutlich genauer synchron:** Kleine Abweichungen gleicht Playtube jetzt unhörbar aus, indem es kurz eine
  Spur schneller oder langsamer abspielt (die Tonhöhe bleibt). Gesprungen wird erst ab 1 s statt 1,5 s – und mit
  etwas Vorhalt, damit man danach nicht hinterherhängt. Im Test lag Playtube dauerhaft etwa 0,06 s am Jam.
- **Jam-Warteschlange in der Playtube-Ansicht:** Während eines Jams zeigt die Warteschlange rechts die Jam-Liste
  (zuletzt gespielt, läuft gerade, als Nächstes) statt der von YouTube Music, die sich bei jedem neuen Song
  komplett erneuerte. Mit Steuerrecht springst du per Klick zu einem Song.
- **Jeder sieht, wer was hinzugefügt hat:** „von Name“ steht an jedem Song in der Liste.
- „Jetzt senden“ (Diagnose) schickt immer einen Bericht, auch ohne neue Logzeilen.

Neu in 2.6.2:

- **Behoben: Das Handy fand den PC manchmal nicht von selbst.** Die automatische Suche im WLAN startete nicht,
  wenn ihr Port gerade noch belegt war (z. B. kurz nach einem Update oder Neustart der Fernsteuerung). Playtube
  teilt sich den Port jetzt und versucht es weiter, bis die Suche läuft.

Neu in 2.6.1:

- **Behoben: „+ Jam“-Knopf flackerte**, sobald die Maus darauf stand (auch der Abspielen-Knopf an Zeilen und
  Karten). Zeile bzw. Karte bleiben jetzt markiert, solange die Maus auf einem ihrer Knöpfe ist; im Jam ist der
  Platz für „+ Jam“ fest reserviert, damit die Songlänge beim Darüberfahren nicht springt.
- **Neu: Diagnosedaten** (Einstellungen › Diagnose): Playtube kann gefilterte Fehler- und Logzeilen (ohne Cookies,
  Tokens, E-Mails, Video-IDs oder Songtitel) mit Version und einer zufälligen Installations-Kennung an den
  Playtube-Dienst schicken, damit Fehler schneller gefunden werden. Standardmäßig an, jederzeit abschaltbar.

Neu in 2.6.0:

- **„+ Jam“ in der Playtube-Ansicht:** Während eines Jams erscheint beim Darüberfahren an jedem Song, Album und
  jeder Playlist ein „+ Jam“-Knopf – auch bei deinen eigenen, privaten Playlists. Oben auf Album- und
  Playlist-Seiten fügt „+ Jam“ alle Songs auf einmal hinzu.
- **Playlists und Alben im Jam:** Im Jam-Tab kannst du jetzt auch Playlist- und Album-Links einfügen; alle Songs
  (bis 200) landen in der gemeinsamen Warteschlange.
- **Songs laufen automatisch weiter:** Der Jam-Server kennt die Länge jedes Songs und startet den nächsten, sobald
  einer zu Ende ist – auch wenn gerade niemand zuhört.

Alles Weitere wie in 2.5.2 (Tonausgabe Normal/Discord, Fixes).

**Installieren:** `Playtube-Setup-2.7.0.exe` laden und starten – ersetzt jede ältere Playtube. Playtube
aktualisiert sich auch von selbst.

---

## Playtube 2.7.0

- **Jam stays much more closely in sync:** small differences are now evened out inaudibly by playing a touch
  faster or slower for a moment (pitch stays the same). Playtube only jumps from 1 s instead of 1.5 s – with a
  little lead so it doesn't lag behind afterwards. In testing Playtube stayed about 0.06 s from the jam.
- **Jam queue in the Playtube view:** during a jam the queue on the right shows the jam list (recently played,
  now playing, up next) instead of the YouTube Music queue, which was completely replaced with every new song.
  With control rights, click a song to jump to it.
- **Everyone sees who added what:** "by name" is shown on every song in the list.
- "Send now" (diagnostics) always sends a report, even without new log lines.

New in 2.6.2:

- **Fixed: the phone sometimes didn't find the PC on its own.** Automatic discovery on the local network didn't
  start while its port was still in use (e.g. right after an update or a restart of the remote control).
  Playtube now shares the port and keeps trying until discovery runs.

New in 2.6.1:

- **Fixed: the "+ Jam" button flickered** while the mouse was on it (the play button on rows and cards too). The
  row or card now stays highlighted while the mouse is on one of its buttons; in a jam the space for "+ Jam" is
  reserved so the song length no longer jumps on hover.
- **New: diagnostics** (Settings › Diagnostics): Playtube can send filtered error and log lines (no cookies,
  tokens, e-mails, video IDs or song titles) with the version and a random installation ID to the Playtube
  service so bugs are found faster. On by default, can be turned off any time.

New in 2.6.0:

- **"+ Jam" in the Playtube view:** while you are in a jam, hovering any song, album or playlist shows a "+ Jam"
  button – including your own private playlists. At the top of album and playlist pages, "+ Jam" adds all songs
  at once.
- **Playlists and albums in the jam:** the jam tab now also accepts playlist and album links; all songs (up to
  200) go into the shared queue.
- **Songs continue automatically:** the jam server knows each song's length and starts the next one when a song
  ends – even when nobody is listening.

Everything else as in 2.5.2 (sound output Normal/Discord, fixes).

**Install:** run `Playtube-Setup-2.7.0.exe` – it replaces any older Playtube. Playtube also updates itself.
