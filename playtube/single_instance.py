"""Einzelinstanz-Schutz: verhindert, dass zwei Playtube-Prozesse gleichzeitig auf
demselben Browser-Profil (webprofile/) laufen.

Hintergrund: Laufen zwei Instanzen auf demselben QtWebEngine-Profil (Cache, GPUCache,
Service Worker, Cookies), zeigt die ZWEITE Instanz auf YouTube/YouTube Music sichtbar
keine Icons mehr (Play/Pause, Suche, Menue ...). Das passiert leicht, weil das Schliessen
des Fensters Playtube nur in den Tray minimiert - eine alte Version laeuft dann
unsichtbar weiter, waehrend eine neu heruntergeladene Version zusaetzlich gestartet wird.

Ansatz: ein lokaler Server (Named Pipe unter Windows, Unix-Socket unter Linux) pro
Datenordner. Der erste Prozess lauscht darauf; jeder weitere verbindet sich damit,
bittet die laufende Instanz, ihr Fenster zu zeigen, und beendet sich selbst.
"""
from __future__ import annotations

from PySide6.QtCore import QObject, Signal
from PySide6.QtNetwork import QLocalServer, QLocalSocket

_CONNECT_TIMEOUT_MS = 500
_WRITE_TIMEOUT_MS = 500
_MSG_SHOW = "show"
_MSG_PATCH = "patch"
_SEPARATOR = "\t"


class SingleInstanceGuard(QObject):
    """Der erste Prozess mit einem bestimmten `key` "besitzt" die Instanz; jeder weitere
    Aufruf von acquire() liefert False und reicht seine Anfrage an den Besitzer weiter."""

    showRequested = Signal()
    patchRequested = Signal(str)

    def __init__(self, key: str, parent: QObject | None = None) -> None:
        super().__init__(parent)
        self._key = key
        self._server: QLocalServer | None = None

    def acquire(self, local_patch: str | None = None) -> bool:
        """True, wenn dieser Prozess die (einzige) Instanz sein darf. False, wenn bereits
        eine laeuft - dann wurde ihr "Fenster zeigen" (bzw. der Patch-Pfad) gemeldet."""
        if self._notify_running_instance(local_patch):
            return False

        # Kein Besitzer erreichbar: ggf. verwaisten Socket (Linux) aufraeumen und selbst
        # lauschen.
        QLocalServer.removeServer(self._key)
        server = QLocalServer(self)
        if not server.listen(self._key):
            # Knappes Rennen zweier gleichzeitiger Starts: der andere war einen Tick
            # frueher - nochmal versuchen, ihn zu erreichen, sonst trotzdem starten
            # (lieber ohne Schutz laufen als gar nicht).
            return not self._notify_running_instance(local_patch)

        server.newConnection.connect(self._on_new_connection)
        self._server = server
        return True

    def _notify_running_instance(self, local_patch: str | None) -> bool:
        socket = QLocalSocket(self)
        socket.connectToServer(self._key)
        if not socket.waitForConnected(_CONNECT_TIMEOUT_MS):
            socket.abort()
            return False

        message = f"{_MSG_PATCH}{_SEPARATOR}{local_patch}" if local_patch else _MSG_SHOW
        socket.write(message.encode("utf-8"))
        socket.waitForBytesWritten(_WRITE_TIMEOUT_MS)
        socket.disconnectFromServer()
        return True

    def _on_new_connection(self) -> None:
        if self._server is None:
            return
        while self._server.hasPendingConnections():
            socket = self._server.nextPendingConnection()
            socket.waitForReadyRead(_CONNECT_TIMEOUT_MS)
            message = bytes(socket.readAll().data()).decode("utf-8", errors="replace")
            socket.disconnectFromServer()
            socket.deleteLater()
            self._dispatch(message)

    def _dispatch(self, message: str) -> None:
        command, _, argument = message.partition(_SEPARATOR)
        if command == _MSG_PATCH and argument:
            self.patchRequested.emit(argument)
        self.showRequested.emit()
