"""Sehr einfaches, immer aktives Datei-Log fuer schwer reproduzierbare Bugs.

Die gepackte .exe laeuft ohne Konsolenfenster (console=False in packaging/playtube.spec),
also sind print()-Debugausgaben dort unsichtbar - selbst mit PLAYTUBE_DEBUG=1 sieht der
Nutzer nichts. Dieses Modul schreibt stattdessen in eine kleine, automatisch gekappte
Log-Datei im App-Datenordner, die sich jederzeit nachtraeglich auslesen laesst.
"""
from __future__ import annotations

import time
import traceback
from pathlib import Path

from .config import app_data_dir

_MAX_LINES = 500


def _log_path() -> Path:
    d = app_data_dir() / "logs"
    d.mkdir(parents=True, exist_ok=True)
    return d / "discord_rpc.log"


def log_line(msg: str) -> None:
    """Haengt eine Zeile mit Zeitstempel an - haelt die Datei klein, indem bei
    Ueberlaenge nur die letzten _MAX_LINES Zeilen behalten werden."""
    try:
        path = _log_path()
        timestamp = time.strftime("%Y-%m-%d %H:%M:%S")
        line = f"[{timestamp}] {msg}\n"
        with open(path, "a", encoding="utf-8") as f:
            f.write(line)

        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        if len(lines) > _MAX_LINES:
            path.write_text("\n".join(lines[-_MAX_LINES:]) + "\n", encoding="utf-8")
    except Exception:
        # Logging darf niemals die App zum Absturz bringen.
        pass


def log_exception(context: str, exc: BaseException) -> None:
    log_line(f"{context}: {exc!r}\n{''.join(traceback.format_exception(exc))}")
