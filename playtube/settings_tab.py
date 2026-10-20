"""Einstellungen-Tab: Discord-RPC, Auto-Update, Start-Tab und Sprache direkt in der App
bearbeitbar, ohne config.json von Hand anfassen zu muessen."""
from __future__ import annotations

from typing import Any

from PySide6.QtCore import Signal
from PySide6.QtMultimedia import QMediaDevices
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
from .audio_routing import list_output_devices
from .config import APP_NAME, save_config
from .i18n import LANGUAGES, get_language, tr
from .updater import REPO_URL

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
        self._title = QLabel()
        self._title.setStyleSheet("font-size: 20px; font-weight: 600;")
        header.addWidget(self._title)
        self._version_label = QLabel()
        self._version_label.setStyleSheet(_LIGHT_TEXT_STYLE)
        header.addWidget(self._version_label)
        outer.addLayout(header)

        # Formular-Beschriftungen als eigene QLabels, damit retranslate() sie neu setzen kann.
        self._form_labels: dict[str, QLabel] = {}

        discord_cfg = config.get("discord", {})
        self._discord_box = QGroupBox()
        discord_form = QFormLayout(self._discord_box)
        self._discord_enabled = QCheckBox()
        self._discord_enabled.setChecked(bool(discord_cfg.get("enabled", True)))
        self._client_id = QLineEdit(str(discord_cfg.get("client_id", "")))
        self._discord_interval = QSpinBox()
        # Minimum 15s: Discord ignoriert Rich-Presence-Updates, die haeufiger kommen,
        # stillschweigend (fuehrt zu "eingefrorenem" Bild/Fortschrittsbalken).
        self._discord_interval.setRange(15, 120)
        self._discord_interval.setSuffix(" s")
        self._discord_interval.setValue(max(15, int(discord_cfg.get("update_interval_seconds", 15))))
        self._show_idle = QCheckBox()
        self._show_idle.setChecked(bool(discord_cfg.get("show_idle_presence", True)))
        discord_form.addRow(self._discord_enabled)
        discord_form.addRow(self._form_label("field.client_id"), self._client_id)
        discord_form.addRow(self._form_label("field.update_interval"), self._discord_interval)
        discord_form.addRow(self._show_idle)
        outer.addWidget(self._discord_box)

        updates_cfg = config.get("updates", {})
        self._update_box = QGroupBox()
        update_form = QFormLayout(self._update_box)
        self._updates_enabled = QCheckBox()
        self._updates_enabled.setChecked(bool(updates_cfg.get("enabled", True)))
        self._check_interval = QSpinBox()
        self._check_interval.setRange(1, 168)
        self._check_interval.setSuffix(" h")
        self._check_interval.setValue(int(updates_cfg.get("check_interval_hours", 6)))
        update_form.addRow(self._updates_enabled)
        update_form.addRow(self._form_label("field.check_interval"), self._check_interval)

        update_actions = QHBoxLayout()
        self._check_updates_btn = QPushButton()
        self._check_updates_btn.clicked.connect(self.checkUpdatesRequested.emit)
        update_actions.addWidget(self._check_updates_btn)
        self._install_update_btn = QPushButton()
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

        outer.addWidget(self._update_box)

        audio_cfg = config.get("audio", {})
        self._audio_box = QGroupBox()
        audio_form = QFormLayout(self._audio_box)
        self._youtube_output = QComboBox()
        self._music_output = QComboBox()
        audio_form.addRow(self._form_label("field.youtube"), self._youtube_output)
        audio_form.addRow(self._form_label("field.music"), self._music_output)
        self._audio_hint = QLabel()
        self._audio_hint.setWordWrap(True)
        self._audio_hint.setStyleSheet(_LIGHT_TEXT_STYLE)
        audio_form.addRow(self._audio_hint)
        outer.addWidget(self._audio_box)

        # QMediaDevices meldet, wenn Geraete ein-/ausgesteckt werden -> Listen aktuell halten.
        self._media_devices = QMediaDevices(self)
        self._fill_output_combo(self._youtube_output, str(audio_cfg.get("youtube_output", "") or ""))
        self._fill_output_combo(self._music_output, str(audio_cfg.get("music_output", "") or ""))
        self._media_devices.audioOutputsChanged.connect(self._reload_output_devices)

        self._general_box = QGroupBox()
        general_form = QFormLayout(self._general_box)
        self._start_tab = QComboBox()
        self._start_tab.addItem("YouTube", "youtube")
        self._start_tab.addItem("YouTube Music", "music")
        idx = self._start_tab.findData(config.get("start_tab", "youtube"))
        self._start_tab.setCurrentIndex(max(0, idx))
        general_form.addRow(self._form_label("field.start_tab"), self._start_tab)
        # Sprachnamen stehen jeweils in ihrer eigenen Sprache und werden nie uebersetzt.
        self._language = QComboBox()
        for code, name in LANGUAGES.items():
            self._language.addItem(name, code)
        self._language.setCurrentIndex(max(0, self._language.findData(get_language())))
        general_form.addRow(self._form_label("field.language"), self._language)
        outer.addWidget(self._general_box)

        button_row = QHBoxLayout()
        self._save_btn = QPushButton()
        self._save_btn.clicked.connect(self._on_save)
        button_row.addWidget(self._save_btn)
        button_row.addStretch(1)
        outer.addLayout(button_row)

        info = QLabel(REPO_URL.removeprefix("https://"))
        info.setStyleSheet(_LIGHT_TEXT_STYLE)
        outer.addWidget(info)

        outer.addStretch(1)

        # (Abschnitt, Feld) bzw. fester Text je Eingabefeld fuer die Discord-Presence ("was
        # wird bearbeitet"). Bewusst nur Feld-NAMEN, nie Inhalte (z.B. nicht die Client-ID
        # selbst). Gespeichert werden Schluessel, damit ein Sprachwechsel greift.
        self._activity_by_widget: dict[QWidget, tuple[str, str | None]] = {
            self._discord_enabled: ("section.discord", "field.enabled"),
            self._client_id: ("section.discord", "field.client_id"),
            self._discord_interval: ("section.discord", "field.update_interval"),
            self._show_idle: ("section.discord", "field.idle_status"),
            self._updates_enabled: ("section.updates", "field.enabled"),
            self._check_interval: ("section.updates", "field.check_interval"),
            self._youtube_output: ("section.audio", "field.youtube"),
            self._music_output: ("section.audio", "field.music"),
            self._check_updates_btn: ("activity.checking_updates", None),
            self._install_update_btn: ("activity.installing_update", None),
            self._start_tab: ("section.general", "field.start_tab"),
            self._language: ("section.general", "field.language"),
            self._save_btn: ("activity.saving", None),
        }
        self._current_activity_key: tuple[str, str | None] | None = None
        app = QApplication.instance()
        if app is not None:
            app.focusChanged.connect(self._on_focus_changed)

        self.retranslate()

    def _form_label(self, key: str) -> QLabel:
        label = QLabel()
        self._form_labels[key] = label
        return label

    def retranslate(self) -> None:
        """Setzt alle Texte in der aktuellen Sprache (beim Aufbau und nach Sprachwechsel)."""
        self._title.setText(tr("settings.title", app=APP_NAME))
        self._version_label.setText(tr("settings.version", version=APP_VERSION))
        for key, label in self._form_labels.items():
            label.setText(f"{tr(key)}:")

        self._discord_box.setTitle(tr("section.discord"))
        self._discord_enabled.setText(tr("field.enabled"))
        self._client_id.setPlaceholderText(tr("discord.client_id_placeholder"))
        self._show_idle.setText(tr("discord.show_idle"))

        self._update_box.setTitle(tr("section.updates"))
        self._updates_enabled.setText(tr("field.enabled"))
        self._check_updates_btn.setText(tr("updates.check_now"))
        self._install_update_btn.setText(tr("updates.install"))

        self._audio_box.setTitle(tr("section.audio"))
        self._audio_hint.setText(tr("audio.hint", app=APP_NAME))
        self._reload_output_devices()  # "Systemstandard"/"nicht verfuegbar" neu beschriften

        self._general_box.setTitle(tr("section.general"))
        self._save_btn.setText(tr("settings.save"))

    def _fill_output_combo(self, combo: QComboBox, saved: str) -> None:
        """Fuellt eine Ausgabe-Auswahl: "Systemstandard" + alle vorhandenen Geraete. Ist das
        gespeicherte Geraet gerade nicht angeschlossen, bleibt es (als "nicht verfuegbar")
        ausgewaehlt, statt die Einstellung stillschweigend zu verlieren."""
        combo.blockSignals(True)
        combo.clear()
        combo.addItem(tr("audio.system_default"), "")
        names = list_output_devices()
        for name in names:
            combo.addItem(name, name)
        if saved and saved not in names:
            combo.addItem(tr("audio.unavailable", name=saved), saved)
        combo.setCurrentIndex(max(0, combo.findData(saved)))
        combo.blockSignals(False)

    def _reload_output_devices(self) -> None:
        for combo in (self._youtube_output, self._music_output):
            self._fill_output_combo(combo, combo.currentData() or "")

    def current_activity(self) -> str | None:
        """Zuletzt bearbeitetes Feld (None, solange noch kein Feld angeklickt wurde)."""
        if self._current_activity_key is None:
            return None
        first, field = self._current_activity_key
        if field is None:
            return tr(first)
        return tr("activity.editing", section=tr(first), field=tr(field))

    def _on_focus_changed(self, _old: QWidget | None, new: QWidget | None) -> None:
        # Vom fokussierten Widget nach oben suchen: bei QSpinBox/QComboBox liegt der Fokus
        # auf einem internen Kind-Widget (z.B. dem Textfeld des Spinners).
        widget = new
        while widget is not None and widget is not self:
            activity_key = self._activity_by_widget.get(widget)
            if activity_key:
                if activity_key != self._current_activity_key:
                    self._current_activity_key = activity_key
                    self.activityChanged.emit(self.current_activity() or "")
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

        self._config.setdefault("audio", {})
        self._config["audio"]["youtube_output"] = self._youtube_output.currentData() or ""
        self._config["audio"]["music_output"] = self._music_output.currentData() or ""

        self._config["start_tab"] = self._start_tab.currentData()

        language_changed = self._language.currentData() != get_language()
        self._config["language"] = self._language.currentData()

        save_config(self._config)
        # MainWindow stellt hier auch die Sprache um - die Meldung unten erscheint deshalb
        # schon in der neuen Sprache.
        self.settingsSaved.emit(self._config)
        text = tr("settings.saved_text")
        if language_changed:
            text += "\n\n" + tr("settings.saved_language", app=APP_NAME)
        QMessageBox.information(self, tr("settings.saved_title"), text)

    # ----------------------------------------------------- Update-Status (von MainWindow)

    def set_checking(self) -> None:
        self._check_updates_btn.setEnabled(False)
        self._install_update_btn.setVisible(False)
        self._update_progress.setVisible(False)
        self._update_status_label.setText(tr("updates.checking"))

    def set_check_done(self) -> None:
        self._check_updates_btn.setEnabled(True)

    def set_update_status(self, text: str) -> None:
        self._update_status_label.setText(text)

    def show_update_available(self, version: str) -> None:
        self._update_status_label.setText(tr("updates.available", version=version))
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
