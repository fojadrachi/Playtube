"""Erstellt bei Bedarf eine Windows-Startmenue-Verknuepfung.

Playtube wird als portables ZIP ausgeliefert (kein MSI/Installer) - ohne das gaebe es
also nie einen Eintrag im Windows-Startmenue, wie man ihn von "richtig installierten"
Programmen kennt. Wird beim Start der gepackten .exe einmalig nachgeholt (idempotent -
prueft vorher, ob die Verknuepfung schon existiert)."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path


def _find_icon(exe_dir: Path) -> Path | None:
    matches = list(exe_dir.rglob("icon.ico"))
    return matches[0] if matches else None


def ensure_start_menu_shortcut(app_name: str) -> None:
    if sys.platform != "win32" or not getattr(sys, "frozen", False):
        return
    try:
        appdata = os.environ.get("APPDATA")
        if not appdata:
            return
        start_menu = Path(appdata) / "Microsoft" / "Windows" / "Start Menu" / "Programs"
        shortcut_path = start_menu / f"{app_name}.lnk"
        if shortcut_path.exists():
            return

        exe_path = Path(sys.executable).resolve()
        icon_path = _find_icon(exe_path.parent) or exe_path

        ps_script = (
            '$WshShell = New-Object -ComObject WScript.Shell\n'
            f'$Shortcut = $WshShell.CreateShortcut("{shortcut_path}")\n'
            f'$Shortcut.TargetPath = "{exe_path}"\n'
            f'$Shortcut.WorkingDirectory = "{exe_path.parent}"\n'
            f'$Shortcut.IconLocation = "{icon_path}"\n'
            f'$Shortcut.Description = "{app_name}"\n'
            '$Shortcut.Save()\n'
        )
        subprocess.run(
            ["powershell", "-NoProfile", "-WindowStyle", "Hidden", "-Command", ps_script],
            capture_output=True,
            timeout=15,
            creationflags=subprocess.CREATE_NO_WINDOW,
        )
    except Exception:
        # Kein Startmenue-Eintrag ist kein Grund, den App-Start scheitern zu lassen.
        pass
