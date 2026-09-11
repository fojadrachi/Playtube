"""Auto-Update ueber GitHub Releases (https://github.com/fojadrachi/Playtube).

Ablauf:
  1. UpdateChecker prueft im Hintergrund die GitHub-Releases-API auf eine neuere
     Version als die aktuell laufende (playtube.__version__).
  2. Bei Fund fragt die UI (siehe mainwindow.py) nach Bestaetigung.
  3. UpdateInstaller installiert das Update:
       - Gepackte App (Windows Playtube.exe oder Linux Playtube-Binary): laedt das
         zum laufenden Betriebssystem passende Release-Paket herunter, entpackt es
         und laesst ein kurzes Skript (nach Prozessende) den Installationsordner
         ersetzen und die App neu starten - PowerShell+robocopy unter Windows,
         ein Shell-Skript unter Linux.
       - Entwicklungsmodus (python main.py): fuehrt 'git pull' + 'pip install -r
         requirements.txt' aus, die App startet sich danach selbst neu (os.execv).
"""
from __future__ import annotations

import json
import os
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import urllib.error
import urllib.request
import zipfile
from pathlib import Path
from typing import Any

from PySide6.QtCore import QThread, Signal

from . import __version__ as CURRENT_VERSION

GITHUB_REPO = "fojadrachi/Playtube"
_API_URL = f"https://api.github.com/repos/{GITHUB_REPO}/releases/latest"
_USER_AGENT = "Playtube-Updater"

_LINUX_BINARY_NAME = "Playtube"
_WINDOWS_BINARY_NAME = "Playtube.exe"


def _parse_version(v: str) -> tuple[int, ...]:
    v = v.strip().lstrip("vV")
    parts: list[int] = []
    for p in v.split("."):
        digits = "".join(ch for ch in p if ch.isdigit())
        parts.append(int(digits) if digits else 0)
    return tuple(parts) or (0,)


def is_newer(remote: str, local: str = CURRENT_VERSION) -> bool:
    return _parse_version(remote) > _parse_version(local)


def fetch_latest_release() -> dict[str, Any] | None:
    req = urllib.request.Request(
        _API_URL, headers={"User-Agent": _USER_AGENT, "Accept": "application/vnd.github+json"}
    )
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except (urllib.error.URLError, TimeoutError, json.JSONDecodeError, OSError, ValueError):
        return None


def _find_platform_asset(release: dict[str, Any]) -> dict[str, Any] | None:
    """Sucht das zur laufenden Plattform passende Release-Paket:
    Windows -> *.zip mit "win" im Namen, Linux -> *.tar.gz mit "linux" im Namen."""
    assets = release.get("assets", [])
    if sys.platform == "win32":
        hints, exts = ("win",), (".zip",)
    elif sys.platform.startswith("linux"):
        hints, exts = ("linux",), (".tar.gz", ".tgz")
    else:
        return None

    for asset in assets:
        name = asset.get("name", "").lower()
        if name.endswith(exts) and any(h in name for h in hints):
            return asset
    for asset in assets:
        if asset.get("name", "").lower().endswith(exts):
            return asset
    return None


class UpdateChecker(QThread):
    """Prueft einmalig im Hintergrund auf eine neue Version."""

    updateAvailable = Signal(str, str, str)  # version, release_notes, download_url
    checkFailed = Signal()
    upToDate = Signal()

    def run(self) -> None:
        release = fetch_latest_release()
        if not release:
            self.checkFailed.emit()
            return
        tag = release.get("tag_name") or release.get("name") or ""
        if not tag or not is_newer(tag):
            self.upToDate.emit()
            return
        asset = _find_platform_asset(release)
        download_url = asset["browser_download_url"] if asset else ""
        notes = (release.get("body") or "").strip()
        self.updateAvailable.emit(tag, notes, download_url)


class UpdateInstaller(QThread):
    """Laedt ein Release herunter und installiert es (siehe Moduldoku)."""

    progress = Signal(str)
    finished_ok = Signal()
    failed = Signal(str)

    def __init__(self, download_url: str, parent=None):
        super().__init__(parent)
        self._download_url = download_url

    def run(self) -> None:
        try:
            if getattr(sys, "frozen", False):
                self._run_packaged_update()
            else:
                self._run_dev_update()
        except Exception as exc:  # noqa: BLE001 - Fehler soll in der UI landen, nicht crashen
            self.failed.emit(str(exc))

    # ---------------------------------------------------------- gepackter Modus

    def _run_packaged_update(self) -> None:
        if not self._download_url:
            self.failed.emit(
                "Kein passendes Release-Paket fuer dieses Betriebssystem gefunden."
            )
            return

        self.progress.emit("Lade Update herunter …")
        install_dir = Path(sys.executable).resolve().parent
        staging = Path(tempfile.mkdtemp(prefix="playtube_update_"))
        archive_name = self._download_url.rsplit("/", 1)[-1]
        archive_path = staging / archive_name
        extract_dir = staging / "extracted"

        req = urllib.request.Request(self._download_url, headers={"User-Agent": _USER_AGENT})
        with urllib.request.urlopen(req, timeout=120) as resp, open(archive_path, "wb") as out:
            shutil.copyfileobj(resp, out)

        self.progress.emit("Entpacke Update …")
        if archive_name.endswith((".tar.gz", ".tgz")):
            with tarfile.open(archive_path) as tf:
                tf.extractall(extract_dir)
        else:
            with zipfile.ZipFile(archive_path) as zf:
                zf.extractall(extract_dir)

        # Release-Archive enthalten meist einen einzelnen Unterordner (z.B. "Playtube/").
        entries = list(extract_dir.iterdir())
        source_dir = entries[0] if len(entries) == 1 and entries[0].is_dir() else extract_dir

        self.progress.emit("Bereite Installation vor …")
        if sys.platform == "win32":
            self._install_windows(source_dir, install_dir, staging)
        else:
            self._install_posix(source_dir, install_dir, staging)
        self.finished_ok.emit()

    # -- Windows: PowerShell-Skript wartet auf Prozessende, kopiert per robocopy --

    def _install_windows(self, source_dir: Path, install_dir: Path, staging: Path) -> None:
        exe_path = install_dir / _WINDOWS_BINARY_NAME
        script = f"""
$ErrorActionPreference = "SilentlyContinue"
Start-Sleep -Seconds 1
$targetPid = {os.getpid()}
while (Get-Process -Id $targetPid -ErrorAction SilentlyContinue) {{
    Start-Sleep -Milliseconds 500
}}
robocopy "{source_dir}" "{install_dir}" /MIR /NFL /NDL /NJH /NJS /NC /NS /NP | Out-Null
Start-Process -FilePath "{exe_path}"
Start-Sleep -Seconds 2
Remove-Item -Recurse -Force "{staging}" -ErrorAction SilentlyContinue
"""
        script_path = staging / "apply_update.ps1"
        script_path.write_text(script, encoding="utf-8")
        subprocess.Popen(
            ["powershell", "-WindowStyle", "Hidden", "-ExecutionPolicy", "Bypass", "-File", str(script_path)],
            creationflags=subprocess.CREATE_NO_WINDOW,
        )

    # -- Linux: Shell-Skript wartet auf Prozessende, kopiert per cp -a --

    def _install_posix(self, source_dir: Path, install_dir: Path, staging: Path) -> None:
        exe_path = install_dir / _LINUX_BINARY_NAME
        script = f"""#!/bin/sh
while kill -0 {os.getpid()} 2>/dev/null; do
    sleep 0.5
done
rm -rf "{install_dir}"/*
cp -a "{source_dir}"/. "{install_dir}"/
chmod +x "{exe_path}"
nohup "{exe_path}" >/dev/null 2>&1 &
rm -rf "{staging}"
"""
        script_path = staging / "apply_update.sh"
        script_path.write_text(script, encoding="utf-8")
        script_path.chmod(script_path.stat().st_mode | stat.S_IEXEC | stat.S_IXGRP | stat.S_IXOTH)
        subprocess.Popen(
            ["/bin/sh", str(script_path)],
            start_new_session=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

    # ------------------------------------------------------------- Entwicklungsmodus

    def _run_dev_update(self) -> None:
        project_root = Path(__file__).resolve().parent.parent
        if not (project_root / ".git").exists():
            self.failed.emit(
                "Kein Git-Repository gefunden - im Entwicklungsmodus bitte manuell "
                "'git pull' im Projektordner ausfuehren."
            )
            return

        self.progress.emit("Lade Aenderungen per git pull …")
        result = subprocess.run(
            ["git", "pull", "--ff-only"], cwd=project_root, capture_output=True, text=True
        )
        if result.returncode != 0:
            self.failed.emit(f"git pull fehlgeschlagen:\n{result.stderr.strip()}")
            return

        self.progress.emit("Installiere Abhaengigkeiten …")
        pip_result = subprocess.run(
            [sys.executable, "-m", "pip", "install", "-q", "-r", str(project_root / "requirements.txt")],
            capture_output=True,
            text=True,
        )
        if pip_result.returncode != 0:
            self.failed.emit(f"pip install fehlgeschlagen:\n{pip_result.stderr.strip()}")
            return

        self.finished_ok.emit()
