## Playtube 2.16.0

- **Playtube für Linux:** Neben Windows gibt es Playtube jetzt auch für Linux (x86_64) – als
  `playtube_2.16.0_amd64.deb` für Debian/Ubuntu (ab 22.04) und als `Playtube-2.16.0-x86_64.AppImage` für andere
  Distributionen (braucht WebKitGTK 4.1). Es kann alles Wichtige: beide Tabs, Playtube-Ansicht, Jam (auch der
  Discord-Bot), Discord-Status, Steuerung vom Handy, Stream-Dock-Fernsteuerung (Unix-Socket), Tray, Autostart
  und Updates (AppImage). Daten liegen in `~/.config/Playtube`. Noch nicht unter Linux: globale Medientasten und
  eine Audioausgabe pro Tab.
- Windows: keine Änderungen.

Neu in 2.15.0:

- **Discord-Bot im Jam:** Ein Discord-Bot kann deinen Jam im Sprachkanal abspielen. Im Discord holst du ihn mit
  `/vc join` in deinen Sprachkanal und verbindest ihn mit `/jam connect <Code>`. Den Ton schickt Playtube von deinem
  PC an den Bot (der Bot lädt selbst nichts von YouTube); solange das läuft, kommt der Ton über Playtube.exe.
  Abschaltbar unter Einstellungen › Audioausgabe.

Neu in 2.14.0:

- **Verlauf:** In der Playtube-Ansicht gibt es links „Verlauf“ – alle Songs, die du gehört hast, nach Tagen
  sortiert und durchsuchbar (die Suchleiste sucht dort im Verlauf). Ein Song zählt, wenn er 30 Sekunden lief. Der
  Verlauf bleibt nur auf deinem PC; abschalten oder löschen unter Einstellungen › Allgemein.
- **Schlaf-Timer:** Mond-Knopf unten rechts in der Playtube-Ansicht oder im Tray-Menü: nach 15 bis 120 Minuten oder
  am Ende des Songs pausieren. Vorher wird die Musik 15 Sekunden lang leiser. Im Jam verlässt du dabei nur den Jam,
  für die anderen läuft er weiter.
- **Mini-Player:** Kleines Fenster mit Cover, Titel und Steuerung, das über allen anderen Fenstern bleibt. Öffnen
  über den Knopf in der Player-Leiste oder im Tray-Menü; mit dem Pfeil geht es zurück zum großen Fenster.
- **Fernsteuerung (Handy, Stream Dock):** neue Befehle `sleep_timer`, `history`, `play_video` und `mini_player`;
  der Status enthält jetzt `sleepTimer` (Restsekunden, -1 = Songende).

Neu in 2.13.0:

- **Jam-Songs zu deinen Lieblingssongs:** In der Jam-Ansicht hat der laufende Song und jeder Song in der
  Warteschlange ein Herz. Ein Klick legt ihn in deine eigenen YouTube-Music-Lieblingssongs (oder nimmt ihn wieder
  heraus) – mit deinem Login, unabhängig von den anderen. Schon gelikte Songs sind am ausgefüllten Herz zu erkennen.
  Dafür musst du in Playtube bei YouTube Music angemeldet sein.

Neu in 2.12.0:

- **Stream Dock: Playlist-Tasten zeigen das Cover.** Playtube liefert dem Stream-Dock-Plugin (ab 1.1.0) jetzt das Bild
  einer Playlist – auch bei privaten Playlists, denn es wird mit deinem Login geholt. Neuer Fernsteuerbefehl
  `playlist_cover`.

Neu in 2.11.0:

- **Tonausgabe „Automatisch“ (neuer Standard):** Sobald du in Discord in einem Sprachkanal bist – nur von dort aus
  kann man streamen –, schaltet Playtube von selbst auf „Für Discord-Streams“: Zuschauer hören die Musik, wenn du
  das Playtube-Fenster teilst. Verlässt du den Kanal, gibt es wieder die normale Tonausgabe in bester Qualität.
  Einstellbar unter Einstellungen › Audioausgabe › Tonausgabe (Normal / Für Discord-Streams / Automatisch).

Neu in 2.10.0:

- **Host abgeben:** Als Host kannst du die Rolle im Jam an jemand anderen übergeben – in der Liste „Dabei“ auf den
  Stern neben dem Namen klicken und zur Bestätigung noch einmal. Das geht auch auf Android und im Browser.
- **„Freunde einladen“ aufgeräumt:** QR-Code mittig, darunter Code, Link und „Link kopieren“.

Neu in 2.9.3:

- **Behoben: Der Jam-Code ragte über die Karte „Freunde einladen“ hinaus.** Code und „Link kopieren“ stehen jetzt
  unter dem QR-Code und passen auch in schmale Fenster.

Neu in 2.9.2:

- **Medientasten der Tastatur:** Play/Pause, Weiter, Zurück und Stopp auf der Tastatur steuern jetzt Playtube –
  auch wenn das Fenster im Hintergrund oder nur im Tray ist, im Jam auch den Jam. Playtube reagiert nur, wenn gerade
  etwas geladen ist, und andere Player bekommen die Tasten weiterhin. Abschaltbar unter Einstellungen › Allgemein.

Neu in 2.9.1:

- **Behoben: In der Suchleiste der Playtube-Ansicht ließ sich nichts eingeben.** Die Tastatur blieb bei der
  (unsichtbaren) YouTube-Music-Seite hängen; jetzt bekommt die Playtube-Ansicht den Fokus.
- **Nach einem Neustart wieder im Jam:** Wird Playtube beendet, neu gestartet (z. B. nach einem Update) oder
  stürzt ab, tritt es beim nächsten Start von selbst wieder dem letzten Jam bei – mit derselben Rolle und
  denselben Rechten wie vorher (der Host bleibt Host). Wer den Jam bewusst verlässt, bleibt draußen.
- **Diagnosedaten:** Video-IDs und Songtitel werden jetzt zuverlässig entfernt, auch in technischen Logzeilen.

Neu in 2.9.0:

- **Mit Windows starten:** Unter Einstellungen › Allgemein kannst du Playtube beim Hochfahren automatisch starten
  lassen (standardmäßig aus) – auf Wunsch nur im Tray, ohne dass ein Fenster aufgeht.
- **Discord-Status immer als „Playtube“:** Die Discord-Client-ID ist jetzt fest eingebaut, ebenso der Takt von
  15 s für Statusupdates (mehr erlaubt Discord nicht) – beide Felder sind aus den Einstellungen verschwunden.
- **Playtube-Ansicht ohne Browserleiste:** Zurück, Neu laden und Adressfeld gibt es nur noch bei YouTube und der
  Original-Seite; die Playtube-Ansicht hat ihre eigene Navigation und mehr Platz.

Neu in 2.8.0:

- **Kleine Updates ohne Setup:** Ab dieser Version lädt Playtube kleine Updates (gleiche Hauptversion) still im
  Hintergrund und spielt sie direkt ein, während es weiterläuft – kein Setup-Fenster, keine Unterbrechung. Die
  neue Version gilt ab dem nächsten Start; wer nicht warten will, klickt in den Einstellungen auf „Jetzt neu
  starten“. Abschaltbar unter Updates › „Kleine Updates automatisch im Hintergrund installieren“. Große Updates
  laufen wie bisher über das Setup.
- **Behoben: Discord zeigte im Jam „Unbekannter Titel“.** Fehlt der Titel in der YouTube-Music-Leiste, nimmt
  Playtube ihn jetzt aus der Wiedergabe-Info bzw. aus dem Jam.

Neu in 2.7.0:

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

**Installieren:** Windows: `Playtube-Setup-2.16.0.exe` laden und starten – ersetzt jede ältere Playtube. Linux:
`sudo apt install ./playtube_2.16.0_amd64.deb` oder das AppImage ausführbar machen und starten. Playtube
aktualisiert sich auch von selbst (Windows und AppImage).

---

## Playtube 2.16.0

- **Playtube for Linux:** besides Windows, Playtube is now available for Linux (x86_64) – as
  `playtube_2.16.0_amd64.deb` for Debian/Ubuntu (22.04 or newer) and as `Playtube-2.16.0-x86_64.AppImage` for other
  distributions (needs WebKitGTK 4.1). It does everything important: both tabs, the Playtube view, jam (including
  the Discord bot), Discord status, phone control, Stream Dock remote control (Unix socket), tray, autostart and
  updates (AppImage). Data lives in `~/.config/Playtube`. Not on Linux yet: global media keys and per-tab audio
  output.
- Windows: no changes.

New in 2.15.0:

- **Discord bot in the jam:** a Discord bot can play your jam in a voice channel. In Discord, bring it into your voice
  channel with `/vc join` and connect it with `/jam connect <code>`. Playtube sends the sound from your PC to the bot
  (the bot never downloads anything from YouTube itself); while it does, the sound comes through Playtube.exe. Can be
  turned off under Settings › Audio output.

New in 2.14.0:

- **History:** the Playtube view has a new "History" entry on the left – every song you played, grouped by day and
  searchable (the search bar searches your history there). A song counts once it has played for 30 seconds. The
  history stays on your PC; turn it off or clear it under Settings › General.
- **Sleep timer:** moon button at the bottom right of the Playtube view or in the tray menu: pause after 15 to 120
  minutes or at the end of the song. The music fades out over the last 15 seconds. In a jam you only leave the jam;
  it keeps going for everyone else.
- **Mini player:** a small always-on-top window with cover, title and controls. Open it from the player bar or the
  tray menu; the arrow brings back the big window.
- **Remote control (phone, Stream Dock):** new commands `sleep_timer`, `history`, `play_video` and `mini_player`;
  the status now includes `sleepTimer` (seconds left, -1 = end of song).

New in 2.13.0:

- **Jam songs to your liked songs:** in the jam view the current song and every song in the queue has a heart. One
  click adds it to your own YouTube Music liked songs (or removes it) – with your login, independently of the others.
  Songs you already liked show a filled heart. You need to be signed in to YouTube Music in Playtube for this.

New in 2.12.0:

- **Stream Dock: playlist keys show the cover.** Playtube now gives the Stream Dock plugin (1.1.0+) a playlist's
  artwork – private playlists included, since it is fetched with your login. New remote command `playlist_cover`.

New in 2.11.0:

- **Sound output "Automatic" (new default):** as soon as you are in a Discord voice channel – the only place you can
  stream from – Playtube switches to "For Discord streams" by itself, so viewers hear the music when you share the
  Playtube window. When you leave the channel, you get the normal best-quality output again. Settings › Audio
  output › Sound output (Normal / For Discord streams / Automatic).

New in 2.10.0:

- **Hand over host:** as host you can pass the role in a jam to someone else – click the star next to their name in
  the "Listening" list and once more to confirm. Works on Android and in the browser too.
- **"Invite friends" tidied up:** QR code centered, with the code, link and "Copy link" below it.

New in 2.9.3:

- **Fixed: the jam code stuck out of the "Invite friends" card.** The code and "Copy link" now sit below the QR
  code and fit narrow windows too.

New in 2.9.2:

- **Keyboard media keys:** play/pause, next, previous and stop on your keyboard now control Playtube – also when the
  window is in the background or only in the tray, and the jam while you are in one. Playtube only reacts while
  something is loaded, and other players still get the keys. Can be turned off under Settings › General.

New in 2.9.1:
- **Fixed: nothing could be typed into the search bar of the Playtube view.** The keyboard stayed with the
  (hidden) YouTube Music page; the Playtube view now gets the focus.
- **Back in the jam after a restart:** if Playtube is closed, restarted (e.g. after an update) or crashes, it
  rejoins the last jam on the next start – with the same role and rights as before (the host stays host).
  Leaving the jam on purpose keeps you out.
- **Diagnostics:** video IDs and song titles are now reliably removed, also from technical log lines.

New in 2.9.0:

- **Start with Windows:** under Settings › General, Playtube can start automatically when you sign in (off by
  default) – optionally in the tray only, without opening a window.
- **Discord status always shows "Playtube":** the Discord client ID and the 15 s update rate (Discord's limit)
  are now built in; both settings fields are gone.
- **Playtube view without browser bar:** back, reload and the address field are only shown for YouTube and the
  original page; the Playtube view has its own navigation and more room.

New in 2.8.0:

- **Small updates without setup:** from this version on, Playtube downloads small updates (same major version)
  quietly in the background and applies them while it keeps running – no setup window, no interruption. The new
  version takes effect on the next start; if you don't want to wait, click "Restart now" in the settings. Can be
  turned off under Updates › "Install small updates automatically in the background". Major updates still use
  the setup.
- **Fixed: Discord showed "Unknown title" during a jam.** If the YouTube Music player bar has no title, Playtube
  now takes it from the media session or from the jam.

New in 2.7.0:

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

**Install:** Windows: run `Playtube-Setup-2.16.0.exe` – it replaces any older Playtube. Linux:
`sudo apt install ./playtube_2.16.0_amd64.deb` or make the AppImage executable and run it. Playtube also updates
itself (Windows and AppImage).
