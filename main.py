"""Playtube - eigenstaendiger YouTube- & YouTube-Music-Player mit Discord Rich Presence.

Start: python main.py  (oder die gepackte Playtube.exe, siehe packaging/README)
"""
from __future__ import annotations

import os
import sys
from pathlib import Path

# Branding-Schritte MUESSEN vor dem Import von QtWebEngine passieren.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from playtube import __version__, app_id  # noqa: E402
from playtube.config import APP_NAME, app_data_dir, clear_cache_on_update, load_config  # noqa: E402
from playtube.remote_control import PIPE_PREFIX, RemoteControlServer  # noqa: E402
from playtube.single_instance import SingleInstanceGuard  # noqa: E402
from playtube.shortcuts import (  # noqa: E402
    ensure_play_file_association,
    ensure_start_menu_shortcut,
)
from playtube.updater import cleanup_old_staging  # noqa: E402

app_id.set_app_user_model_id()
app_id.configure_webengine_process_path()

# High-DPI, sauberes GPU-Verhalten und (in Kombination mit den Sec-CH-UA-Headern in
# playtube/browser.py) ein moeglichst "normales" Chrome-Fingerprint, damit Google-Login
# das eingebettete QtWebEngine nicht als Embedded-WebView blockiert.
os.environ.setdefault(
    "QTWEBENGINE_CHROMIUM_FLAGS",
    "--disable-features=WinRetrieveSuggestionsOnlyOnDemand "
    "--disable-blink-features=AutomationControlled",
)

from PySide6.QtCore import Qt  # noqa: E402
from PySide6.QtGui import QIcon  # noqa: E402
from PySide6.QtWidgets import QApplication  # noqa: E402

from playtube.mainwindow import MainWindow  # noqa: E402


def _pending_local_patch() -> str | None:
    """Falls Playtube per Doppelklick auf eine ".play"-Patchdatei gestartet wurde
    (siehe shortcuts.ensure_play_file_association), liefert den Pfad dazu."""
    for arg in sys.argv[1:]:
        if arg.lower().endswith(".play") and Path(arg).is_file():
            return str(Path(arg).resolve())
    return None


def main() -> int:
    QApplication.setAttribute(Qt.ApplicationAttribute.AA_ShareOpenGLContexts, True)
    app = QApplication(sys.argv)
    app.setApplicationName(APP_NAME)
    app.setApplicationDisplayName(APP_NAME)
    app.setOrganizationName(APP_NAME)
    app.setQuitOnLastWindowClosed(False)  # Tray haelt die App am Leben

    icon_path = Path(__file__).resolve().parent / "assets" / "icon.ico"
    if icon_path.exists():
        app.setWindowIcon(QIcon(str(icon_path)))

    # Nur EINE Playtube-Instanz pro Datenordner: laufen zwei gleichzeitig auf demselben
    # Browser-Profil, zeigt die zweite auf YouTube keine Icons mehr (siehe
    # playtube/single_instance.py). Muss vor jedem Profil-Zugriff (Cache-Reset, Browser)
    # passieren. Die Instanz eines aelteren Playtube ohne diesen Schutz wird dadurch
    # allerdings nicht erkannt - die muss einmalig ueber das Tray-Icon beendet werden.
    local_patch = _pending_local_patch()
    instance_guard = SingleInstanceGuard(f"{APP_NAME}.SingleInstance.{app_data_dir().name}", app)
    if not instance_guard.acquire(local_patch):
        return 0

    cleanup_old_staging()  # Reste eines frueheren Updates (heruntergeladenes Setup) entfernen

    config = load_config()
    # Cache leeren, wenn seit dem letzten Start ein Update installiert wurde (Login
    # bleibt erhalten, siehe clear_cache_on_update); Startmenue-Verknuepfung fehlt sonst
    # komplett, da Playtube als portables ZIP ohne Installer ausgeliefert wird.
    clear_cache_on_update(__version__)
    ensure_start_menu_shortcut(APP_NAME)
    ensure_play_file_association(APP_NAME)

    window = MainWindow(config)
    window.show()

    # Ein zweiter Start (Verknuepfung, .play-Doppelklick) landet hier in der laufenden
    # Instanz: Fenster aus dem Tray holen bzw. den Patch installieren.
    instance_guard.showRequested.connect(window.show_and_raise)
    instance_guard.patchRequested.connect(window.install_local_patch)

    # Lokale Fernsteuerung (Stream-Dock-Plugin), eigener Pipe-Name pro Datenordner - so
    # steuert das Plugin die installierte Playtube.exe, nicht versehentlich einen Dev-Start.
    if config.get("remote_control", {}).get("enabled", True):
        remote_server = RemoteControlServer(f"{PIPE_PREFIX}{app_data_dir().name}", window, app)
        remote_server.start()

    # Playtube wurde per Doppelklick auf eine heruntergeladene .play-Patchdatei
    # gestartet -> direkt installieren statt selbst etwas herunterzuladen.
    if local_patch:
        window.install_local_patch(local_patch)

    return app.exec()


if __name__ == "__main__":
    sys.exit(main())
