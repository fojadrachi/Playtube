"""Laden/Speichern der Benutzerkonfiguration (config.json im Projekt- bzw. App-Datenordner)."""
from __future__ import annotations

import json
import os
import shutil
import sys
from pathlib import Path
from typing import Any

APP_NAME = "Playtube"
APP_AUMID = "Playtube.DesktopClient"  # Windows AppUserModelID

# Der Entwicklungsmodus (python main.py) benutzt einen eigenen APPDATA-Ordner
# ("PlaytubeDev" statt "Playtube"), damit Login-Profil und Config sich NIE mit einer
# gepackten/installierten Playtube.exe ueberschneiden (frueher fuehrte das dazu, dass
# ein lokaler Test-Build und die echte Installation sich dieselbe config.json bzw.
# denselben Browser-Profil-Lock geteilt haben).
_DATA_DIR_NAME = APP_NAME if getattr(sys, "frozen", False) else f"{APP_NAME}Dev"

DEFAULT_CONFIG: dict[str, Any] = {
    "app_name": APP_NAME,
    "discord": {
        "enabled": True,
        # Deine Discord Application Client-ID (discord.com/developers/applications).
        "client_id": "1548023494976086127",
        "update_interval_seconds": 15,
        # Wenn nichts laeuft: Idle-Status anzeigen statt Presence komplett zu leeren.
        "show_idle_presence": True,
    },
    "updates": {
        "enabled": True,
        "check_interval_hours": 6,
    },
    # Ausgabegeraet pro Tab (Name des Geraets, wie in den Einstellungen gewaehlt; leer =
    # Systemstandard) - siehe playtube/audio_routing.py.
    "audio": {
        "youtube_output": "",
        "music_output": "",
    },
    "start_tab": "youtube",  # "youtube" oder "music"
    "home_youtube": "https://www.youtube.com/",
    "home_music": "https://music.youtube.com/",
    "window": {"width": 1366, "height": 860},
}


def _app_data_dir() -> Path:
    """Ordner fuer persistente Daten (Login-Profil, Config) - auch im gepackten .exe
    stabil. Entwicklungsmodus und gepackte .exe nutzen bewusst unterschiedliche
    Ordner (siehe _DATA_DIR_NAME)."""
    base = os.environ.get("APPDATA") or str(Path.home())
    d = Path(base) / _DATA_DIR_NAME
    d.mkdir(parents=True, exist_ok=True)
    return d


def app_data_dir() -> Path:
    """Oeffentlicher Zugriff auf den App-Datenordner, z.B. fuer Debug-Logs - die
    gepackte .exe laeuft ohne Konsolenfenster (console=False), print()-Debugging ist
    dort also unsichtbar; ein Log-File ist die einzige Moeglichkeit, dort etwas
    nachtraeglich einzusehen."""
    return _app_data_dir()


def _config_path() -> Path:
    # Im Entwicklungsmodus liegt config.json direkt im Projektordner (leicht editierbar),
    # im gepackten Build im APPDATA-Ordner.
    if getattr(sys, "frozen", False):
        return _app_data_dir() / "config.json"
    return Path(__file__).resolve().parent.parent / "config.json"


def _deep_merge(base: dict, override: dict) -> dict:
    result = dict(base)
    for key, value in override.items():
        if isinstance(value, dict) and isinstance(result.get(key), dict):
            result[key] = _deep_merge(result[key], value)
        else:
            result[key] = value
    return result


def load_config() -> dict[str, Any]:
    path = _config_path()
    if path.exists():
        try:
            with open(path, "r", encoding="utf-8") as f:
                user_cfg = json.load(f)
            return _deep_merge(DEFAULT_CONFIG, user_cfg)
        except (json.JSONDecodeError, OSError):
            pass
    save_config(DEFAULT_CONFIG)
    return dict(DEFAULT_CONFIG)


def save_config(cfg: dict[str, Any]) -> None:
    path = _config_path()
    try:
        with open(path, "w", encoding="utf-8") as f:
            json.dump(cfg, f, indent=2, ensure_ascii=False)
    except OSError:
        pass


def profile_dir() -> Path:
    """Ordner fuer das persistente Browser-Profil (Login-Session, Cookies, Local Storage)."""
    d = _app_data_dir() / "webprofile"
    d.mkdir(parents=True, exist_ok=True)
    return d


def clear_cache_on_update(current_version: str) -> None:
    """Loescht den QtWebEngine-HTTP-Cache (webprofile/cache), wenn seit dem letzten
    Start ein Update installiert wurde - der Login (Cookies/LocalStorage liegen in
    webprofile/storage, einem komplett getrennten Ordner) bleibt dabei unangetastet.
    Alte Cache-Eintraege (z.B. Icon-Sprites, Skripte) koennen nach einem Update nicht
    mehr zum neuen Code passen - frueher Ursache fuer fehlende Icons nach einem
    beschaedigten Cache, siehe README."""
    marker = _app_data_dir() / "installed_version.txt"
    previous = None
    if marker.exists():
        try:
            previous = marker.read_text(encoding="utf-8").strip()
        except OSError:
            previous = None

    if previous != current_version:
        cache_dir = profile_dir() / "cache"
        if cache_dir.exists():
            shutil.rmtree(cache_dir, ignore_errors=True)
        try:
            marker.write_text(current_version, encoding="utf-8")
        except OSError:
            pass
