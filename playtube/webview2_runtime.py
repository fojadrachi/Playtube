"""Laedt Microsoft Edge WebView2 (per pythonnet / .NET Framework) fuer den Browser-Tab.

Die DLLs (Microsoft.Web.WebView2.Core/WinForms, WebView2Loader) liegen im Projektordner
vendor/webview2 (offizielles NuGet-Paket "Microsoft.Web.WebView2"). Die eigentliche
Browser-Engine ist die auf Windows 10/11 vorinstallierte "WebView2 Runtime" (Edge) - sie
bringt AAC/H.264 mit, die in QtWebEngine fehlen.
"""
from __future__ import annotations

import os
import sys
from functools import lru_cache
from pathlib import Path
from types import SimpleNamespace


class WebView2Unavailable(RuntimeError):
    """WebView2 konnte nicht geladen werden (DLLs oder Runtime fehlen)."""


def _app_base_dir() -> Path:
    return Path(getattr(sys, "_MEIPASS", Path(__file__).resolve().parent.parent))


def _vendor_dir() -> Path:
    return _app_base_dir() / "vendor" / "webview2"


def _unblock_downloaded_files(folders: tuple[Path, ...]) -> None:
    """Entfernt die "Mark of the Web" (Zone.Identifier) von den DLLs, die .NET laden soll.

    Wer die ZIP aus dem Internet laedt und entpackt, bekommt alle Dateien mit dieser
    Markierung; .NET verweigert dann das Laden von Python.Runtime.dll ("Failed to resolve
    Python.Runtime.Loader.Initialize"). Die Dateien liegen im eigenen App-Ordner - das
    Entfernen der Markierung betrifft nur sie."""
    if os.name != "nt":
        return
    for folder in folders:
        for path in folder.rglob("*"):
            if path.suffix.lower() in (".dll", ".exe"):
                try:
                    os.remove(f"{path}:Zone.Identifier")
                except OSError:
                    pass  # keine Markierung vorhanden (Normalfall) oder Dateisystem ohne Datenstroeme


@lru_cache(maxsize=1)
def load() -> SimpleNamespace:
    """Laedt .NET + WebView2 einmalig und gibt die benoetigten Klassen zurueck."""
    vendor = _vendor_dir()
    core_dll = vendor / "Microsoft.Web.WebView2.Core.dll"
    winforms_dll = vendor / "Microsoft.Web.WebView2.WinForms.dll"
    if not core_dll.exists() or not winforms_dll.exists():
        raise WebView2Unavailable(f"WebView2-DLLs fehlen in {vendor}")

    base = _app_base_dir()
    _unblock_downloaded_files((vendor, base / "pythonnet", base / "clr_loader"))

    # WebView2Loader.dll (nativ) wird ueber den Suchpfad bzw. runtimes/win-x64/native gefunden.
    os.environ["PATH"] = f"{vendor}{os.pathsep}{os.environ.get('PATH', '')}"
    try:
        from pythonnet import load as load_runtime

        load_runtime("netfx")
        import clr

        clr.AddReference("System.Windows.Forms")
        clr.AddReference("System.Drawing")
        clr.AddReference(str(core_dll))
        clr.AddReference(str(winforms_dll))

        from Microsoft.Web.WebView2.Core import (
            CoreWebView2PermissionKind,
            CoreWebView2PermissionState,
        )
        from Microsoft.Web.WebView2.WinForms import CoreWebView2CreationProperties, WebView2
    except Exception as error:  # pythonnet wirft je nach Fehler unterschiedliche Typen
        raise WebView2Unavailable(f"WebView2 konnte nicht geladen werden: {error}") from error

    return SimpleNamespace(
        WebView2=WebView2,
        CreationProperties=CoreWebView2CreationProperties,
        PermissionKind=CoreWebView2PermissionKind,
        PermissionState=CoreWebView2PermissionState,
    )
