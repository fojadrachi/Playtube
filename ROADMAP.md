# Playtube – Fahrplan

Stand: 15.11.2026 (PC 2.14.0, Android 1.9.0, Stream-Dock-Plugin 1.3.0). Reihenfolge = Vorschlag: erst kleine
Sachen mit viel Nutzen, dann Grundlagen für die großen Funktionen.

## Phase 1 – schnell gemacht ✅ (PC 2.13/2.14, Android 1.8/1.9, Plugin 1.2/1.3)

| Funktion | Wo | Notiz |
|---|---|---|
| Jam-Songs in die eigenen Lieblingssongs | PC, Android, (Web mit Login) | fertig (Android nur am laufenden Song) |
| Schlaf-Timer | PC, Android | Pausiert nach X Minuten oder am Songende; mit Ausblenden |
| Mini-Player | PC | fertig; Knöpfe in der Taskleisten-Vorschau fehlen noch |
| Verlauf | PC, Android | Lokal gespeichert, durchsuchbar. Grundlage für Wrapped und Empfehlungen |
| Mehr Stream-Dock-Tasten | Plugin | fertig: Gefällt mir nicht, Fortschritt am Cover, Schlaf-Timer, Mini-Player; Jam-Tasten fehlen noch |

## Phase 2 – mittel

| Funktion | Wo | Notiz |
|---|---|---|
| Songtexte | PC, Android | YouTube Music liefert Texte über InnerTube (oft ohne Zeitstempel); mitlaufend nur, wo es synchrone Texte gibt |
| Playlists bearbeiten | PC, Android | Anlegen, umbenennen, Songs hinzufügen/entfernen/umsortieren über InnerTube mit eigenem Login |
| Jam: Chat & Reaktionen | Jam-Server, PC, Android, Web | Nachrichten + Emoji-Reaktionen auf den laufenden Song; Host kann Nachrichten löschen |
| Android-Widget | Android | Startbildschirm-Widget + Sperrbildschirm-Steuerung (MediaSession) |
| Crossfade | PC, Android | Schwierig: Es gibt nur einen Player pro Tab. Braucht einen zweiten, versteckten Player für den nächsten Song |

## Phase 3 – groß

| Funktion | Wo | Notiz |
|---|---|---|
| Playtube Wrapped | PC, Android, Website | Monats- und Jahresrückblick aus dem Verlauf: Top-Songs, Künstler, Hörzeit, teilbares Bild |
| Empfehlungen | PC, Android | Eigene Startseite: YouTube-Music-Mixe plus Vorschläge aus dem Verlauf |
| Öffentliche Jam-Räume | Jam-Server, alle | Raumliste, beitreten ohne Code. Braucht Moderation (melden, kicken, sperren) und Spam-Schutz |
| Discord-Bot hört im Jam mit | neuer Bot + Jam-Server | Bot im Sprachkanal spielt den Jam ab. Am saubersten: Der Ton kommt vom Host-PC (Playtube-Ton/relay.rs), nicht von einem Server, der YouTube selbst lädt. Server-seitiges Laden verstößt gegen die YouTube-Nutzungsbedingungen, deshalb wurden große Musik-Bots abgeschaltet |

## Phase 4 – andere Systeme

| Funktion | Notiz |
|---|---|
| Linux / macOS | Slint und wry laufen dort. Neu bauen muss man alles, was an Windows hängt: WebView2-Cookies, Playtube-Ton, Named Pipe, Medientasten, Tray, Autostart, Setup und Updater |

## Offen / ungetestet

- Playlist-Cover im Stream Dock mit echtem Login und privaten Playlists.
- Playtube-Ansicht mit echtem Login (Bibliothek, Suche).
