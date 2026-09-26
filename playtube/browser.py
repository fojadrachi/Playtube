"""Eingebetteter Browser-Tab (Microsoft Edge WebView2) fuer YouTube bzw. YouTube Music, mit
persistentem Login-Profil und periodischem Auslesen der aktuellen Wiedergabe fuer Discord Rich
Presence.

Edge-Variante von Playtube: statt QtWebEngine (kein AAC/H.264 - hochgeladene Titel liessen sich
nicht abspielen) rendert hier WebView2. Die Schnittstelle von BrowserTab ist dieselbe wie in der
Qt-Variante, damit MainWindow, Fernsteuerung und Discord unveraendert bleiben.
"""
from __future__ import annotations

import ctypes
import json
import os
from typing import Any, Callable

from PySide6.QtCore import Qt, QTimer, QUrl, Signal
from PySide6.QtWidgets import QWidget

from . import webview2_runtime
from .audio_routing import build_router_script
from .config import profile_dir
from .media_control import LIST_PLAYLISTS_JS, MUSIC_PLAYLIST_URL, build_control_js
from .media_probe import MEDIA_PROBE_JS, NEXT_TRACK_JS, PREV_TRACK_JS, TOGGLE_PLAYBACK_JS

# Nach einem Fernsteuerungsbefehl den Status kurz darauf neu auslesen (Seite braucht einen
# Moment, bis z.B. der neue Titel/Like-Status im DOM steht).
_REFRESH_AFTER_COMMAND_MS = 300
_TASK_POLL_MS = 25
_MEDIA_POLL_MS = 2000
_YOUTUBE_ORIGINS = ("https://www.youtube.com", "https://music.youtube.com")
# Ohne Nutzergeste starten koennen (Playlist-Start per Stream Dock, Autoplay).
_BROWSER_ARGUMENTS = "--autoplay-policy=no-user-gesture-required"

_user32 = ctypes.windll.user32 if os.name == "nt" else None
if _user32 is not None:
    # 64-Bit-sichere Signaturen (Fensterstil und Handles sind pointer-gross).
    _user32.GetWindowLongPtrW.restype = ctypes.c_ssize_t
    _user32.GetWindowLongPtrW.argtypes = (ctypes.c_void_p, ctypes.c_int)
    _user32.SetWindowLongPtrW.restype = ctypes.c_ssize_t
    _user32.SetWindowLongPtrW.argtypes = (ctypes.c_void_p, ctypes.c_int, ctypes.c_ssize_t)
    _user32.SetParent.argtypes = (ctypes.c_void_p, ctypes.c_void_p)
    _user32.MoveWindow.argtypes = (ctypes.c_void_p, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_bool)
    _user32.ShowWindow.argtypes = (ctypes.c_void_p, ctypes.c_int)

_GWL_STYLE = -16
_WS_POPUP = 0x80000000
_WS_CAPTION = 0x00C00000
_WS_CHILD = 0x40000000
_WS_VISIBLE = 0x10000000
_WS_CLIPSIBLINGS = 0x04000000
_WS_CLIPCHILDREN = 0x02000000
_SW_SHOW = 5


def _decode_script_result(raw: Any) -> Any:
    """ExecuteScriptAsync liefert das Ergebnis JSON-kodiert; unsere Skripte geben selbst
    einen JSON-Text zurueck, deshalb wird bei Bedarf ein zweites Mal dekodiert."""
    if raw is None:
        return None
    try:
        value = json.loads(str(raw))
    except (TypeError, ValueError):
        return None
    if isinstance(value, str):
        try:
            return json.loads(value)
        except ValueError:
            return value
    return value


def _in_document(source: str) -> str:
    """Umhuellt ein Skript, das document.documentElement braucht, damit es auch beim frueh
    (vor dem DOM) ausgefuehrten WebView2-Dokumentskript funktioniert."""
    return (
        "(function(){function go(){" + source + "}"
        "if(document.readyState==='loading'){document.addEventListener('DOMContentLoaded',go);}"
        "else{go();}})();"
    )


class BrowserTab(QWidget):
    """Ein WebView2-Tab mit Medien-Ueberwachung fuer Discord Rich Presence."""

    mediaInfoChanged = Signal(dict)
    titleUpdated = Signal(str)
    loadFinished = Signal(bool)
    urlChanged = Signal(QUrl)

    def __init__(self, home_url: str, parent=None):
        super().__init__(parent)
        self._home_url = home_url
        self._current_url = QUrl(home_url)
        self._audio_output_set = False
        self._allow_device_names = False
        self._router_script_id: str | None = None
        self._core = None
        self._pending: list[tuple[Any, Callable[[Any], None] | None]] = []
        self._queued_urls: list[str] = [home_url]
        self._queued_router: str | None = None

        # Natives Fenster-Handle, damit das WebView2-Steuerelement als Kindfenster einhaengt.
        self.setAttribute(Qt.WidgetAttribute.WA_NativeWindow, True)
        self.setMinimumSize(200, 150)

        runtime = webview2_runtime.load()
        self._runtime = runtime
        properties = runtime.CreationProperties()
        properties.UserDataFolder = str(profile_dir() / "webview2")
        properties.AdditionalBrowserArguments = _BROWSER_ARGUMENTS
        properties.Language = "de-DE"
        self._view = runtime.WebView2()
        self._view.CreationProperties = properties
        self._view.CoreWebView2InitializationCompleted += self._on_core_initialized

        self._task_timer = QTimer(self)
        self._task_timer.setInterval(_TASK_POLL_MS)
        self._task_timer.timeout.connect(self._drain_tasks)
        self._task_timer.start()

        self._poll_timer = QTimer(self)
        self._poll_timer.setInterval(_MEDIA_POLL_MS)
        self._poll_timer.timeout.connect(self._poll_media_state)
        self._poll_timer.start()

        # Steuerelement erst nach dem Aufbau des Fensters erzeugen.
        QTimer.singleShot(0, self._create_control)

    # ------------------------------------------------------------ Aufbau

    def _create_control(self) -> None:
        self._view.CreateControl()
        handle = self._handle()
        # Aus dem frei stehenden WinForms-Fenster ein sichtbares Kindfenster des Qt-Widgets machen.
        style = _user32.GetWindowLongPtrW(handle, _GWL_STYLE)
        style = (style & ~_WS_POPUP & ~_WS_CAPTION) | _WS_CHILD | _WS_VISIBLE | _WS_CLIPSIBLINGS | _WS_CLIPCHILDREN
        _user32.SetWindowLongPtrW(handle, _GWL_STYLE, style)
        _user32.SetParent(handle, int(self.winId()))
        _user32.ShowWindow(handle, _SW_SHOW)
        self._fit_to_widget()
        self._view.EnsureCoreWebView2Async(None)

    def _handle(self) -> int:
        return int(self._view.Handle.ToInt64())

    def _fit_to_widget(self) -> None:
        if _user32 is None or self._view is None or not self._view.IsHandleCreated:
            return
        ratio = self.devicePixelRatioF()
        _user32.MoveWindow(self._handle(), 0, 0, int(self.width() * ratio), int(self.height() * ratio), True)

    def resizeEvent(self, event) -> None:  # noqa: N802 (Qt-Override)
        super().resizeEvent(event)
        self._fit_to_widget()

    def _on_core_initialized(self, _sender, args) -> None:
        try:
            self._setup_core(args)
        except Exception:  # pythonnet verschluckt Fehler in Ereignis-Handlern sonst still
            import traceback
            traceback.print_exc()

    def _setup_core(self, args) -> None:
        if not args.IsSuccess:
            print(f"[webview2] Initialisierung fehlgeschlagen: {args.InitializationException}", flush=True)
            self.loadFinished.emit(False)
            return
        core = self._view.CoreWebView2
        self._core = core
        settings = core.Settings
        settings.IsStatusBarEnabled = False
        settings.AreDefaultScriptDialogsEnabled = True
        core.NavigationCompleted += lambda _s, a: self.loadFinished.emit(bool(a.IsSuccess))
        core.SourceChanged += lambda _s, _a: self._on_source_changed()
        core.DocumentTitleChanged += lambda _s, _a: self.titleUpdated.emit(str(core.DocumentTitle or ""))
        core.NewWindowRequested += self._on_new_window
        core.ContainsFullScreenElementChanged += lambda _s, _a: self._on_fullscreen_changed()

        self._apply_device_name_permission()
        if self._queued_router is not None:
            self._install_router(self._queued_router)
            self._queued_router = None
        for url in self._queued_urls:
            core.Navigate(url)
        self._queued_urls.clear()

    def _on_source_changed(self) -> None:
        self._current_url = QUrl(str(self._core.Source))
        self.urlChanged.emit(self._current_url)

    def _on_new_window(self, _sender, args) -> None:
        # Links mit target=_blank im selben Tab oeffnen (kein zusaetzliches Fenster).
        args.Handled = True
        self.setUrl(QUrl(str(args.Uri)))

    def _on_fullscreen_changed(self) -> None:
        window = self.window()
        if self._core.ContainsFullScreenElement:
            window.showFullScreen()
        else:
            window.showNormal()
        self._fit_to_widget()

    # ------------------------------------------------ asynchrone .NET-Aufgaben

    def _await(self, task: Any, callback: Callable[[Any], None] | None = None) -> None:
        """Wartet ohne Blockieren auf eine .NET-Task und ruft danach callback(Ergebnis) auf."""
        self._pending.append((task, callback))

    def _drain_tasks(self) -> None:
        if not self._pending:
            return
        still_pending = []
        for task, callback in self._pending:
            if not task.IsCompleted:
                still_pending.append((task, callback))
                continue
            if callback is None:
                continue
            try:
                callback(None if task.IsFaulted or task.IsCanceled else task.Result)
            except Exception:  # Fehler im Rueckruf duerfen die Warteschlange nicht blockieren
                pass
        self._pending = still_pending

    def _run_js(self, script: str, callback: Callable[[Any], None] | None = None) -> None:
        if self._core is None:
            return
        task = self._core.ExecuteScriptAsync(script)
        self._await(task, (lambda raw: callback(_decode_script_result(raw))) if callback else None)

    # ------------------------------------------------------------ Navigation

    def url(self) -> QUrl:
        return self._current_url

    def setUrl(self, url: QUrl) -> None:  # noqa: N802 (Qt-kompatibler Name)
        target = url.toString()
        self._current_url = url
        if self._core is None:
            self._queued_urls = [target]
        else:
            self._core.Navigate(target)

    def go_home(self) -> None:
        self.setUrl(QUrl(self._home_url))

    def back(self) -> None:
        if self._core is not None and self._core.CanGoBack:
            self._core.GoBack()

    def forward(self) -> None:
        if self._core is not None and self._core.CanGoForward:
            self._core.GoForward()

    def reload(self) -> None:
        if self._core is not None:
            self._core.Reload()

    # ------------------------------------------------------ Audio-Ausgabe

    def set_audio_output(self, device_name: str) -> None:
        """Legt den Ton dieses Tabs auf das Ausgabegeraet `device_name` (leer =
        Systemstandard), siehe audio_routing.py. Wirkt sofort auf die geoeffnete Seite und
        bleibt bei Seitenwechseln erhalten (Skript pro Dokument)."""
        source = build_router_script(device_name)
        if self._core is None:
            self._queued_router = source if device_name else None
        else:
            self._install_router(source if device_name else None)
            # Bereits geladene Seite sofort umstellen - auch zurueck auf den Systemstandard.
            if device_name or self._audio_output_set:
                self._run_js(source)
        self._audio_output_set = bool(device_name)

    def _install_router(self, source: str | None) -> None:
        old_id, self._router_script_id = self._router_script_id, None
        if old_id:
            self._core.RemoveScriptToExecuteOnDocumentCreated(old_id)
        if source:
            task = self._core.AddScriptToExecuteOnDocumentCreatedAsync(_in_document(source))
            self._await(task, self._remember_router_id)

    def _remember_router_id(self, script_id: Any) -> None:
        self._router_script_id = str(script_id) if script_id else None

    def set_device_names_allowed(self, allowed: bool) -> None:
        """Erlaubt der Seite, die Namen der Ausgabegeraete zu sehen (siehe audio_routing.py)."""
        self._allow_device_names = allowed
        self._apply_device_name_permission()

    def _apply_device_name_permission(self) -> None:
        if self._core is None:
            return
        state = self._runtime.PermissionState.Allow if self._allow_device_names else self._runtime.PermissionState.Default
        try:
            for origin in _YOUTUBE_ORIGINS:
                self._core.Profile.SetPermissionStateAsync(self._runtime.PermissionKind.Microphone, origin, state)
        except Exception as error:  # aeltere Runtime ohne diese API: Seite laedt trotzdem
            print(f"[webview2] Geraetenamen-Freigabe nicht setzbar: {error}", flush=True)

    # ----------------------------------------------------------- Wiedergabe

    def toggle_playback(self) -> None:
        self._run_js(TOGGLE_PLAYBACK_JS)

    def next_track(self) -> None:
        self._run_js(NEXT_TRACK_JS)

    def previous_track(self) -> None:
        self._run_js(PREV_TRACK_JS)

    def run_media_command(self, command: str, value: float = 0) -> None:
        """Fuehrt einen Fernsteuerungsbefehl (siehe media_control.py) aus und liest den
        Wiedergabestatus kurz danach neu aus, damit z.B. das Stream Dock den neuen Zustand
        sofort statt erst beim naechsten 2-Sekunden-Poll sieht."""
        self._run_js(build_control_js(command, value))
        QTimer.singleShot(_REFRESH_AFTER_COMMAND_MS, self._poll_media_state)

    def list_playlists(self, callback) -> None:
        """Ruft callback(list | None) mit den Playlists aus der Seitenleiste auf."""
        if self._core is None:
            callback(None)
            return
        self._run_js(LIST_PLAYLISTS_JS, lambda result: callback(result if isinstance(result, list) else None))

    def play_playlist(self, playlist_id: str) -> None:
        """Oeffnet die Playlist (ID vorher in remote_control validiert); Autoplay startet
        die Wiedergabe."""
        self.setUrl(QUrl(MUSIC_PLAYLIST_URL.format(playlist_id=playlist_id)))

    def _poll_media_state(self) -> None:
        self._run_js(MEDIA_PROBE_JS, self._on_media_probe_result)

    def _on_media_probe_result(self, info: Any) -> None:
        if os.environ.get("PLAYTUBE_DEBUG"):
            print(f"[media-probe:{self._home_url}] geparst={info!r}", flush=True)
        if isinstance(info, dict):
            self.mediaInfoChanged.emit(info)

    # ----------------------------------------------------------- Aufraeumen

    def closeEvent(self, event) -> None:  # noqa: N802 (Qt-Override)
        self.dispose()
        super().closeEvent(event)

    def dispose(self) -> None:
        self._task_timer.stop()
        self._poll_timer.stop()
        if self._view is not None:
            self._view.Dispose()
            self._view = None
