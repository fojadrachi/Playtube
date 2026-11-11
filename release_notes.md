## Playtube 2.5.2

- **Tonausgabe wählbar:** In den Einstellungen unter Audioausgabe gibt es jetzt „Tonausgabe“:
  **Normal (beste Qualität)** – wie in früheren Versionen, Standard – oder **Für Discord-Streams** (Ton über
  Playtube, damit Zuschauer beim Fenster-Teilen mithören und Sonar „Playtube“ zeigt).
- **Discord-Modus klingt besser:** größerer Puffer und sanftes Ein-/Ausblenden statt Knacksern bei Aussetzern.
- **Behoben: ständiges Play/Pause**, sobald Einstellungen oder Jam offen waren, während Musik lief. Nicht gezeigte
  Tabs bleiben für die Edge-Engine jetzt „sichtbar“ – dadurch starten auch Jam-Songs zuverlässig, wenn gerade ein
  anderer Tab offen ist.

Neu in 2.5.1:

- **Behoben: Playtube reagierte nicht mehr auf Klicks** (z. B. Einstellungen ließen sich nicht scrollen), nachdem
  YouTube Music eine Seite geladen hatte. Ursache war das Auffrischen der Anmeldung aus 2.5.0, das auf die WebView
  wartete; Playtube liest die Anmeldung jetzt ohne zu warten.

Neu in 2.5.0:

- **Ton in Discord-Streams:** Teilst du in Discord das Playtube-Fenster, hören die Zuschauer jetzt auch die Musik.
  Bisher kam der Ton aus dem Hilfsprozess der Edge-Engine („msedgewebview2“), den Discord nicht dem Fenster
  zuordnet. Jetzt gibt Playtube den Ton selbst aus.
- **Mixer zeigen „Playtube“:** SteelSeries Sonar, der Windows-Lautstärkemixer & Co. zeigen jetzt Playtube statt
  MSEDGEWEBVIEW2 an.
- **Playlists und Bibliothek laden zuverlässiger:** Playtube meldet sich jetzt wie die Webseite selbst bei YouTube
  Music an (inkl. Markenkonten/Kanälen) und frischt die Anmeldung auf, sobald YouTube Music nach dem Start fertig
  geladen ist – vorher blieben die Playlists nach einem Neustart teils leer.
- **YouTube Music wahlweise im Original:** Unter Allgemein › „YouTube Music anzeigen als“ wählst du zwischen der
  Playtube-Ansicht und der Original-Seite.

**Installieren:** `Playtube-Setup-2.5.2.exe` laden und starten – ersetzt jede ältere Playtube. Playtube
aktualisiert sich auch von selbst.

---

## Playtube 2.5.2

- **Choose the sound output:** Settings › Audio output › "Sound output": **Normal (best quality)** – like earlier
  versions, default – or **For Discord streams** (sound through Playtube so viewers hear it when you share the
  window, and Sonar shows "Playtube").
- **Discord mode sounds better:** larger buffer and gentle fades instead of clicks on dropouts.
- **Fixed: constant play/pause** while the settings or jam tab was open and music was playing. Tabs that are not
  shown now stay "visible" for the Edge engine – jam songs also start reliably while another tab is open.

New in 2.5.1:

- **Fixed: Playtube stopped responding to clicks** (e.g. the settings could not be scrolled) after YouTube Music
  loaded a page. The sign-in refresh added in 2.5.0 waited on the WebView; Playtube now reads the sign-in without
  waiting.

New in 2.5.0:

- **Sound in Discord streams:** when you share the Playtube window in Discord, viewers now hear the music too.
  Previously the sound came from the Edge engine's helper process ("msedgewebview2"), which Discord does not link to
  the window. Playtube now outputs the sound itself.
- **Mixers show "Playtube":** SteelSeries Sonar, the Windows volume mixer and others now show Playtube instead of
  MSEDGEWEBVIEW2.
- **Playlists and library load more reliably:** Playtube now signs in to YouTube Music exactly like the website
  (including brand accounts/channels) and refreshes the sign-in once YouTube Music has finished loading after
  startup – previously playlists could stay empty after a restart.
- **YouTube Music in the original layout if you like:** General › "Show YouTube Music as" lets you choose between the
  Playtube view and the original page.

**Install:** run `Playtube-Setup-2.5.2.exe` – it replaces any older Playtube. Playtube also updates itself.
