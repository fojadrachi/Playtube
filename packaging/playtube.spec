# PyInstaller-Spec fuer Playtube.
# Bauen mit:  pyinstaller packaging/playtube.spec --noconfirm
# (aus dem Projekt-Root ausfuehren, oder packaging/build.ps1 benutzen)
import sys
from pathlib import Path

from PyInstaller.utils.hooks import collect_all

block_cipher = None

ROOT = Path(SPECPATH).resolve().parent

# Edge-Variante: pythonnet + clr_loader (bringen Python.Runtime.dll/ClrLoader.dll mit) und die
# Microsoft-WebView2-DLLs aus vendor/webview2 (siehe playtube/webview2_runtime.py).
pythonnet_datas, pythonnet_binaries, pythonnet_hidden = collect_all("pythonnet")
clr_datas, clr_binaries, clr_hidden = collect_all("clr_loader")

a = Analysis(
    [str(ROOT / "main.py")],
    pathex=[str(ROOT)],
    binaries=pythonnet_binaries + clr_binaries,
    datas=[
        (str(ROOT / "assets"), "assets"),
        (str(ROOT / "config.json"), "."),
        (str(ROOT / "vendor" / "webview2"), "vendor/webview2"),
    ] + pythonnet_datas + clr_datas,
    hiddenimports=["pypresence", "clr"] + pythonnet_hidden + clr_hidden,
    hookspath=[],
    runtime_hooks=[],
    # Kein QtWebEngine mehr (WebView2 statt dessen) - haelt das Paket klein.
    excludes=["PySide6.QtWebEngineCore", "PySide6.QtWebEngineWidgets", "PySide6.QtWebEngineQuick"],
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

# icon/version-Ressourcen sind Windows-spezifisch (.ico + Versionsressource) - unter
# Linux/macOS gibt es diese Konzepte fuer ELF-Binaries nicht, deshalb nur dort setzen.
exe_kwargs = dict(
    exclude_binaries=True,
    name="PlaytubeEdge",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    console=False,
)
if sys.platform == "win32":
    exe_kwargs["icon"] = str(ROOT / "assets" / "icon.ico")
    exe_kwargs["version"] = str(ROOT / "packaging" / "version_info.txt")

exe = EXE(pyz, a.scripts, [], **exe_kwargs)

coll = COLLECT(
    exe,
    a.binaries,
    a.zipfiles,
    a.datas,
    strip=False,
    upx=False,
    name="PlaytubeEdge",
)
