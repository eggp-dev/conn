#!/bin/sh
# Install the Conn desktop app with one line.
#
#   curl -fsSL https://raw.githubusercontent.com/eggp-dev/conn/main/scripts/install.sh | sh
#
# Linux x86_64: downloads the AppImage of the newest release, verifies it against the release's
# SHA256SUMS, and installs it for the current user only (no sudo). The AppImage updates itself from
# inside the app. macOS: use Homebrew (printed below). Nothing is installed system-wide.
#
#   CONN_VERSION=v0.8.1   install that release instead of the newest one
#   CONN_BIN_DIR=DIR      where the AppImage goes (default ~/.local/bin)
#   CONN_REPO=owner/name  the repository to install from, if it has moved
set -eu

REPO="${CONN_REPO:-eggp-dev/conn}"
BIN_DIR="${CONN_BIN_DIR:-$HOME/.local/bin}"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"

say() { printf '%s\n' "$*"; }
die() { printf 'conn install: %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" > /dev/null 2>&1 || die "'$1' is required"; }

os="$(uname -s)"; arch="$(uname -m)"
case "$os" in
  Darwin)
    [ "$arch" = arm64 ] || die "macOS builds are Apple Silicon only for now. See https://github.com/$REPO/releases"
    say "On macOS install Conn with Homebrew:"; say ""; say "  brew install --cask eggp-dev/tap/conn"; say ""
    say "Or download the signed and notarized DMG: https://github.com/$REPO/releases"; exit 0 ;;
  Linux) [ "$arch" = x86_64 ] || die "Linux builds are x86_64 only for now (this is $arch). See https://github.com/$REPO/releases" ;;
  *) die "unsupported system '$os'. Windows: download the installer from https://github.com/$REPO/releases" ;;
esac
need curl; need sha256sum

tag="${CONN_VERSION:-}"
if [ -z "$tag" ]; then
  # Conn's releases are marked as previews, which "releases/latest" skips, so take the newest published one.
  tag="$(curl -fsSL "https://api.github.com/repos/$REPO/releases?per_page=10" | sed -n 's/^ *"tag_name": *"\(v[0-9][^"]*\)".*/\1/p' | head -n 1)"
  [ -n "$tag" ] || die "could not find a published release of $REPO"
fi
case "$tag" in v[0-9]*) ;; *) die "CONN_VERSION must look like v0.8.1 (got '$tag')" ;; esac

asset="conn-$tag-x86_64-unknown-linux-gnu-desktop.AppImage"
base="https://github.com/$REPO/releases/download/$tag"
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT INT TERM

say "Conn $tag: downloading $asset"
curl -fL --progress-bar -o "$tmp/$asset" "$base/$asset" || die "download failed: $base/$asset"
curl -fsSL -o "$tmp/SHA256SUMS" "$base/SHA256SUMS" || die "could not download SHA256SUMS for $tag"
( cd "$tmp" && grep " $asset\$" SHA256SUMS > expected && sha256sum -c expected > /dev/null 2>&1 ) || die "checksum mismatch for $asset; nothing was installed"
say "Checksum verified against the release's SHA256SUMS."

mkdir -p "$BIN_DIR" "$DATA_DIR/applications" "$DATA_DIR/icons/hicolor/scalable/apps"
install -m 755 "$tmp/$asset" "$BIN_DIR/conn-desktop.AppImage"
ln -sf "$BIN_DIR/conn-desktop.AppImage" "$BIN_DIR/conn-desktop"
curl -fsSL -o "$DATA_DIR/icons/hicolor/scalable/apps/conn.svg" "https://raw.githubusercontent.com/$REPO/$tag/frontends/tauri/public/conn-icon.svg" 2> /dev/null || true
cat > "$DATA_DIR/applications/conn.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Conn
Comment=A terminal you share with your AI agent
Exec=$BIN_DIR/conn-desktop.AppImage
Icon=conn
Terminal=false
Categories=Development;TerminalEmulator;
StartupWMClass=conn-desktop
DESKTOP
command -v update-desktop-database > /dev/null 2>&1 && update-desktop-database "$DATA_DIR/applications" > /dev/null 2>&1 || true

say ""
say "Installed: $BIN_DIR/conn-desktop.AppImage (also in your app menu as Conn)"
case ":$PATH:" in *":$BIN_DIR:"*) say "Start it with: conn-desktop" ;; *) say "Start it with: $BIN_DIR/conn-desktop   ($BIN_DIR is not on your PATH)" ;; esac
if ! { ldconfig -p 2> /dev/null | grep -q 'libfuse\.so\.2'; }; then
  say ""
  say "Note: AppImages need FUSE 2, which this system does not have yet:"
  say "  sudo apt install libfuse2t64        (Ubuntu 24.04 and newer; 'libfuse2' on older releases)"
fi
say ""
say "Next: open Conn, then Settings -> Agents -> Set up. Updates install from inside the app."
say "Remove: rm $BIN_DIR/conn-desktop $BIN_DIR/conn-desktop.AppImage $DATA_DIR/applications/conn.desktop"
