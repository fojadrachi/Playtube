"""Auto-Update ueber GitHub Releases (https://github.com/fojadrachi/Playtube).

Ablauf:
  1. UpdateChecker prueft im Hintergrund die GitHub-Releases-API auf eine neuere
     Version als die aktuell laufende (playtube.__version__).
  2. Bei Fund fragt die UI (siehe mainwindow.py) nach Bestaetigung.
  3. UpdateInstaller installiert das Update:
       - Gepackte App (Windows Playtube.exe oder Linux Playtube-Binary): laedt
         bevorzugt das kleine "Patch"-Paket herunter (enthaelt NUR die eigentliche
         .exe/Binary mit unserem Anwendungscode, ca. 2-3 MB statt ~200 MB) und
         ersetzt lediglich diese Datei - der riesige PySide6/QtWebEngine-Laufzeit-
         Ordner (_internal/) bleibt unangetastet, da er sich zwischen Patch-Releases
         normalerweise nicht aendert. Gibt es kein Patch-Paket (z.B. beim allerersten
         Release oder wenn CI es nicht gebaut hat), faellt es automatisch auf das
         volle Release-Paket zurueck und ersetzt den kompletten Installationsordner.
         Ein kurzes Skript wartet dafuer (nach Prozessende) und startet die App neu -
         PowerShell unter Windows, ein Shell-Skript unter Linux.
       - Entwicklungsmodus (python main.py): fuehrt 'git pull' + 'pip install -r
         requirements.txt' aus, die App startet sich danach selbst neu (os.execv).
"""
from __future__ import annotations

import json
import os
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
    """Sucht das zur laufenden Plattform passende Release-Paket. Bevorzugt das kleine
    "-patch"-Paket (nur die .exe/Binary) gegenueber dem vollen Release-Paket - siehe
    Moduldoku. Windows -> *.zip mit "win" im Namen, Linux -> *.tar.gz mit "linux"."""
    assets = release.get("assets", [])
    if sys.platform == "win32":
        hints, exts = ("win",), (".zip",)
    elif sys.platform.startswith("linux"):
        hints, exts = ("linux",), (".tar.gz", ".tgz")
    else:
        return None

    def find(want_patch: bool, require_hint: bool) -> dict[str, Any] | None:
        for asset in assets:
            name = asset.get("name", "").lower()
            if not name.endswith(exts):
                continue
            if require_hint and not any(h in name for h in hints):
                continue
            if ("patch" in name) != want_patch:
                continue
            return asset
        return None

    return (
        find(True, True)
        or find(True, False)
        or find(False, True)
        or find(False, False)
    )


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
    # Download-Fortschritt in Prozent (0-100); -1 = unbestimmt (z.B. waehrend
    # Entpacken/Installieren, wo sich kein Prozentsatz sinnvoll angeben laesst).
    progress_percent = Signal(int)
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
        self.progress_percent.emit(0)
        install_dir = Path(sys.executable).resolve().parent
        staging = Path(tempfile.mkdtemp(prefix="playtube_update_"))
        archive_name = self._download_url.rsplit("/", 1)[-1]
        archive_path = staging / archive_name
        extract_dir = staging / "extracted"

        req = urllib.request.Request(self._download_url, headers={"User-Agent": _USER_AGENT})
        with urllib.request.urlopen(req, timeout=120) as resp:
            total = int(resp.headers.get("Content-Length") or 0)
            downloaded = 0
            last_emitted = -1
            with open(archive_path, "wb") as out:
                while True:
                    chunk = resp.read(256 * 1024)
                    if not chunk:
                        break
                    out.write(chunk)
                    downloaded += len(chunk)
                    if total:
                        percent = int(downloaded * 100 / total)
                        if percent != last_emitted:
                            self.progress_percent.emit(percent)
                            last_emitted = percent

        self.progress_percent.emit(100)
        self.progress.emit("Entpacke Update …")
        # Unbestimmter Fortschritt waehrend Entpacken/Installieren - die UI zeigt
        # dafuer einen "laufenden" Balken statt einer Prozentzahl.
        self.progress_percent.emit(-1)
        if archive_name.endswith((".tar.gz", ".tgz")):
            with tarfile.open(archive_path) as tf:
                tf.extractall(extract_dir)
        else:
            with zipfile.ZipFile(archive_path) as zf:
                zf.extractall(extract_dir)

        is_patch = "patch" in archive_name.lower()
        self.progress.emit("Bereite Installation vor …")

        if is_patch:
            binary_name = _WINDOWS_BINARY_NAME if sys.platform == "win32" else _LINUX_BINARY_NAME
            matches = list(extract_dir.rglob(binary_name))
            if not matches:
                # Unerwartet leeres/falsches Patch-Paket -> nicht kaputt installieren,
                # lieber sauber fehlschlagen und den Nutzer auf das volle Paket
                # hinweisen (naechster Check faellt automatisch darauf zurueck).
                self.failed.emit(
                    f"Patch-Paket enthielt kein '{binary_name}'. Bitte erneut versuchen."
                )
                return
            if sys.platform == "win32":
                self._install_patch_windows(matches[0], install_dir, staging)
            else:
                self._install_patch_posix(matches[0], install_dir, staging)
        else:
            # Release-Archive enthalten meist einen einzelnen Unterordner (z.B. "Playtube/").
            entries = list(extract_dir.iterdir())
            source_dir = entries[0] if len(entries) == 1 and entries[0].is_dir() else extract_dir
            if sys.platform == "win32":
                self._install_full_windows(source_dir, install_dir, staging)
            else:
                self._install_full_posix(source_dir, install_dir, staging)

        self.finished_ok.emit()

    # Kleines, immer sichtbares Fortschrittsfenster fuer den Windows-Installationsschritt.
    # Der Hauptprozess ist zu diesem Zeitpunkt schon beendet (Datei-Sperren!), daher
    # laeuft das komplett im separaten PowerShell-Skript - ohne dieses Fenster wuerde
    # der Nutzer nach dem Schliessen der App fuer die Dauer der Installation (bei
    # einem vollen Paket ggf. mehrere Sekunden) gar nichts sehen, was wie ein Absturz
    # oder Haenger wirkt.
    _WINDOWS_PROGRESS_FORM_PS = """
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$form = New-Object System.Windows.Forms.Form
$form.Text = "Playtube-Update"
$form.Size = New-Object System.Drawing.Size(380,130)
$form.StartPosition = "CenterScreen"
$form.FormBorderStyle = "FixedDialog"
$form.ControlBox = $false
$form.TopMost = $true
$label = New-Object System.Windows.Forms.Label
$label.Text = "Playtube wird aktualisiert – bitte warten …"
$label.AutoSize = $false
$label.Size = New-Object System.Drawing.Size(340,20)
$label.Location = New-Object System.Drawing.Point(20,15)
$form.Controls.Add($label)
$bar = New-Object System.Windows.Forms.ProgressBar
$bar.Style = "Marquee"
$bar.MarqueeAnimationSpeed = 30
$bar.Size = New-Object System.Drawing.Size(340,20)
$bar.Location = New-Object System.Drawing.Point(20,50)
$form.Controls.Add($bar)
$form.Show()
$form.Refresh()
"""

    # -- Patch (nur .exe/Binary tauschen, _internal/ bleibt unangetastet) --

    def _install_patch_windows(self, new_exe: Path, install_dir: Path, staging: Path) -> None:
        exe_path = install_dir / _WINDOWS_BINARY_NAME
        script = self._WINDOWS_PROGRESS_FORM_PS + f"""
$ErrorActionPreference = "SilentlyContinue"
$targetPid = {os.getpid()}
while (Get-Process -Id $targetPid -ErrorAction SilentlyContinue) {{
    Start-Sleep -Milliseconds 300
    [System.Windows.Forms.Application]::DoEvents()
}}
$label.Text = "Kopiere aktualisierte Datei …"
$form.Refresh()
[System.Windows.Forms.Application]::DoEvents()
Copy-Item -Path "{new_exe}" -Destination "{exe_path}" -Force
$label.Text = "Fertig – Playtube wird neu gestartet …"
$form.Refresh()
[System.Windows.Forms.Application]::DoEvents()
Start-Process -FilePath "{exe_path}"
Start-Sleep -Milliseconds 800
$form.Close()
Remove-Item -Recurse -Force "{staging}" -ErrorAction SilentlyContinue
"""
        self._spawn_windows_script(script, staging)

    # Falls zenity installiert ist (auf den meisten Desktop-Distros vorhanden), waehrend
    # Wartezeit/Installation ein pulsierendes Fortschrittsfenster zeigen - rein optisch,
    # das Update funktioniert auch ohne (dann passiert der Neustart einfach unsichtbar).
    _POSIX_PROGRESS_HEADER = """#!/bin/sh
ZPID=""
if command -v zenity >/dev/null 2>&1; then
    tail -f /dev/null | zenity --progress --title="Playtube-Update" \
        --text="Playtube wird aktualisiert - bitte warten ..." --pulsate --no-cancel \
        >/dev/null 2>&1 &
    ZPID=$!
fi
"""
    _POSIX_PROGRESS_FOOTER = """
[ -n "$ZPID" ] && kill "$ZPID" 2>/dev/null
"""

    def _install_patch_posix(self, new_binary: Path, install_dir: Path, staging: Path) -> None:
        exe_path = install_dir / _LINUX_BINARY_NAME
        script = self._POSIX_PROGRESS_HEADER + f"""
while kill -0 {os.getpid()} 2>/dev/null; do
    sleep 0.5
done
cp -f "{new_binary}" "{exe_path}"
chmod +x "{exe_path}"
""" + self._POSIX_PROGRESS_FOOTER + f"""
nohup "{exe_path}" >/dev/null 2>&1 &
rm -rf "{staging}"
"""
        self._spawn_posix_script(script, staging)

    # -- Volles Paket (kein Patch verfuegbar/passend) - kompletten Ordner ersetzen --

    def _install_full_windows(self, source_dir: Path, install_dir: Path, staging: Path) -> None:
        exe_path = install_dir / _WINDOWS_BINARY_NAME
        script = self._WINDOWS_PROGRESS_FORM_PS + f"""
$ErrorActionPreference = "SilentlyContinue"
$targetPid = {os.getpid()}
while (Get-Process -Id $targetPid -ErrorAction SilentlyContinue) {{
    Start-Sleep -Milliseconds 300
    [System.Windows.Forms.Application]::DoEvents()
}}
$label.Text = "Kopiere Programmdateien … (kann etwas dauern)"
$form.Refresh()
[System.Windows.Forms.Application]::DoEvents()
# Ueber Start-Process (statt direktem Aufruf) gestartet und per Polling statt -Wait
# abgewartet, damit die Fensternachrichtenschleife per DoEvents() weiterlaeuft -
# sonst wuerde Windows das Fenster waehrend robocopy als "Keine Rueckmeldung" anzeigen.
$roboArgs = @("{source_dir}", "{install_dir}", "/MIR", "/NFL", "/NDL", "/NJH", "/NJS", "/NC", "/NS", "/NP")
$roboProc = Start-Process -FilePath "robocopy" -ArgumentList $roboArgs -WindowStyle Hidden -PassThru
while (-not $roboProc.HasExited) {{
    Start-Sleep -Milliseconds 200
    [System.Windows.Forms.Application]::DoEvents()
}}
$label.Text = "Fertig – Playtube wird neu gestartet …"
$form.Refresh()
[System.Windows.Forms.Application]::DoEvents()
Start-Process -FilePath "{exe_path}"
Start-Sleep -Milliseconds 800
$form.Close()
Remove-Item -Recurse -Force "{staging}" -ErrorAction SilentlyContinue
"""
        self._spawn_windows_script(script, staging)

    def _install_full_posix(self, source_dir: Path, install_dir: Path, staging: Path) -> None:
        exe_path = install_dir / _LINUX_BINARY_NAME
        script = self._POSIX_PROGRESS_HEADER + f"""
while kill -0 {os.getpid()} 2>/dev/null; do
    sleep 0.5
done
rm -rf "{install_dir}"/*
cp -a "{source_dir}"/. "{install_dir}"/
chmod +x "{exe_path}"
""" + self._POSIX_PROGRESS_FOOTER + f"""
nohup "{exe_path}" >/dev/null 2>&1 &
rm -rf "{staging}"
"""
        self._spawn_posix_script(script, staging)

    # -- Skript-Ausfuehrung: PowerShell (wartet+robocopy/Copy) unter Windows, Shell
    #    (wartet+cp) unter Linux; wird detached gestartet, damit es diesen Prozess
    #    ueberlebt, wenn dieser sich gleich beendet. --

    def _spawn_windows_script(self, script: str, staging: Path) -> None:
        script_path = staging / "apply_update.ps1"
        script_path.write_text(script, encoding="utf-8")
        subprocess.Popen(
            ["powershell", "-WindowStyle", "Hidden", "-ExecutionPolicy", "Bypass", "-File", str(script_path)],
            creationflags=subprocess.CREATE_NO_WINDOW,
        )

    def _spawn_posix_script(self, script: str, staging: Path) -> None:
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
