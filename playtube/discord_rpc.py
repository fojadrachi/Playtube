"""Discord Rich Presence Integration.

Laeuft in einem eigenen Thread (pypresence macht blockierende IPC-Aufrufe), damit die
Oberflaeche nie haengt, egal ob Discord laeuft, gerade startet oder gar nicht
installiert ist. Verbindungsfehler werden abgefangen und in Intervallen erneut
versucht, ohne die App zu beeintraechtigen.
"""
from __future__ import annotations

import os
import queue
import re
import time
from typing import Any

from PySide6.QtCore import QThread

from .config import APP_NAME
from .debug_log import log_line

try:
    from pypresence.types import ActivityType
except ImportError:  # pypresence fehlt -> Discord-Feature bleibt einfach aus
    ActivityType = None

# Asset-Keys, die (optional) unter discord.com/developers/applications -> Rich
# Presence -> Art Assets hochgeladen werden koennen. Fehlen sie, zeigt Discord
# einfach kein Bild an - es gibt keinen Fehler.
ASSET_YOUTUBE = "youtube_logo"
ASSET_MUSIC = "music_logo"
ASSET_DEFAULT = "logo"
ASSET_PLAY = "play_icon"
ASSET_PAUSE = "pause_icon"

_RECONNECT_DELAY_SECONDS = 10


def _truncate(text: str | None, limit: int = 128, fallback: str = "") -> str:
    text = (text or fallback).strip()
    if not text:
        text = fallback
    if len(text) > limit:
        text = text[: limit - 1].rstrip() + "…"
    return text


_THUMBNAIL_SIZE_RE = re.compile(r"=w\d+-h\d+(-[a-z0-9-]*)?$", re.IGNORECASE)


def _upsize_thumbnail(url: str | None) -> str | None:
    """YouTube-Music-Player-Bar-Thumbnails kommen sehr klein (z.B. '=w60-h60-l90-rj').
    Fuer eine scharfe Darstellung in Discord die Groesse im URL-Suffix hochsetzen."""
    if not url:
        return url
    return _THUMBNAIL_SIZE_RE.sub("=w544-h544-l90-rj", url)


def build_presence_payload(
    info: dict[str, Any], session_start: int, start_ts: int, end_ts: int | None
) -> dict[str, Any]:
    """Baut das update()-Payload fuer pypresence aus den vom Browser-Tab gelieferten
    Medien-Informationen. start_ts/end_ts werden vom DiscordRPCWorker mitgegeben (siehe
    dort _track_timestamps) statt hier direkt aus info["currentTime"] berechnet zu
    werden."""
    is_music = bool(info.get("isMusic"))
    playing = bool(info.get("playing"))
    title = _truncate(info.get("title"), fallback="Unbekannter Titel")
    default_state = "YouTube Music" if is_music else "YouTube"
    state = _truncate(info.get("subtitle"), fallback=default_state)

    thumbnail = info.get("thumbnail")
    if isinstance(thumbnail, str) and thumbnail.startswith("http"):
        # Discord akzeptiert fuer large_image auch direkte externe Bild-URLs (nicht nur
        # vorab hochgeladene Asset-Keys) - so zeigt Discord das echte Video-/Cover-Bild
        # statt eines statischen Logos.
        large_image = _upsize_thumbnail(thumbnail) if is_music else thumbnail
    else:
        large_image = ASSET_MUSIC if is_music else ASSET_YOUTUBE

    payload: dict[str, Any] = {
        # LISTENING = Hoert, WATCHING = Schaut - statt des Default-Typs PLAYING
        # (Spielt). Muss ein ActivityType-Enum-Member sein, kein rohes int - pypresence
        # ruft intern .value darauf auf.
        "activity_type": ActivityType.LISTENING if is_music else ActivityType.WATCHING,
        "instance": False,
        "details": title,
        "state": state,
        "large_image": large_image,
        "large_text": "YouTube Music" if is_music else "YouTube",
        "small_image": ASSET_PLAY if playing else ASSET_PAUSE,
        "small_text": "Spielt" if playing else "Pausiert",
        "start": start_ts,
    }
    if end_ts:
        payload["end"] = end_ts

    url = info.get("url")
    if url and isinstance(url, str) and url.startswith("http"):
        label = "In YouTube Music öffnen" if is_music else "Auf YouTube ansehen"
        payload["buttons"] = [{"label": label, "url": url}]

    return payload


def build_idle_payload(session_start: int) -> dict[str, Any]:
    return {
        "details": f"Bei {APP_NAME}",
        "state": "Stöbert gerade",
        "large_image": ASSET_DEFAULT,
        "large_text": APP_NAME,
        "start": session_start,
    }


SETTINGS_DETAILS = "In den Einstellungen"
SETTINGS_DEFAULT_STATE = "Schaut sich die Einstellungen an"


def build_settings_payload(session_start: int, activity: str | None) -> dict[str, Any]:
    """Presence, solange der Einstellungen-Tab offen ist. `activity` beschreibt, was
    gerade bearbeitet wird (z.B. "Bearbeitet: Discord Rich Presence › Client-ID") - es
    wird bewusst nur der NAME des Feldes uebergeben, nie sein Inhalt (sonst wuerde
    z.B. die eingetragene Client-ID oeffentlich im Discord-Profil stehen)."""
    return {
        "details": SETTINGS_DETAILS,
        "state": _truncate(activity, fallback=SETTINGS_DEFAULT_STATE),
        "large_image": ASSET_DEFAULT,
        "large_text": APP_NAME,
        "start": session_start,
    }


class DiscordRPCWorker(QThread):
    """Eigener Thread, der die Verbindung zu Discord haelt und Presence-Updates
    debounced (max. 1 Update pro `interval` Sekunden) versendet."""

    def __init__(self, client_id: str, interval: float, show_idle: bool, parent=None):
        super().__init__(parent)
        self._client_id = client_id
        # Discord ignoriert/verwirft Rich-Presence-Updates stillschweigend, wenn sie
        # haeufiger als ca. alle 15 Sekunden gesendet werden (offizielle Grenze fuer
        # SET_ACTIVITY). Wird diese Grenze unterschritten, landet zwar technisch jedes
        # Update im Code, aber Discord uebernimmt nur einen Teil davon - nach aussen
        # sieht das wie "eingefrorene" Bilder/Zeiten aus (Bild wechselt nicht, Fortschritt
        # "stackt" beim Songwechsel), weil zufaellig immer wieder ein veraltetes Update
        # durchkommt statt des aktuellen. Deshalb hartes Minimum von 15s, unabhaengig
        # davon, was in der Konfiguration steht.
        self._interval = max(15.0, float(interval))
        self._show_idle = show_idle
        self._queue: "queue.Queue[dict | None | object]" = queue.Queue(maxsize=1)
        self._running = True
        self._connected = False
        self._presence = None
        self._session_start = int(time.time())
        # Anker fuer die Fortschrittsanzeige: wird NICHT mehr bei jedem Send aus
        # video.currentTime neu berechnet (siehe _track_timestamps).
        self._track_key: tuple | None = None
        self._track_start_ts: int | None = None

    def submit_media_info(self, info: dict[str, Any] | None) -> None:
        """Neuester bekannter Zustand (None = nichts spielt / idle)."""
        self._replace_queue(info if info is not None else _IDLE_SENTINEL)

    def submit_settings_activity(self, activity: str | None) -> None:
        """Nutzer ist im Einstellungen-Tab; `activity` = was gerade bearbeitet wird
        (None = nur "schaut sich die Einstellungen an")."""
        self._replace_queue(_SettingsActivity(activity))

    def _replace_queue(self, item) -> None:
        try:
            self._queue.get_nowait()
        except queue.Empty:
            pass
        try:
            self._queue.put_nowait(item)
        except queue.Full:
            pass

    def stop(self) -> None:
        self._running = False
        self._replace_queue(_STOP_SENTINEL)

    def run(self) -> None:
        last_sent = 0.0
        while self._running:
            try:
                item = self._queue.get()
            except Exception:
                break

            if item is _STOP_SENTINEL or not self._running:
                break

            wait = self._interval - (time.time() - last_sent)
            if wait > 0:
                time.sleep(wait)
                # Waehrend des Wartens evtl. neuere Daten uebernehmen (debounce).
                try:
                    while True:
                        item = self._queue.get_nowait()
                except queue.Empty:
                    pass

            if item is _STOP_SENTINEL or not self._running:
                break

            self._ensure_connected()
            if self._connected:
                self._send(item)
                last_sent = time.time()
            else:
                # Nicht verbunden -> nicht sinnlos oft versuchen.
                time.sleep(_RECONNECT_DELAY_SECONDS)

        self._cleanup()

    def _ensure_connected(self) -> None:
        if self._connected:
            return
        try:
            from pypresence import Presence

            self._presence = Presence(self._client_id)
            self._presence.connect()
            self._connected = True
            log_line("[connect] verbunden")
            if os.environ.get("PLAYTUBE_DEBUG"):
                print("[discord-rpc] verbunden", flush=True)
        except Exception as exc:
            self._connected = False
            if os.environ.get("PLAYTUBE_DEBUG"):
                print(f"[discord-rpc] Verbindung fehlgeschlagen: {exc!r}", flush=True)

    def _track_timestamps(self, info: dict[str, Any]) -> tuple[int, int | None]:
        """Liefert (start_ts, end_ts) fuer die Fortschrittsanzeige. Der Anker wird nur
        NEU gesetzt, wenn sich Titel/Untertitel aendern (= neuer Track) - nicht bei
        jedem Send aus video.currentTime neu berechnet. Grund: YouTube Music spielt
        beim Songwechsel oft nahtlos (gapless) aus einem durchgehenden Buffer weiter -
        video.currentTime springt dabei nicht zuverlaessig auf 0 zurueck, sondern kann
        einfach vom vorherigen Titel weiterzaehlen. Wuerde man start_ts jedes Mal aus
        currentTime neu ableiten, "stackt" die in Discord angezeigte Zeit ueber mehrere
        Songs hinweg, obwohl Titel/Bild laengst gewechselt haben."""
        title = info.get("title") or ""
        subtitle = info.get("subtitle") or ""
        is_music = bool(info.get("isMusic"))
        key = (is_music, title, subtitle)
        duration = info.get("duration") or 0
        current_time = info.get("currentTime") or 0
        now = time.time()

        if key != self._track_key:
            old_key = self._track_key
            self._track_key = key
            # current_time nur als grobe Anfangs-Schaetzung verwenden (z.B. Programm
            # startet waehrend ein Titel schon laeuft) - plausibilisiert, damit ein
            # verlaesslicher Wert genau EINMAL beim Trackwechsel einfriert und danach
            # rein ueber die Systemzeit weiterlaeuft statt ueber currentTime.
            offset = current_time if (duration <= 0 or 0 <= current_time <= duration) else 0
            self._track_start_ts = int(now - offset)
            log_line(f"[track-change] alt={old_key!r} neu={key!r} offset={offset:.1f}s")

        start_ts = self._track_start_ts if self._track_start_ts is not None else int(now)
        end_ts = start_ts + int(duration) if duration and duration > 0 else None
        return start_ts, end_ts

    def _send(self, item) -> None:
        if self._presence is None:
            return
        try:
            if item is _IDLE_SENTINEL:
                # Naechster echter Track soll wieder einen frischen Zeit-Anker bekommen.
                self._track_key = None
                self._track_start_ts = None
                if self._show_idle:
                    payload = build_idle_payload(self._session_start)
                    self._presence.update(**payload)
                else:
                    payload = None
                    self._presence.clear()
            elif isinstance(item, _SettingsActivity):
                # Wie beim Leerlauf: der naechste echte Track bekommt einen frischen
                # Zeit-Anker.
                self._track_key = None
                self._track_start_ts = None
                payload = build_settings_payload(self._session_start, item.activity)
                self._presence.update(**payload)
                log_line("[send-settings] state=%r" % (payload["state"],))
            else:
                start_ts, end_ts = self._track_timestamps(item)
                payload = build_presence_payload(item, self._session_start, start_ts, end_ts)
                self._presence.update(**payload)
                log_line(
                    "[send] title=%r url=%r thumbnail=%r large_image=%r start=%s end=%s"
                    % (
                        item.get("title"),
                        item.get("url"),
                        item.get("thumbnail"),
                        payload.get("large_image"),
                        start_ts,
                        end_ts,
                    )
                )
            if os.environ.get("PLAYTUBE_DEBUG"):
                print(f"[discord-rpc] gesendet: {payload!r}", flush=True)
        except Exception as exc:
            # Discord evtl. geschlossen worden -> beim naechsten Mal neu verbinden.
            self._connected = False
            log_line(f"[send-error] {exc!r}")
            if os.environ.get("PLAYTUBE_DEBUG"):
                print(f"[discord-rpc] Senden fehlgeschlagen: {exc!r}", flush=True)

    def _cleanup(self) -> None:
        if self._presence is not None:
            try:
                self._presence.clear()
                self._presence.close()
            except Exception:
                pass


class _SettingsActivity:
    """Queue-Eintrag: Nutzer ist in den Einstellungen und bearbeitet `activity`."""

    __slots__ = ("activity",)

    def __init__(self, activity: str | None) -> None:
        self.activity = activity


class _Sentinel:
    __slots__ = ()


_IDLE_SENTINEL = _Sentinel()
_STOP_SENTINEL = _Sentinel()
