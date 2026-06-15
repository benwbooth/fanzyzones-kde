#!/bin/sh
# FanzyZones KDE installer.
#
#   curl -fsSL https://raw.githubusercontent.com/benwbooth/fanzyzones-kde/main/install.sh | sh
#
# Downloads the latest self-contained static binary and installs the Plasma
# applet + KWin script (the binary embeds them and registers them itself).
set -eu

REPO="benwbooth/fanzyzones-kde"
ASSET="fanzyzones-kde-x86_64-linux"
BIN_DIR="${XDG_BIN_HOME:-$HOME/.local/bin}"
BIN="$BIN_DIR/fanzyzones-kde"
URL="https://github.com/$REPO/releases/latest/download/$ASSET"

arch="$(uname -m)"
if [ "$arch" != "x86_64" ]; then
    echo "fanzyzones-kde: only x86_64 prebuilt binaries are published (got '$arch')." >&2
    echo "Build from source instead: https://github.com/$REPO#from-source" >&2
    exit 1
fi

if ! command -v kpackagetool6 >/dev/null 2>&1; then
    echo "Warning: 'kpackagetool6' not found — FanzyZones needs KDE Plasma 6." >&2
fi

echo "Downloading $ASSET ..."
mkdir -p "$BIN_DIR"
if command -v curl >/dev/null 2>&1; then
    curl -fSL "$URL" -o "$BIN"
elif command -v wget >/dev/null 2>&1; then
    wget -O "$BIN" "$URL"
else
    echo "Need 'curl' or 'wget' to download." >&2
    exit 1
fi
chmod +x "$BIN"

echo "Installing the Plasma applet + KWin script ..."
"$BIN" install --reload

cat <<EOF

FanzyZones KDE is installed and added to your system tray automatically.
  Binary:  $BIN
  (If you don't see the tray icon, right-click the tray -> Configure System
  Tray and enable "FanzyZones".)

Re-run this script any time to update. If '$BIN_DIR' is not on your PATH and you
want to run 'fanzyzones-kde' directly, add it to your shell profile.
EOF
