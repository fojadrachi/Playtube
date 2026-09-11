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


def build_presence_payload(info: dict[str, Any], session_start: int) -> dict[str, Any]:
    """Baut das update()-Payload fuer pypresence aus den vom Browser-Tab gelieferten
    Medien-Informationen."""
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
    }

    duration = info.get("duration") or 0
    current_time = info.get("currentTime") or 0
    # IMMER start/end mitschicken, unabhaengig vom playing-Status - nicht nur wenn
    # playing=true. Discord ersetzt "timestamps" bei einem SET_ACTIVITY-Update ohne
    # diese Felder offenbar nicht sauber, sondern behaelt intern die zuletzt bekannten
    # Werte bei ("stackt"). Waehrend eines Songwechsels ist "playing" durch das kurze
    # Neuladen des <video>-Elements oft fuer 1-2 Polls faelschlich false - wurden
    # start/end dann weggelassen, blieb Discords alte (viel zu weit zurueckliegende)
    # Zeit einfach stehen, bis irgendwann wieder echte Werte kamen. Ein pausierter
    # Titel zeigt so einfach einen eingefrorenen Fortschrittsbalken statt gar keinen.
    start_ts = int(time.time() - current_time)
    payload["start"] = start_ts
    if duration and duration > 0:
        payload["end"] = start_ts + int(duration)

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

    def submit_media_info(self, info: dict[str, Any] | None) -> None:
        """Neuester bekannter Zustand (None = nichts spielt / idle)."""
        self._replace_queue(info if info is not None else _IDLE_SENTINEL)

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
            if os.environ.get("PLAYTUBE_DEBUG"):
                print("[discord-rpc] verbunden", flush=True)
        except Exception as exc:
            self._connected = False
            if os.environ.get("PLAYTUBE_DEBUG"):
                print(f"[discord-rpc] Verbindung fehlgeschlagen: {exc!r}", flush=True)

    def _send(self, item) -> None:
        if self._presence is None:
            return
        try:
            if item is _IDLE_SENTINEL:
                if self._show_idle:
                    payload = build_idle_payload(self._session_start)
                    self._presence.update(**payload)
                else:
                    payload = None
                    self._presence.clear()
            else:
                payload = build_presence_payload(item, self._session_start)
                self._presence.update(**payload)
            if os.environ.get("PLAYTUBE_DEBUG"):
                print(f"[discord-rpc] gesendet: {payload!r}", flush=True)
        except Exception as exc:
            # Discord evtl. geschlossen worden -> beim naechsten Mal neu verbinden.
            self._connected = False
            if os.environ.get("PLAYTUBE_DEBUG"):
                print(f"[discord-rpc] Senden fehlgeschlagen: {exc!r}", flush=True)

    def _cleanup(self) -> None:
        if self._presence is not None:
            try:
                self._presence.clear()
                self._presence.close()
            except Exception:
                pass


class _Sentinel:
    __slots__ = ()


_IDLE_SENTINEL = _Sentinel()
_STOP_SENTINEL = _Sentinel()
