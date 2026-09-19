"""Einstellungen-Tab: Discord-RPC, Auto-Update und Start-Tab direkt in der App
bearbeitbar, ohne config.json von Hand anfassen zu muessen."""
from __future__ import annotations

from typing import Any

from PySide6.QtCore import Signal
from PySide6.QtWidgets import (
    QApplication,
    QCheckBox,
    QComboBox,
    QFormLayout,
    QGroupBox,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QMessageBox,
    QProgressBar,
    QPushButton,
    QSpinBox,
    QVBoxLayout,
    QWidget,
)

from . import __version__ as APP_VERSION
from .config import APP_NAME, save_config
from .updater import GITHUB_REPO

# Helle Schrift fuer Versionsnummer, Update-Status und GitHub-Link. palette(text) ist die
# Standard-Textfarbe (im dunklen Design weiss, im hellen dunkel) - das fruehere
# palette(mid) war ein dunkles Grau, das auf dem dunklen Hintergrund kaum lesbar war.
_LIGHT_TEXT_STYLE = "color: palette(text);"

# Update-Fortschrittsbalken: helle Beschriftung und gut sichtbarer Balken. Sobald ein
# Stylesheet gesetzt ist, zeichnet Qt den Balken nicht mehr mit dem nativen Stil - deshalb
# hier Rahmen, Hintergrund und Balken (chunk) vollstaendig angegeben.
_PROGRESS_BAR_STYLE = """
QProgressBar {
    color: palette(text);
    background-color: palette(base);
    border: 1px solid palette(mid);
    border-radius: 4px;
    text-align: center;
}
QProgressBar::chunk {
    background-color: #2f81f7;
    border-radius: 3px;
}
"""


class SettingsTab(QWidget):
    """Speichert Aenderungen sofort in config.json und meldet sie per Signal an
    MainWindow, damit Discord-RPC/Auto-Update ohne Neustart neu konfiguriert werden."""

    settingsSaved = Signal(dict)
    checkUpdatesRequested = Signal()
    installUpdateRequested = Signal()
    # Neue Beschreibung, was gerade bearbeitet wird (fuer die Discord-Presence, siehe
    # MainWindow._update_discord_presence).
    activityChanged = Signal(str)

    def __init__(self, config: dict[str, Any], parent=None):
        super().__init__(parent)
        self._config = config

        outer = QVBoxLayout(self)
        outer.setContentsMargins(32, 28, 32, 28)
        outer.setSpacing(18)

        header = QVBoxLayout()
        header.setSpacing(2)
        title = QLabel(f"{APP_NAME}-Einstellungen")
        title.setStyleSheet("font-size: 20px; font-weight: 600;")
        header.addWidget(title)
        version_label = QLabel(f"Version {APP_VERSION}")
        version_label.setStyleSheet(_LIGHT_TEXT_STYLE)
        header.addWidget(version_label)
        outer.addLayout(header)

        discord_cfg = config.get("discord", {})
        discord_box = QGroupBox("Discord Rich Presence")
        discord_form = QFormLayout(discord_box)
        self._discord_enabled = QCheckBox("Aktiviert")
        self._discord_enabled.setChecked(bool(discord_cfg.get("enabled", True)))
        self._client_id = QLineEdit(str(discord_cfg.get("client_id", "")))
        self._client_id.setPlaceholderText("Discord Application Client-ID")
        self._discord_interval = QSpinBox()
        # Minimum 15s: Discord ignoriert Rich-Presence-Updates, die haeufiger kommen,
        # stillschweigend (fuehrt zu "eingefrorenem" Bild/Fortschrittsbalken).
        self._discord_interval.setRange(15, 120)
        self._discord_interval.setSuffix(" s")
        self._discord_interval.setValue(max(15, int(discord_cfg.get("update_interval_seconds", 15))))
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

        update_actions = QHBoxLayout()
        self._check_updates_btn = QPushButton("Jetzt nach Updates suchen")
        self._check_updates_btn.clicked.connect(self.checkUpdatesRequested.emit)
        update_actions.addWidget(self._check_updates_btn)
        self._install_update_btn = QPushButton("Update installieren")
        self._install_update_btn.setVisible(False)
        self._install_update_btn.clicked.connect(self.installUpdateRequested.emit)
        update_actions.addWidget(self._install_update_btn)
        update_actions.addStretch(1)
        update_form.addRow(update_actions)

        self._update_status_label = QLabel("")
        self._update_status_label.setStyleSheet(_LIGHT_TEXT_STYLE)
        self._update_status_label.setWordWrap(True)
        update_form.addRow(self._update_status_label)

        self._update_progress = QProgressBar()
        self._update_progress.setRange(0, 100)
        self._update_progress.setTextVisible(True)
        self._update_progress.setStyleSheet(_PROGRESS_BAR_STYLE)
        self._update_progress.setVisible(False)
        update_form.addRow(self._update_progress)

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

        info = QLabel(f"github.com/{GITHUB_REPO}")
        info.setStyleSheet(_LIGHT_TEXT_STYLE)
        outer.addWidget(info)

        outer.addStretch(1)

        # Beschreibung je Eingabefeld fuer die Discord-Presence ("was wird bearbeitet").
        # Bewusst nur Feld-NAMEN, nie Inhalte (z.B. nicht die Client-ID selbst).
        self._activity_by_widget: dict[QWidget, str] = {
            self._discord_enabled: "Bearbeitet: Discord Rich Presence › Aktiviert",
            self._client_id: "Bearbeitet: Discord Rich Presence › Client-ID",
            self._discord_interval: "Bearbeitet: Discord Rich Presence › Update-Intervall",
            self._show_idle: "Bearbeitet: Discord Rich Presence › Status im Leerlauf",
            self._updates_enabled: "Bearbeitet: Automatische Updates › Aktiviert",
            self._check_interval: "Bearbeitet: Automatische Updates › Prüfintervall",
            self._check_updates_btn: "Sucht nach Updates",
            self._install_update_btn: "Installiert ein Update",
            self._start_tab: "Bearbeitet: Allgemein › Beim Start öffnen",
            save_btn: "Speichert die Einstellungen",
        }
        self._current_activity: str | None = None
        app = QApplication.instance()
        if app is not None:
            app.focusChanged.connect(self._on_focus_changed)

    def current_activity(self) -> str | None:
        """Zuletzt bearbeitetes Feld (None, solange noch kein Feld angeklickt wurde)."""
        return self._current_activity

    def _on_focus_changed(self, _old: QWidget | None, new: QWidget | None) -> None:
        # Vom fokussierten Widget nach oben suchen: bei QSpinBox/QComboBox liegt der Fokus
        # auf einem internen Kind-Widget (z.B. dem Textfeld des Spinners).
        widget = new
        while widget is not None and widget is not self:
            activity = self._activity_by_widget.get(widget)
            if activity:
                if activity != self._current_activity:
                    self._current_activity = activity
                    self.activityChanged.emit(activity)
                return
            widget = widget.parentWidget()

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

    # ----------------------------------------------------- Update-Status (von MainWindow)

    def set_checking(self) -> None:
        self._check_updates_btn.setEnabled(False)
        self._install_update_btn.setVisible(False)
        self._update_progress.setVisible(False)
        self._update_status_label.setText("Suche nach Updates …")

    def set_check_done(self) -> None:
        self._check_updates_btn.setEnabled(True)

    def set_update_status(self, text: str) -> None:
        self._update_status_label.setText(text)

    def show_update_available(self, version: str) -> None:
        self._update_status_label.setText(f"Neue Version verfügbar: {version}")
        self._install_update_btn.setVisible(True)

    def hide_install_button(self) -> None:
        self._install_update_btn.setVisible(False)

    def set_download_progress(self, percent: int) -> None:
        """percent: 0-100 fuer einen konkreten Fortschritt, -1 fuer unbestimmt
        (z.B. waehrend Entpacken/Installieren), negativ->versteckt den Balken nicht
        automatisch - dafuer set_progress_hidden() aufrufen."""
        if percent < 0:
            self._update_progress.setRange(0, 0)  # "laufender" Balken ohne Prozentzahl
        else:
            self._update_progress.setRange(0, 100)
            self._update_progress.setValue(percent)
        self._update_progress.setVisible(True)

    def set_progress_hidden(self) -> None:
        self._update_progress.setVisible(False)
        self._update_progress.setRange(0, 100)
