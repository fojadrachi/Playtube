"""Playtube - eigenstaendiger YouTube- & YouTube-Music-Player mit Discord Rich Presence.

Start: python main.py  (oder die gepackte Playtube.exe, siehe packaging/README)
"""
from __future__ import annotations

import os
import sys
from pathlib import Path

# Branding-Schritte MUESSEN vor dem Import von QtWebEngine passieren.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from playtube import app_id  # noqa: E402
from playtube.config import APP_NAME, load_config  # noqa: E402

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

    config = load_config()
    window = MainWindow(config)
    window.show()

    return app.exec()


if __name__ == "__main__":
    sys.exit(main())
