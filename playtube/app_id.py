"""Branding-Helfer: eigener Prozessname/Icon im Taskmanager, in der Taskleiste und
(so weit von Windows respektiert) im Lautstaerkemixer.

Wichtig: Diese Funktionen muessen VOR dem Import von QtWebEngineWidgets/QtWebEngineCore
aufgerufen werden, weil QtWebEngine seinen Sub-Prozess (QtWebEngineProcess.exe) beim
Import-Zeitpunkt vorbereitet.
"""
from __future__ import annotations

import os
import sys
from pathlib import Path

from .config import APP_AUMID


def set_app_user_model_id() -> None:
    """Setzt die Windows AppUserModelID fuer diesen Prozess.

    Das sorgt dafuer, dass Windows die App in Taskleiste/Alt-Tab/Benachrichtigungen
    als eigenstaendige Anwendung fuehrt (eigenes Icon, eigene Gruppierung) statt als
    generisches "Python".
    """
    if sys.platform != "win32":
        return
    try:
        import ctypes

        ctypes.windll.shell32.SetCurrentProcessExplicitAppUserModelID(APP_AUMID)
    except Exception:
        # Auf aelteren Windows-Versionen oder ohne shell32 einfach ignorieren.
        pass


def configure_webengine_process_path() -> None:
    """Wenn ein umbenannter QtWebEngineProcess (siehe packaging/build.ps1) neben der
    .exe liegt, wird QtWebEngine angewiesen diesen zu benutzen statt des Standard-
    "QtWebEngineProcess.exe". Dadurch zeigt auch der Sub-Prozess, der tatsaechlich den
    Ton wiedergibt, im Taskmanager/Lautstaerkemixer den Playtube-Namen statt
    "QtWebEngineProcess".

    Nur relevant fuer gepackte (PyInstaller-)Builds. Im Entwicklungsmodus (python
    main.py) wird der Standardprozess von PySide6 benutzt - das ist voellig normal
    und hat keinen Einfluss auf die Funktion der App.
    """
    if sys.platform != "win32" or not getattr(sys, "frozen", False):
        return

    exe_dir = Path(sys.executable).resolve().parent
    # PyInstaller legt PySide6 je nach Version unterschiedlich tief in _internal ab
    # (z.B. _internal\PySide6\), daher rekursiv suchen statt feste Pfade anzunehmen.
    try:
        candidate = next(exe_dir.rglob("PlaytubeHelper.exe"))
    except StopIteration:
        return
    os.environ["QTWEBENGINEPROCESS_PATH"] = str(candidate)
