#!/usr/bin/env bash
# Baut die Linux-Version von Playtube (x86_64) und packt sie als .deb und AppImage nach dist/.
#   ./build-linux.sh            (Release-Build + Pakete)
# Voraussetzungen (Debian/Ubuntu): libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev
#   libxdo-dev libfontconfig1-dev libxkbcommon-dev dpkg-dev; für das AppImage lädt das Skript
#   appimagetool herunter (oder APPIMAGETOOL=/pfad/zu/appimagetool setzen).
set -euo pipefail
cd "$(dirname "$0")"

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
DIST=dist
mkdir -p "$DIST"

cargo build --release --locked
BIN=${CARGO_TARGET_DIR:-target}/release/playtube

# ------------------------------------------------------------------ .deb
PKG=$(mktemp -d)
install -Dm755 "$BIN" "$PKG/usr/bin/playtube"
install -Dm644 assets/linux/playtube.desktop "$PKG/usr/share/applications/playtube.desktop"
install -Dm644 assets/icon.png "$PKG/usr/share/icons/hicolor/256x256/apps/playtube.png"
install -Dm644 assets/linux/MaterialIcons-LICENSE.txt "$PKG/usr/share/doc/playtube/MaterialIcons-LICENSE.txt"
SIZE=$(du -sk "$PKG/usr" | cut -f1)
mkdir -p "$PKG/DEBIAN"
cat > "$PKG/DEBIAN/control" <<CONTROL
Package: playtube
Version: $VERSION
Architecture: amd64
Maintainer: Fojadrachi
Installed-Size: $SIZE
Depends: libwebkit2gtk-4.1-0, libgtk-3-0, libxkbcommon-x11-0, libfontconfig1
Recommends: libayatana-appindicator3-1, pulseaudio-utils, gstreamer1.0-plugins-good, gstreamer1.0-libav
Section: sound
Priority: optional
Homepage: https://github.com/fojadrachi/Playtube
Description: YouTube und YouTube Music als eigene Desktop-App
 Playtube zeigt YouTube und YouTube Music in einem eigenen Fenster, mit
 Tray-Symbol, Playtube Jam, Discord-Status und Steuerung vom Handy.
CONTROL
dpkg-deb --root-owner-group --build "$PKG" "$DIST/playtube_${VERSION}_amd64.deb"
rm -rf "$PKG"

# ------------------------------------------------------------------ AppImage
# Schlankes AppImage: Programm, Icon und .desktop-Datei; WebKitGTK/GTK kommen vom System
# (wie beim .deb), damit Grafiktreiber und Codecs des Systems benutzt werden.
APPDIR=$(mktemp -d)/Playtube.AppDir
install -Dm755 "$BIN" "$APPDIR/usr/bin/playtube"
install -Dm644 assets/linux/playtube.desktop "$APPDIR/usr/share/applications/playtube.desktop"
install -Dm644 assets/icon.png "$APPDIR/usr/share/icons/hicolor/256x256/apps/playtube.png"
cp assets/linux/playtube.desktop "$APPDIR/playtube.desktop"
cp assets/icon.png "$APPDIR/playtube.png"
cat > "$APPDIR/AppRun" <<'APPRUN'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/playtube" "$@"
APPRUN
chmod +x "$APPDIR/AppRun"

TOOL=${APPIMAGETOOL:-}
if [ -z "$TOOL" ]; then
  TOOL=$(mktemp -d)/appimagetool
  curl -fsSL -o "$TOOL" https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
  chmod +x "$TOOL"
fi
OUT="$DIST/Playtube-${VERSION}-x86_64.AppImage"
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$TOOL" --no-appstream "$APPDIR" "$OUT"
chmod +x "$OUT"
ls -la "$DIST"
