from __future__ import annotations

import os
import sys
from pathlib import Path
from typing import Any

from PySide6.QtCore import QTimer, QUrl, Qt
from PySide6.QtGui import QAction, QIcon
from PySide6.QtWidgets import (
    QLineEdit,
    QMainWindow,
    QMenu,
    QMessageBox,
    QSystemTrayIcon,
    QTabWidget,
    QToolBar,
    QWidget,
)

from . import __version__ as APP_VERSION
from .browser import BrowserTab
from .config import APP_NAME
from .discord_rpc import DiscordRPCWorker
from .updater import UpdateChecker, UpdateInstaller

ASSETS_DIR = Path(__file__).resolve().parent.parent / "assets"


class MainWindow(QMainWindow):
    def __init__(self, config: dict[str, Any]):
        super().__init__()
        self._config = config
        self.setWindowTitle(APP_NAME)
        self.resize(config["window"]["width"], config["window"]["height"])

        icon_path = ASSETS_DIR / "icon.ico"
        self._app_icon = QIcon(str(icon_path)) if icon_path.exists() else QIcon()
        if not self._app_icon.isNull():
            self.setWindowIcon(self._app_icon)

        self._tabs = QTabWidget(self)
        self._tabs.setDocumentMode(True)
        self.setCentralWidget(self._tabs)

        self._youtube_tab = BrowserTab(config["home_youtube"], self)
        self._music_tab = BrowserTab(config["home_music"], self)
        self._tabs.addTab(self._youtube_tab, "YouTube")
        self._tabs.addTab(self._music_tab, "YouTube Music")
        self._tabs.setCurrentIndex(0 if config.get("start_tab") != "music" else 1)

        self._youtube_tab.mediaInfoChanged.connect(self._on_media_info)
        self._music_tab.mediaInfoChanged.connect(self._on_media_info)

        self._latest_media: dict[int, dict] = {}

        self._build_toolbar()
        self._build_tray()

        self._rpc_worker: DiscordRPCWorker | None = None
        discord_cfg = config.get("discord", {})
        if discord_cfg.get("enabled") and discord_cfg.get("client_id"):
            self._rpc_worker = DiscordRPCWorker(
                client_id=str(discord_cfg["client_id"]),
                interval=discord_cfg.get("update_interval_seconds", 15),
                show_idle=discord_cfg.get("show_idle_presence", True),
            )
            self._rpc_worker.start()

        self._update_checker: UpdateChecker | None = None
        self._update_installer: UpdateInstaller | None = None
        self._pending_update_version: str | None = None
        self._setup_auto_update()

    # ------------------------------------------------------------------ UI

    def _build_toolbar(self) -> None:
        toolbar = QToolBar("Navigation", self)
        toolbar.setMovable(False)
        self.addToolBar(toolbar)

        back_action = QAction("◀", self)
        back_action.triggered.connect(lambda: self._current_tab().back())
        toolbar.addAction(back_action)

        forward_action = QAction("▶", self)
        forward_action.triggered.connect(lambda: self._current_tab().forward())
        toolbar.addAction(forward_action)

        reload_action = QAction("⟳", self)
        reload_action.triggered.connect(lambda: self._current_tab().reload())
        toolbar.addAction(reload_action)

        home_action = QAction("⌂", self)
        home_action.triggered.connect(lambda: self._current_tab().go_home())
        toolbar.addAction(home_action)

        toolbar.addSeparator()

        self._url_bar = QLineEdit(self)
        self._url_bar.setPlaceholderText("Suche oder URL eingeben …")
        self._url_bar.returnPressed.connect(self._navigate_to_url_bar)
        toolbar.addWidget(self._url_bar)

        self._tabs.currentChanged.connect(self._on_tab_changed)
        self._youtube_tab.urlChanged.connect(lambda u: self._sync_url_bar(self._youtube_tab, u))
        self._music_tab.urlChanged.connect(lambda u: self._sync_url_bar(self._music_tab, u))

    def _build_tray(self) -> None:
        self._tray = QSystemTrayIcon(self)
        if not self._app_icon.isNull():
            self._tray.setIcon(self._app_icon)
        self._tray.setToolTip(APP_NAME)

        menu = QMenu()
        show_action = menu.addAction("Anzeigen")
        show_action.triggered.connect(self._show_and_raise)

        play_pause_action = menu.addAction("Wiedergabe umschalten")
        play_pause_action.triggered.connect(lambda: self._current_tab().toggle_playback())

        next_action = menu.addAction("Nächster Titel")
        next_action.triggered.connect(lambda: self._current_tab().next_track())

        prev_action = menu.addAction("Vorheriger Titel")
        prev_action.triggered.connect(lambda: self._current_tab().previous_track())

        menu.addSeparator()
        quit_action = menu.addAction("Beenden")
        quit_action.triggered.connect(self._quit)

        self._tray.setContextMenu(menu)
        self._tray.activated.connect(self._on_tray_activated)
        self._tray.show()

    # --------------------------------------------------------------- Slots

    def _current_tab(self) -> BrowserTab:
        return self._tabs.currentWidget()

    def _navigate_to_url_bar(self) -> None:
        text = self._url_bar.text().strip()
        if not text:
            return
        if " " in text or ("." not in text and "://" not in text):
            url = QUrl("https://www.google.com/search?q=" + QUrl.toPercentEncoding(text).data().decode())
        else:
            url = QUrl.fromUserInput(text)
        self._current_tab().setUrl(url)

    def _sync_url_bar(self, tab: BrowserTab, url: QUrl) -> None:
        if self._current_tab() is tab:
            self._url_bar.setText(url.toString())

    def _on_tab_changed(self, index: int) -> None:
        self._url_bar.setText(self._current_tab().url().toString())

    def _on_media_info(self, info: dict) -> None:
        sender = self.sender()
        self._latest_media[id(sender)] = info

        # Update Fenstertitel + Discord: bevorzugt der Tab, der gerade wirklich
        # abspielt, sonst der aktuell sichtbare Tab.
        playing_info = next((i for i in self._latest_media.values() if i.get("playing")), None)
        active_info = playing_info or self._latest_media.get(id(self._current_tab()))

        if active_info and active_info.get("title"):
            self.setWindowTitle(f"{active_info['title']} – {APP_NAME}")
        else:
            self.setWindowTitle(APP_NAME)

        if self._rpc_worker is not None:
            if active_info and active_info.get("hasVideo"):
                self._rpc_worker.submit_media_info(active_info)
            else:
                self._rpc_worker.submit_media_info(None)

    def _on_tray_activated(self, reason) -> None:
        if reason == QSystemTrayIcon.ActivationReason.Trigger:
            self._show_and_raise()

    def _show_and_raise(self) -> None:
        self.showNormal()
        self.raise_()
        self.activateWindow()

    def _quit(self) -> None:
        if self._rpc_worker is not None:
            self._rpc_worker.stop()
            self._rpc_worker.wait(2000)
        self._tray.hide()
        from PySide6.QtWidgets import QApplication

        QApplication.quit()

    # -------------------------------------------------------------- Events

    def closeEvent(self, event) -> None:
        # Beim Schliessen in den Tray minimieren statt beenden (Musik laeuft weiter).
        event.ignore()
        self.hide()
        self._tray.showMessage(
            APP_NAME,
            f"{APP_NAME} läuft im Hintergrund weiter. Rechtsklick auf das Tray-Icon zum Beenden.",
            self._app_icon if not self._app_icon.isNull() else QSystemTrayIcon.MessageIcon.Information,
            3000,
        )

    # -------------------------------------------------------------- Auto-Update

    def _setup_auto_update(self) -> None:
        updates_cfg = self._config.get("updates", {})
        if not updates_cfg.get("enabled", True):
            return

        self._check_for_updates()

        interval_ms = max(1, int(updates_cfg.get("check_interval_hours", 6))) * 60 * 60 * 1000
        self._update_timer = QTimer(self)
        self._update_timer.setInterval(interval_ms)
        self._update_timer.timeout.connect(self._check_for_updates)
        self._update_timer.start()

    def _check_for_updates(self) -> None:
        if self._update_checker is not None and self._update_checker.isRunning():
            return
        self._update_checker = UpdateChecker(self)
        self._update_checker.updateAvailable.connect(self._on_update_available)
        self._update_checker.start()

    def _on_update_available(self, version: str, notes: str, download_url: str) -> None:
        if self._pending_update_version == version:
            return  # Nutzer hat diese Version schon abgelehnt/wird schon gefragt
        self._pending_update_version = version

        self._show_and_raise()
        notes_preview = (notes[:400] + "…") if len(notes) > 400 else notes
        text = f"Eine neue Version von {APP_NAME} ist verfügbar: {version}\n(aktuell installiert: {APP_VERSION})"
        if notes_preview:
            text += f"\n\n{notes_preview}"
        text += "\n\nJetzt installieren?"

        answer = QMessageBox.question(
            self,
            f"{APP_NAME}-Update verfügbar",
            text,
            QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
            QMessageBox.StandardButton.Yes,
        )
        if answer == QMessageBox.StandardButton.Yes:
            self._start_update_install(download_url)

    def _start_update_install(self, download_url: str) -> None:
        self._tray.setToolTip(f"{APP_NAME} – Update wird installiert …")
        self._update_installer = UpdateInstaller(download_url, self)
        self._update_installer.progress.connect(
            lambda msg: self._tray.setToolTip(f"{APP_NAME} – {msg}")
        )
        self._update_installer.finished_ok.connect(self._on_update_finished)
        self._update_installer.failed.connect(self._on_update_failed)
        self._update_installer.start()

    def _on_update_finished(self) -> None:
        if getattr(sys, "frozen", False):
            # Ein Hintergrund-Skript wartet bereits darauf, dass dieser Prozess
            # beendet wird, tauscht dann die Dateien aus und startet die App neu.
            self._quit()
        else:
            QMessageBox.information(
                self,
                APP_NAME,
                "Update installiert. Die App wird jetzt neu gestartet.",
            )
            self._restart_dev_process()

    def _on_update_failed(self, error: str) -> None:
        self._tray.setToolTip(APP_NAME)
        QMessageBox.warning(self, f"{APP_NAME}-Update fehlgeschlagen", error)

    def _restart_dev_process(self) -> None:
        if self._rpc_worker is not None:
            self._rpc_worker.stop()
            self._rpc_worker.wait(2000)
        python = sys.executable
        os.execv(python, [python] + sys.argv)
