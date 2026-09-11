#!/bin/sh
# Installiert eine entpackte Playtube-Linux-Version (aus dem Playtube-vX.Y.Z-linux-x86_64.tar.gz
# Release-Paket) als "richtige" Desktop-App: eigener Ordner unter ~/.local/share/Playtube,
# Startmenue-Eintrag mit Icon, Kommandozeilen-Befehl "playtube".
#
# Aufruf: im entpackten Ordner (der die Datei "Playtube" enthaelt) ausfuehren:
#   sh install-linux.sh

set -e

SRC_DIR="$(cd "$(dirname "$0")" && pwd)"
INSTALL_DIR="$HOME/.local/share/Playtube"
BIN_DIR="$HOME/.local/bin"
DESKTOP_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor/512x512/apps"

if [ ! -f "$SRC_DIR/Playtube" ]; then
    echo "Fehler: $SRC_DIR/Playtube nicht gefunden - bitte im entpackten Release-Ordner ausfuehren." >&2
    exit 1
fi

echo "==> Installiere nach $INSTALL_DIR ..."
mkdir -p "$INSTALL_DIR"
cp -a "$SRC_DIR"/. "$INSTALL_DIR"/
chmod +x "$INSTALL_DIR/Playtube"

mkdir -p "$BIN_DIR"
ln -sf "$INSTALL_DIR/Playtube" "$BIN_DIR/playtube"

mkdir -p "$ICON_DIR"
if [ -f "$INSTALL_DIR/assets/icon.png" ]; then
    cp -f "$INSTALL_DIR/assets/icon.png" "$ICON_DIR/playtube.png"
fi

mkdir -p "$DESKTOP_DIR"
cat > "$DESKTOP_DIR/playtube.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Playtube
Comment=YouTube & YouTube Music mit Discord Rich Presence
Exec=$INSTALL_DIR/Playtube
Icon=playtube
Terminal=false
Categories=AudioVideo;Audio;Video;Network;
StartupWMClass=Playtube
EOF

update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true
gtk-update-icon-cache "$HOME/.local/share/icons/hicolor" 2>/dev/null || true

echo ""
echo "Fertig! Playtube ist jetzt im Anwendungsmenue verfuegbar,"
echo "oder direkt per Terminal-Befehl 'playtube' startbar"
echo "(ggf. $BIN_DIR zum PATH hinzufuegen, falls das nicht klappt)."
