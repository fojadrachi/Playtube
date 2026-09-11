# PyInstaller-Spec fuer Playtube.
# Bauen mit:  pyinstaller packaging/playtube.spec --noconfirm
# (aus dem Projekt-Root ausfuehren, oder packaging/build.ps1 benutzen)
import sys
from pathlib import Path

block_cipher = None

ROOT = Path(SPECPATH).resolve().parent

a = Analysis(
    [str(ROOT / "main.py")],
    pathex=[str(ROOT)],
    binaries=[],
    datas=[
        (str(ROOT / "assets"), "assets"),
        (str(ROOT / "config.json"), "."),
    ],
    hiddenimports=["pypresence"],
    hookspath=[],
    runtime_hooks=[],
    excludes=[],
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name="Playtube",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    console=False,
    icon=str(ROOT / "assets" / "icon.ico"),
    version=str(ROOT / "packaging" / "version_info.txt"),
)

coll = COLLECT(
    exe,
    a.binaries,
    a.zipfiles,
    a.datas,
    strip=False,
    upx=False,
    name="Playtube",
)
