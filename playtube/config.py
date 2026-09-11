"""Laden/Speichern der Benutzerkonfiguration (config.json im Projekt- bzw. App-Datenordner)."""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path
from typing import Any

APP_NAME = "Playtube"
APP_AUMID = "Playtube.DesktopClient"  # Windows AppUserModelID

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
    "start_tab": "youtube",  # "youtube" oder "music"
    "home_youtube": "https://www.youtube.com/",
    "home_music": "https://music.youtube.com/",
    "window": {"width": 1366, "height": 860},
}


def _app_data_dir() -> Path:
    """Ordner fuer persistente Daten (Login-Profil, Config) - auch im gepackten .exe stabil."""
    base = os.environ.get("APPDATA") or str(Path.home())
    d = Path(base) / APP_NAME
    d.mkdir(parents=True, exist_ok=True)
    return d


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
