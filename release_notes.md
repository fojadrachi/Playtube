## Playtube 2.3.0

- **Jam mit YouTube Music:** Während eines Jams kannst du im YouTube-Music-Tab normal weiterstöbern (Suche,
  Bibliothek, Playlists), ohne dass der Jam-Song neu lädt. Tippst du dort einen Song an, kommt er wie bei
  Spotify Jam in die gemeinsame Warteschlange. Videos aus dem YouTube-Tab fügst du weiter per Knopf hinzu.
- **Zuverlässigeres Synchronisieren:** Der Jam liest Position und Status direkt vom Player (YouTube Music hat
  teils mehrere Video-Elemente) und steuert ihn über die Player-API.
- **Jam verbindet auch hinter zickigen Routern:** Findet Windows `jam.fojadrachi.de` nicht (manche Router merken
  sich bei neuen Adressen eine Zeit lang „gibt es nicht“), fragt Playtube die Adresse verschlüsselt bei
  Cloudflare-DNS nach und verbindet trotzdem.

**Installieren:** `Playtube-Setup-2.3.0.exe` laden und starten – ersetzt jede ältere Playtube. Playtube
aktualisiert sich auch von selbst.

---

## Playtube 2.3.0

- **Jam with YouTube Music:** during a jam you can keep browsing the YouTube Music tab (search, library,
  playlists) without the jam song reloading. Tap a song there and it joins the shared queue, like Spotify Jam.
- **More reliable sync:** the jam reads position and state straight from the player and controls it via the
  player API.
- **Jam connects behind stubborn routers too:** if Windows can't resolve `jam.fojadrachi.de`, Playtube looks it
  up via encrypted Cloudflare DNS and connects anyway.

**Install:** run `Playtube-Setup-2.3.0.exe` – it replaces any older Playtube. Playtube also updates itself.
