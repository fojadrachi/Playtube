"""Lokale Fernsteuerung fuer externe Tools - z.B. das Stream-Dock-Plugin
(com.fojadrachi.playtube.sdPlugin).

Transport: ein QLocalServer (Named Pipe unter Windows, z.B. \\\\.\\pipe\\Playtube.Remote.Playtube)
- nur lokal erreichbar, kein offener Netzwerk-Port. Getrennt vom Einzelinstanz-Pipe
(single_instance.py), damit ein Steuerbefehl nie das Fenster nach vorne holt und aeltere
Playtube-Versionen nicht faelschlich reagieren.

Protokoll (eine JSON-Nachricht pro Zeile, UTF-8, "\\n"-getrennt, Verbindung bleibt offen):

    -> {"id": 1, "cmd": "play_pause", "target": "auto"}
    <- {"id": 1, "ok": true}
    -> {"id": 2, "cmd": "status"}
    <- {"id": 2, "ok": true, "state": {...}}
    <- {"id": 3, "ok": false, "error": "..."}

Playlists (nur YouTube Music):

    -> {"id": 4, "cmd": "list_playlists"}
    <- {"id": 4, "ok": true, "playlists": [{"id": "LM", "title": "Titel, die ich mag"}, ...]}
    -> {"id": 5, "cmd": "play_playlist", "playlist": "PL..."}

`cmd` stammt aus einer festen Whitelist (APP_COMMANDS + media_control.MEDIA_COMMANDS),
`target` ist "auto" | "youtube" | "music", `value` eine Zahl mit befehlsabhaengigem
Wertebereich. Beliebiges JavaScript oder URLs lassen sich bewusst NICHT senden.
"""
from __future__ import annotations

import json
import math
import re
from dataclasses import dataclass
from typing import Any, Callable, Protocol

from PySide6.QtCore import QObject
from PySide6.QtNetwork import QLocalServer, QLocalSocket

from .debug_log import log_exception
from .media_control import MEDIA_COMMANDS

PROTOCOL_VERSION = 1
PIPE_PREFIX = "Playtube.Remote."

MAX_LINE_BYTES = 4096
MAX_CLIENTS = 8

TAB_NAMES = ("youtube", "music")
TARGETS = frozenset({"auto", *TAB_NAMES})
APP_COMMANDS = frozenset({"status", "show", "switch_tab", "list_playlists", "play_playlist"})

# Playlist-IDs (z.B. "PL...", "OLAK5uy_...", "LM") - landen in einer URL, daher streng.
PLAYLIST_ID_RE = re.compile(r"^[A-Za-z0-9_-]{2,64}$")
LIKED_MUSIC = {"id": "LM", "title": "Titel, die ich mag"}
MAX_PLAYLISTS = 100
MAX_PLAYLIST_TITLE = 100

# Befehl -> (min, max) fuer `value`; Befehle ohne Eintrag ignorieren `value`.
_VALUE_RANGES: dict[str, tuple[float, float]] = {
    "seek": (-3600, 3600),
    "volume_change": (-100, 100),
    "set_volume": (0, 100),
}

# Nur diese Felder des Media-Probes (media_probe.py) werden nach aussen gegeben.
_STATE_FIELDS = (
    "isMusic", "title", "subtitle", "thumbnail", "playing", "currentTime", "duration",
    "hasVideo", "volume", "muted", "likeStatus", "repeatMode",
)


class ProtocolError(ValueError):
    """Ungueltige Anfrage - wird dem Client als {"ok": false, "error": ...} gemeldet."""


@dataclass(frozen=True)
class Request:
    id: Any
    command: str
    target: str = "auto"
    value: float = 0
    playlist: str | None = None


class RemoteController(Protocol):
    """Was der Server von der App braucht (implementiert von MainWindow)."""

    def remote_state(self) -> dict: ...
    def remote_show(self) -> None: ...
    def remote_switch_tab(self, target: str) -> None: ...
    def remote_media_command(self, target: str, command: str, value: float) -> None: ...
    def remote_list_playlists(self, callback: Callable[[Any], None]) -> None: ...
    def remote_play_playlist(self, playlist_id: str) -> None: ...


def parse_request(line: str) -> Request:
    """Parst und validiert eine Anfragezeile. Wirft ProtocolError bei jedem Fehler."""
    try:
        data = json.loads(line)
    except json.JSONDecodeError as exc:
        raise ProtocolError("ungueltiges JSON") from exc
    if not isinstance(data, dict):
        raise ProtocolError("Anfrage muss ein JSON-Objekt sein")

    request_id = data.get("id")
    if isinstance(request_id, bool) or (
        request_id is not None and not isinstance(request_id, (int, str))
    ):
        raise ProtocolError("id muss Zahl oder Text sein")

    command = data.get("cmd")
    if not isinstance(command, str) or (
        command not in APP_COMMANDS and command not in MEDIA_COMMANDS
    ):
        raise ProtocolError(f"unbekannter Befehl: {command!r}")

    target = data.get("target", "auto")
    if not isinstance(target, str) or target not in TARGETS:
        raise ProtocolError(f"unbekanntes Ziel: {target!r}")

    playlist = None
    if command == "play_playlist":
        playlist = data.get("playlist")
        if not isinstance(playlist, str) or not PLAYLIST_ID_RE.fullmatch(playlist):
            raise ProtocolError("play_playlist braucht eine gueltige Playlist-ID")

    return Request(
        id=request_id,
        command=command,
        target=target,
        value=_parse_value(command, data),
        playlist=playlist,
    )


def _parse_value(command: str, data: dict) -> float:
    value_range = _VALUE_RANGES.get(command)
    if value_range is None:
        return 0
    raw = data.get("value")
    # bool ist in Python ein int - true/false sind hier aber kein sinnvoller Wert.
    if isinstance(raw, bool) or not isinstance(raw, (int, float)) or not math.isfinite(raw):
        raise ProtocolError(f"{command} braucht einen Zahlenwert")
    low, high = value_range
    if not low <= raw <= high:
        raise ProtocolError(f"{command}: Wert muss zwischen {low:g} und {high:g} liegen")
    return float(raw)


Reply = Callable[[dict], None]


def handle_request(request: Request, controller: RemoteController, reply: Reply) -> None:
    """Fuehrt eine gueltige Anfrage aus und meldet die Antwort (ohne id) ueber `reply` -
    sofort, oder bei list_playlists sobald die Seite geantwortet hat."""
    if request.command == "status":
        reply({"ok": True, "state": controller.remote_state()})
        return
    if request.command == "list_playlists":
        controller.remote_list_playlists(
            lambda raw: reply({"ok": True, "playlists": sanitize_playlists(raw)})
        )
        return
    if request.command == "show":
        controller.remote_show()
    elif request.command == "switch_tab":
        controller.remote_switch_tab(request.target)
    elif request.command == "play_playlist":
        controller.remote_play_playlist(request.playlist)
    else:
        controller.remote_media_command(request.target, request.command, request.value)
    reply({"ok": True})


def sanitize_playlists(raw: Any) -> list[dict]:
    """Bereinigt die von der Seite gelieferte Liste (fremde Daten!): nur gueltige IDs,
    Titel als gekuerzter Text, ohne Duplikate, "Titel, die ich mag" immer zuerst."""
    result = [dict(LIKED_MUSIC)]
    seen = {LIKED_MUSIC["id"]}
    for item in raw if isinstance(raw, list) else []:
        if len(result) >= MAX_PLAYLISTS:
            break
        if not isinstance(item, dict):
            continue
        playlist_id, title = item.get("id"), item.get("title")
        if not isinstance(playlist_id, str) or not PLAYLIST_ID_RE.fullmatch(playlist_id):
            continue
        if playlist_id in seen:
            continue
        seen.add(playlist_id)
        clean_title = " ".join(title.split())[:MAX_PLAYLIST_TITLE] if isinstance(title, str) else ""
        result.append({"id": playlist_id, "title": clean_title or playlist_id})
    return result


def resolve_target(target: str, visible_tab: str | None, tabs: dict[str, dict | None]) -> str:
    """Welcher Tab einen Medienbefehl bekommt. "auto": der Tab, der gerade abspielt
    (spielen beide, der sichtbare), sonst der sichtbare Browser-Tab, sonst der mit einem
    Video, sonst YouTube."""
    if target in TAB_NAMES:
        return target
    playing = [name for name in TAB_NAMES if (tabs.get(name) or {}).get("playing")]
    if len(playing) == 1:
        return playing[0]
    if visible_tab in TAB_NAMES:
        return visible_tab
    with_video = [name for name in TAB_NAMES if (tabs.get(name) or {}).get("hasVideo")]
    return with_video[0] if with_video else TAB_NAMES[0]


def build_state(version: str, visible_tab: str | None, tabs: dict[str, dict | None]) -> dict:
    """Status-Antwort: pro Tab die oeffentlichen Probe-Felder plus der "auto"-Zieltab."""
    public_tabs = {}
    for name in TAB_NAMES:
        info = tabs.get(name)
        public_tabs[name] = {key: info.get(key) for key in _STATE_FIELDS} if info else None
    return {
        "protocol": PROTOCOL_VERSION,
        "version": version,
        "visibleTab": visible_tab,
        "activeTab": resolve_target("auto", visible_tab, tabs),
        "tabs": public_tabs,
    }


def encode_response(request_id: Any, response: dict) -> bytes:
    return (json.dumps({"id": request_id, **response}, ensure_ascii=False) + "\n").encode("utf-8")


def process_line(line: str, controller: RemoteController, write: Callable[[bytes], None]) -> None:
    """Eine Anfragezeile -> genau eine kodierte Antwortzeile ueber `write`. Wirft nie."""
    try:
        request = parse_request(line)
    except ProtocolError as exc:
        write(encode_response(None, {"ok": False, "error": str(exc)}))
        return
    try:
        handle_request(request, controller, lambda response: write(encode_response(request.id, response)))
    except Exception as exc:  # noqa: BLE001 - ein Steuerbefehl darf die App nie abstuerzen lassen
        log_exception(f"remote_control: {request.command}", exc)
        write(encode_response(request.id, {"ok": False, "error": "interner Fehler"}))


class RemoteControlServer(QObject):
    """Lauscht auf der Named Pipe und beantwortet Anfragen im Qt-Hauptthread (die
    Controller-Methoden fassen Widgets an und duerfen nur dort laufen)."""

    def __init__(self, key: str, controller: RemoteController, parent: QObject | None = None) -> None:
        super().__init__(parent)
        self._key = key
        self._controller = controller
        self._server: QLocalServer | None = None
        self._buffers: dict[QLocalSocket, bytearray] = {}

    def start(self) -> bool:
        server = QLocalServer(self)
        # Nur der angemeldete Benutzer darf verbinden (Windows: Pipe-ACL).
        server.setSocketOptions(QLocalServer.SocketOption.UserAccessOption)
        if not server.listen(self._key):
            log_exception("remote_control: listen", RuntimeError(server.errorString()))
            return False
        server.newConnection.connect(self._on_new_connection)
        self._server = server
        return True

    def _on_new_connection(self) -> None:
        while self._server is not None and self._server.hasPendingConnections():
            socket = self._server.nextPendingConnection()
            if len(self._buffers) >= MAX_CLIENTS:
                socket.disconnectFromServer()
                socket.deleteLater()
                continue
            self._buffers[socket] = bytearray()
            socket.readyRead.connect(lambda s=socket: self._on_ready_read(s))
            socket.disconnected.connect(lambda s=socket: self._on_disconnected(s))

    def _on_ready_read(self, socket: QLocalSocket) -> None:
        buffer = self._buffers.get(socket)
        if buffer is None:
            return
        buffer.extend(bytes(socket.readAll().data()))
        while (newline := buffer.find(b"\n")) >= 0:
            raw = bytes(buffer[:newline])
            del buffer[: newline + 1]
            line = raw.decode("utf-8", errors="replace").strip()
            if line:
                process_line(line, self._controller, lambda data, s=socket: self._write(s, data))
        if len(buffer) > MAX_LINE_BYTES:
            # Kein Zeilenende in Sicht: Client verhaelt sich falsch -> trennen.
            buffer.clear()
            socket.write(encode_response(None, {"ok": False, "error": "Nachricht zu lang"}))
            socket.disconnectFromServer()

    def _write(self, socket: QLocalSocket, data: bytes) -> None:
        # Asynchrone Antworten (list_playlists) koennen nach dem Trennen eintreffen.
        if socket in self._buffers:
            socket.write(data)

    def _on_disconnected(self, socket: QLocalSocket) -> None:
        self._buffers.pop(socket, None)
        socket.deleteLater()
