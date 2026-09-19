"""Erstellt bei Bedarf eine Windows-Startmenue-Verknuepfung sowie die Dateizuordnung
fuer die eigene ".play"-Patchdateiendung.

Playtube-Setup (packaging/playtube.iss) legt Startmenue-Eintrag und Dateizuordnung
selbst an. Fuer eine portable, aus einem ZIP entpackte Kopie gaebe es sonst nie einen
Eintrag im Windows-Startmenue - der wird beim Start der gepackten .exe einmalig
nachgeholt (idempotent - prueft vorher, ob die Verknuepfung schon existiert). Eine durch
Setup installierte Kopie fasst die Verknuepfung nicht an, damit sie nicht versehentlich
wieder auf eine andere (portable) Kopie umgebogen wird."""
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
    # Import erst hier: shortcuts.py wird vor dem restlichen Qt-Setup importiert (siehe
    # main.py), updater.py zieht PySide6 nach.
    from .updater import is_installed_via_setup

    if is_installed_via_setup():
        return  # Verknuepfung gehoert dem Installer (siehe Moduldoku)
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


def ensure_play_file_association(app_name: str) -> None:
    """Registriert ".play" (unsere eigene Endung fuer Patch-Pakete, siehe updater.py)
    als Windows-Dateizuordnung fuer Playtube - ein manuell heruntergeladenes Patch kann
    danach per Doppelklick installiert werden (Playtube startet dann mit dem Dateipfad
    als Kommandozeilenargument, siehe main.py). Nur unter HKEY_CURRENT_USER, damit keine
    Admin-Rechte noetig sind. Wird bei jedem Start erneut geschrieben (billig, idempotent
    und heilt sich selbst, falls die .exe z.B. nach einem Update an einem neuen Pfad
    liegt)."""
    if sys.platform != "win32" or not getattr(sys, "frozen", False):
        return
    try:
        import winreg

        exe_path = str(Path(sys.executable).resolve())
        prog_id = f"{app_name}.PatchFile"
        icon_path = _find_icon(Path(exe_path).parent) or Path(exe_path)

        with winreg.CreateKey(winreg.HKEY_CURRENT_USER, r"Software\Classes\.play") as key:
            winreg.SetValueEx(key, "", 0, winreg.REG_SZ, prog_id)
        with winreg.CreateKey(winreg.HKEY_CURRENT_USER, rf"Software\Classes\{prog_id}") as key:
            winreg.SetValueEx(key, "", 0, winreg.REG_SZ, f"{app_name}-Patchdatei")
        with winreg.CreateKey(
            winreg.HKEY_CURRENT_USER, rf"Software\Classes\{prog_id}\DefaultIcon"
        ) as key:
            winreg.SetValueEx(key, "", 0, winreg.REG_SZ, str(icon_path))
        with winreg.CreateKey(
            winreg.HKEY_CURRENT_USER, rf"Software\Classes\{prog_id}\shell\open\command"
        ) as key:
            winreg.SetValueEx(key, "", 0, winreg.REG_SZ, f'"{exe_path}" "%1"')

        # Explorer informieren, damit die neue Zuordnung sofort (ohne Neustart) greift.
        import ctypes

        SHCNE_ASSOCCHANGED = 0x08000000
        SHCNF_IDLIST = 0x0000
        ctypes.windll.shell32.SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None)
    except Exception:
        # Keine Dateizuordnung ist kein Grund, den App-Start scheitern zu lassen.
        pass
