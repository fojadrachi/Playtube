"""Eingebetteter Browser-Tab fuer YouTube bzw. YouTube Music, mit persistentem Login-
Profil (eigener Datenordner, kein System-Browser-Profil) und periodischem Auslesen
der aktuellen Wiedergabe fuer Discord Rich Presence."""
from __future__ import annotations

import json
import os
import sys

from PySide6.QtCore import QTimer, Signal, QUrl
from PySide6.QtWebEngineCore import (
    QWebEnginePage,
    QWebEngineProfile,
    QWebEngineScript,
    QWebEngineSettings,
    QWebEngineUrlRequestInterceptor,
)
from PySide6.QtWebEngineWidgets import QWebEngineView

from .audio_routing import SCRIPT_NAME as AUDIO_SCRIPT_NAME
from .audio_routing import build_router_script
from .chrome_shim import CHROME_FULL, CHROME_MAJOR, CHROME_SHIM_JS
from .config import profile_dir
from .media_control import LIST_PLAYLISTS_JS, MUSIC_PLAYLIST_URL, build_control_js
from .media_probe import MEDIA_PROBE_JS, NEXT_TRACK_JS, PREV_TRACK_JS, TOGGLE_PLAYBACK_JS

# Nach einem Fernsteuerungsbefehl den Status kurz darauf neu auslesen (Seite braucht einen
# Moment, bis z.B. der neue Titel/Like-Status im DOM steht).
_REFRESH_AFTER_COMMAND_MS = 300

# Chrome-Versionsnummer, die exakt zur tatsaechlich in QtWebEngine eingebetteten
# Chromium-Version passt (siehe QWebEngineCore.qWebEngineChromiumVersion()). Legacy-
# User-Agent, die "Sec-CH-UA" Client-Hints (Header) UND navigator.userAgentData (JS,
# siehe chrome_shim.py) muessen konsistent dieselbe Version + Marke ("Google Chrome")
# melden - sonst erkennt Google-Login den Browser als nicht vertrauenswuerdiges
# Embedded-WebView und blockiert die Anmeldung mit "Dieser Browser oder diese App ist
# unter Umstaenden nicht sicher".
_CHROME_VERSION = CHROME_FULL
_CHROME_MAJOR = CHROME_MAJOR

if sys.platform == "win32":
    _UA_PLATFORM_TOKEN = "Windows NT 10.0; Win64; x64"
    _SEC_CH_UA_PLATFORM = b'"Windows"'
    _SEC_CH_UA_PLATFORM_VERSION = b'"15.0.0"'
elif sys.platform.startswith("linux"):
    _UA_PLATFORM_TOKEN = "X11; Linux x86_64"
    _SEC_CH_UA_PLATFORM = b'"Linux"'
    _SEC_CH_UA_PLATFORM_VERSION = b'"6.0.0"'
else:
    _UA_PLATFORM_TOKEN = "Macintosh; Intel Mac OS X 10_15_7"
    _SEC_CH_UA_PLATFORM = b'"macOS"'
    _SEC_CH_UA_PLATFORM_VERSION = b'"14.0.0"'

_USER_AGENT = (
    f"Mozilla/5.0 ({_UA_PLATFORM_TOKEN}) AppleWebKit/537.36 "
    f"(KHTML, like Gecko) Chrome/{_CHROME_VERSION} Safari/537.36"
)

_SEC_CH_UA = (
    f'"Not(A:Brand";v="99", "Google Chrome";v="{_CHROME_MAJOR}", '
    f'"Chromium";v="{_CHROME_MAJOR}"'
).encode("ascii")
_SEC_CH_UA_FULL_VERSION_LIST = (
    f'"Not(A:Brand";v="99.0.0.0", "Google Chrome";v="{_CHROME_VERSION}", '
    f'"Chromium";v="{_CHROME_VERSION}"'
).encode("ascii")


class _ChromeBrandingInterceptor(QWebEngineUrlRequestInterceptor):
    """Ergaenzt/korrigiert die Sec-CH-UA Client-Hint-Header auf jeder Anfrage, damit
    sie zum gesetzten User-Agent passen (echtes QtWebEngine meldet dort normalerweise
    nur "Chromium", ohne die Marke "Google Chrome" - genau daran erkennt Googles
    Login-Seite ein Embedded-WebView und blockiert die Anmeldung)."""

    def interceptRequest(self, info) -> None:  # noqa: N802 (Qt-Override)
        info.setHttpHeader(b"sec-ch-ua", _SEC_CH_UA)
        info.setHttpHeader(b"sec-ch-ua-full-version-list", _SEC_CH_UA_FULL_VERSION_LIST)
        info.setHttpHeader(b"sec-ch-ua-mobile", b"?0")
        info.setHttpHeader(b"sec-ch-ua-platform", _SEC_CH_UA_PLATFORM)
        info.setHttpHeader(b"sec-ch-ua-platform-version", _SEC_CH_UA_PLATFORM_VERSION)


_shared_profile: QWebEngineProfile | None = None
_shared_interceptor: _ChromeBrandingInterceptor | None = None


def get_shared_profile() -> QWebEngineProfile:
    """Ein gemeinsames, persistentes Profil fuer alle Tabs, damit ein einmaliges
    Google-Login fuer YouTube und YouTube Music gleichermassen gilt."""
    global _shared_profile, _shared_interceptor
    if _shared_profile is None:
        _shared_profile = QWebEngineProfile("playtube-profile")
        _shared_profile.setPersistentStoragePath(str(profile_dir() / "storage"))
        _shared_profile.setCachePath(str(profile_dir() / "cache"))
        _shared_profile.setPersistentCookiesPolicy(
            QWebEngineProfile.PersistentCookiesPolicy.ForcePersistentCookies
        )
        _shared_profile.setHttpUserAgent(_USER_AGENT)
        _shared_profile.setHttpAcceptLanguage("de-DE,de;q=0.9,en-US;q=0.8,en;q=0.7")

        # Referenz muss gehalten werden, sonst sammelt Python das Objekt vorzeitig ein.
        _shared_interceptor = _ChromeBrandingInterceptor()
        _shared_profile.setUrlRequestInterceptor(_shared_interceptor)

        # JS-seitiger "Chrome-Shim" (window.chrome, navigator.userAgentData, Plugins) -
        # muss vor jedem Seiten-JS laufen, daher DocumentCreation + MainWorld.
        shim_script = QWebEngineScript()
        shim_script.setName("playtube-chrome-shim")
        shim_script.setInjectionPoint(QWebEngineScript.InjectionPoint.DocumentCreation)
        shim_script.setWorldId(QWebEngineScript.ScriptWorldId.MainWorld)
        shim_script.setRunsOnSubFrames(True)
        shim_script.setSourceCode(CHROME_SHIM_JS)
        _shared_profile.scripts().insert(shim_script)
    return _shared_profile


class BrowserTab(QWebEngineView):
    """Ein WebEngine-Tab mit Medien-Ueberwachung fuer Discord Rich Presence."""

    mediaInfoChanged = Signal(dict)
    titleUpdated = Signal(str)

    def __init__(self, home_url: str, parent=None):
        super().__init__(parent)
        self._home_url = home_url
        self._audio_output_set = False  # war schon ein eigenes Ausgabegeraet gewaehlt? (siehe set_audio_output)

        page = QWebEnginePage(get_shared_profile(), self)
        self.setPage(page)

        settings = page.settings()
        settings.setAttribute(QWebEngineSettings.WebAttribute.JavascriptEnabled, True)
        settings.setAttribute(QWebEngineSettings.WebAttribute.PlaybackRequiresUserGesture, False)
        settings.setAttribute(QWebEngineSettings.WebAttribute.FullScreenSupportEnabled, True)
        settings.setAttribute(QWebEngineSettings.WebAttribute.LocalStorageEnabled, True)
        settings.setAttribute(QWebEngineSettings.WebAttribute.ScreenCaptureEnabled, True)
        settings.setAttribute(QWebEngineSettings.WebAttribute.JavascriptCanOpenWindows, True)

        page.fullScreenRequested.connect(self._on_full_screen_requested)

        self.load(QUrl(home_url))

        self._poll_timer = QTimer(self)
        self._poll_timer.setInterval(2000)
        self._poll_timer.timeout.connect(self._poll_media_state)
        self._poll_timer.start()

        self.titleChanged.connect(self.titleUpdated.emit)

    def go_home(self) -> None:
        self.load(QUrl(self._home_url))

    def set_audio_output(self, device_name: str) -> None:
        """Legt den Ton dieses Tabs auf das Ausgabegeraet `device_name` (leer =
        Systemstandard), siehe audio_routing.py. Wirkt sofort auf die geoeffnete Seite und
        bleibt bei Seitenwechseln/Neuladen erhalten (Skript pro Seite)."""
        page = self.page()
        scripts = page.scripts()
        for old in scripts.find(AUDIO_SCRIPT_NAME):
            scripts.remove(old)

        source = build_router_script(device_name)
        if device_name:
            script = QWebEngineScript()
            script.setName(AUDIO_SCRIPT_NAME)
            script.setInjectionPoint(QWebEngineScript.InjectionPoint.DocumentReady)
            script.setWorldId(QWebEngineScript.ScriptWorldId.MainWorld)
            script.setRunsOnSubFrames(False)
            script.setSourceCode(source)
            scripts.insert(script)

        # Bereits geladene Seite sofort umstellen - auch zurueck auf den Systemstandard,
        # falls vorher ein eigenes Geraet gewaehlt war. Ohne jemals gewaehltes Geraet wird die
        # Seite gar nicht angefasst.
        if device_name or self._audio_output_set:
            page.runJavaScript(source)
        self._audio_output_set = bool(device_name)

    def toggle_playback(self) -> None:
        self.page().runJavaScript(TOGGLE_PLAYBACK_JS)

    def next_track(self) -> None:
        self.page().runJavaScript(NEXT_TRACK_JS)

    def previous_track(self) -> None:
        self.page().runJavaScript(PREV_TRACK_JS)

    def run_media_command(self, command: str, value: float = 0) -> None:
        """Fuehrt einen Fernsteuerungsbefehl (siehe media_control.py) aus und liest den
        Wiedergabestatus kurz danach neu aus, damit z.B. das Stream Dock den neuen Zustand
        sofort statt erst beim naechsten 2-Sekunden-Poll sieht."""
        self.page().runJavaScript(build_control_js(command, value))
        QTimer.singleShot(_REFRESH_AFTER_COMMAND_MS, self._poll_media_state)

    def list_playlists(self, callback) -> None:
        """Ruft callback(list | None) mit den Playlists aus der Seitenleiste auf."""

        def on_result(result) -> None:
            try:
                callback(json.loads(result) if isinstance(result, str) and result else None)
            except json.JSONDecodeError:
                callback(None)

        self.page().runJavaScript(LIST_PLAYLISTS_JS, on_result)

    def play_playlist(self, playlist_id: str) -> None:
        """Oeffnet die Playlist (ID vorher in remote_control validiert); Autoplay startet
        die Wiedergabe (PlaybackRequiresUserGesture ist aus)."""
        self.load(QUrl(MUSIC_PLAYLIST_URL.format(playlist_id=playlist_id)))

    def _on_full_screen_requested(self, request) -> None:
        # Erlaubt echtes Fullscreen-Video (z.B. per YouTube-Fullscreen-Button).
        request.accept()
        window = self.window()
        if request.toggleOn():
            window.showFullScreen()
        else:
            window.showNormal()

    def _poll_media_state(self) -> None:
        self.page().runJavaScript(MEDIA_PROBE_JS, self._on_media_probe_result)

    def _on_media_probe_result(self, result) -> None:
        # runJavaScript() liefert JS-Objekte ueber diese Bruecke nicht zuverlaessig
        # als dict (siehe media_probe.py) - das Probe-Skript gibt daher einen
        # JSON-String zurueck, der hier geparst wird.
        info = None
        if isinstance(result, str) and result:
            try:
                info = json.loads(result)
            except json.JSONDecodeError:
                info = None
        elif isinstance(result, dict):
            info = result

        if os.environ.get("PLAYTUBE_DEBUG"):
            print(f"[media-probe:{self._home_url}] roh={result!r} geparst={info!r}", flush=True)

        if isinstance(info, dict):
            self.mediaInfoChanged.emit(info)
