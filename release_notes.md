## Playtube 2.5.1

- **Behoben: Playtube reagierte nicht mehr auf Klicks** (z. B. Einstellungen ließen sich nicht scrollen), nachdem
  YouTube Music eine Seite geladen hatte. Ursache war das Auffrischen der Anmeldung aus 2.5.0, das auf die WebView
  wartete; Playtube liest die Anmeldung jetzt ohne zu warten.

Neu in 2.5.0:

- **Ton in Discord-Streams:** Teilst du in Discord das Playtube-Fenster, hören die Zuschauer jetzt auch die Musik.
  Bisher kam der Ton aus dem Hilfsprozess der Edge-Engine („msedgewebview2“), den Discord nicht dem Fenster
  zuordnet. Jetzt gibt Playtube den Ton selbst aus.
- **Mixer zeigen „Playtube“:** SteelSeries Sonar, der Windows-Lautstärkemixer & Co. zeigen jetzt Playtube statt
  MSEDGEWEBVIEW2 an.
- Einstellbar unter Audioausgabe: „Ton über Playtube ausgeben“ (standardmäßig an). Die Geräteauswahl pro Tab gilt
  weiterhin.
- **Playlists und Bibliothek laden zuverlässiger:** Playtube meldet sich jetzt wie die Webseite selbst bei YouTube
  Music an (inkl. Markenkonten/Kanälen) und frischt die Anmeldung auf, sobald YouTube Music nach dem Start fertig
  geladen ist – vorher blieben die Playlists nach einem Neustart teils leer.
- **YouTube Music wahlweise im Original:** Unter Allgemein › „YouTube Music anzeigen als“ wählst du zwischen der
  Playtube-Ansicht und der Original-Seite.

**Installieren:** `Playtube-Setup-2.5.1.exe` laden und starten – ersetzt jede ältere Playtube. Playtube
aktualisiert sich auch von selbst.

---

## Playtube 2.5.1

- **Fixed: Playtube stopped responding to clicks** (e.g. the settings could not be scrolled) after YouTube Music
  loaded a page. The sign-in refresh added in 2.5.0 waited on the WebView; Playtube now reads the sign-in without
  waiting.

New in 2.5.0:

- **Sound in Discord streams:** when you share the Playtube window in Discord, viewers now hear the music too.
  Previously the sound came from the Edge engine's helper process ("msedgewebview2"), which Discord does not link to
  the window. Playtube now outputs the sound itself.
- **Mixers show "Playtube":** SteelSeries Sonar, the Windows volume mixer and others now show Playtube instead of
  MSEDGEWEBVIEW2.
- Configurable under Audio output: "Play sound through Playtube" (on by default). The per-tab device choice still
  applies.
- **Playlists and library load more reliably:** Playtube now signs in to YouTube Music exactly like the website
  (including brand accounts/channels) and refreshes the sign-in once YouTube Music has finished loading after
  startup – previously playlists could stay empty after a restart.
- **YouTube Music in the original layout if you like:** General › "Show YouTube Music as" lets you choose between the
  Playtube view and the original page.

**Install:** run `Playtube-Setup-2.5.1.exe` – it replaces any older Playtube. Playtube also updates itself.
