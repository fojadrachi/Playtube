"""Auto-Update ueber GitHub Releases (https://github.com/fojadrachi/Playtube).

Ablauf:
  1. UpdateChecker prueft im Hintergrund die GitHub-Releases-API auf eine neuere
     Version als die aktuell laufende (playtube.__version__).
  2. Bei Fund fragt die UI (siehe mainwindow.py) nach Bestaetigung.
  3. UpdateInstaller installiert das Update:
       - Gepackte Playtube.exe: laedt das Windows-Release-Zip herunter, entpackt es
         und laesst ein kurzes PowerShell-Skript (nach Prozessende) den Installations-
         ordner per robocopy /MIR ersetzen und die App neu starten.
       - Entwicklungsmodus (python main.py): fuehrt 'git pull' + 'pip install -r
         requirements.txt' aus, die App startet sich danach selbst neu (os.execv).
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
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


def _find_windows_zip_asset(release: dict[str, Any]) -> dict[str, Any] | None:
    assets = release.get("assets", [])
    for asset in assets:
        name = asset.get("name", "").lower()
        if name.endswith(".zip") and ("win" in name or "windows" in name):
            return asset
    for asset in assets:
        if asset.get("name", "").lower().endswith(".zip"):
            return asset
    return None


class UpdateChecker(QThread):
    """Prueft einmalig im Hintergrund auf eine neue Version."""

    updateAvailable = Signal(str, str, str)  # version, release_notes, zip_download_url
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
        asset = _find_windows_zip_asset(release)
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
                "Kein Windows-Release-Paket (.zip) im neuesten Release gefunden."
            )
            return

        self.progress.emit("Lade Update herunter …")
        install_dir = Path(sys.executable).resolve().parent
        staging = Path(tempfile.mkdtemp(prefix="playtube_update_"))
        zip_path = staging / "update.zip"
        extract_dir = staging / "extracted"

        req = urllib.request.Request(self._download_url, headers={"User-Agent": _USER_AGENT})
        with urllib.request.urlopen(req, timeout=120) as resp, open(zip_path, "wb") as out:
            shutil.copyfileobj(resp, out)

        self.progress.emit("Entpacke Update …")
        with zipfile.ZipFile(zip_path) as zf:
            zf.extractall(extract_dir)

        # Manche Release-Zips enthalten einen einzelnen Unterordner (z.B. "Playtube/").
        entries = list(extract_dir.iterdir())
        source_dir = entries[0] if len(entries) == 1 and entries[0].is_dir() else extract_dir

        self.progress.emit("Bereite Installation vor …")
        script_path = self._write_apply_script(source_dir, install_dir, staging)
        subprocess.Popen(
            ["powershell", "-WindowStyle", "Hidden", "-ExecutionPolicy", "Bypass", "-File", str(script_path)],
            creationflags=subprocess.CREATE_NO_WINDOW,
        )
        self.finished_ok.emit()

    def _write_apply_script(self, source_dir: Path, install_dir: Path, staging: Path) -> Path:
        exe_path = install_dir / "Playtube.exe"
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
        return script_path

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
