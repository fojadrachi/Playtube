"""Einstellungen-Tab: Discord-RPC, Auto-Update und Start-Tab direkt in der App
bearbeitbar, ohne config.json von Hand anfassen zu muessen."""
from __future__ import annotations

from typing import Any

from PySide6.QtCore import Signal
from PySide6.QtWidgets import (
    QCheckBox,
    QComboBox,
    QFormLayout,
    QGroupBox,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QMessageBox,
    QPushButton,
    QSpinBox,
    QVBoxLayout,
    QWidget,
)

from . import __version__ as APP_VERSION
from .config import APP_NAME, save_config
from .updater import GITHUB_REPO


class SettingsTab(QWidget):
    """Speichert Aenderungen sofort in config.json und meldet sie per Signal an
    MainWindow, damit Discord-RPC/Auto-Update ohne Neustart neu konfiguriert werden."""

    settingsSaved = Signal(dict)

    def __init__(self, config: dict[str, Any], parent=None):
        super().__init__(parent)
        self._config = config

        outer = QVBoxLayout(self)
        outer.setContentsMargins(32, 28, 32, 28)
        outer.setSpacing(18)

        title = QLabel(f"{APP_NAME}-Einstellungen")
        title.setStyleSheet("font-size: 20px; font-weight: 600;")
        outer.addWidget(title)

        discord_cfg = config.get("discord", {})
        discord_box = QGroupBox("Discord Rich Presence")
        discord_form = QFormLayout(discord_box)
        self._discord_enabled = QCheckBox("Aktiviert")
        self._discord_enabled.setChecked(bool(discord_cfg.get("enabled", True)))
        self._client_id = QLineEdit(str(discord_cfg.get("client_id", "")))
        self._client_id.setPlaceholderText("Discord Application Client-ID")
        self._discord_interval = QSpinBox()
        self._discord_interval.setRange(5, 120)
        self._discord_interval.setSuffix(" s")
        self._discord_interval.setValue(int(discord_cfg.get("update_interval_seconds", 15)))
        self._show_idle = QCheckBox("Status anzeigen, wenn gerade nichts läuft")
        self._show_idle.setChecked(bool(discord_cfg.get("show_idle_presence", True)))
        discord_form.addRow(self._discord_enabled)
        discord_form.addRow("Client-ID:", self._client_id)
        discord_form.addRow("Update-Intervall:", self._discord_interval)
        discord_form.addRow(self._show_idle)
        outer.addWidget(discord_box)

        updates_cfg = config.get("updates", {})
        update_box = QGroupBox("Automatische Updates")
        update_form = QFormLayout(update_box)
        self._updates_enabled = QCheckBox("Aktiviert")
        self._updates_enabled.setChecked(bool(updates_cfg.get("enabled", True)))
        self._check_interval = QSpinBox()
        self._check_interval.setRange(1, 168)
        self._check_interval.setSuffix(" h")
        self._check_interval.setValue(int(updates_cfg.get("check_interval_hours", 6)))
        update_form.addRow(self._updates_enabled)
        update_form.addRow("Prüfintervall:", self._check_interval)
        outer.addWidget(update_box)

        general_box = QGroupBox("Allgemein")
        general_form = QFormLayout(general_box)
        self._start_tab = QComboBox()
        self._start_tab.addItem("YouTube", "youtube")
        self._start_tab.addItem("YouTube Music", "music")
        idx = self._start_tab.findData(config.get("start_tab", "youtube"))
        self._start_tab.setCurrentIndex(max(0, idx))
        general_form.addRow("Beim Start öffnen:", self._start_tab)
        outer.addWidget(general_box)

        button_row = QHBoxLayout()
        save_btn = QPushButton("Speichern")
        save_btn.clicked.connect(self._on_save)
        button_row.addWidget(save_btn)
        button_row.addStretch(1)
        outer.addLayout(button_row)

        info = QLabel(f"{APP_NAME} v{APP_VERSION}  ·  github.com/{GITHUB_REPO}")
        info.setStyleSheet("color: palette(mid);")
        outer.addWidget(info)

        outer.addStretch(1)

    def _on_save(self) -> None:
        self._config.setdefault("discord", {})
        self._config.setdefault("updates", {})

        self._config["discord"]["enabled"] = self._discord_enabled.isChecked()
        self._config["discord"]["client_id"] = self._client_id.text().strip()
        self._config["discord"]["update_interval_seconds"] = self._discord_interval.value()
        self._config["discord"]["show_idle_presence"] = self._show_idle.isChecked()

        self._config["updates"]["enabled"] = self._updates_enabled.isChecked()
        self._config["updates"]["check_interval_hours"] = self._check_interval.value()

        self._config["start_tab"] = self._start_tab.currentData()

        save_config(self._config)
        self.settingsSaved.emit(self._config)
        QMessageBox.information(
            self,
            "Gespeichert",
            "Einstellungen gespeichert und übernommen (Discord-Verbindung wurde "
            "mit den neuen Werten neu gestartet).",
        )
