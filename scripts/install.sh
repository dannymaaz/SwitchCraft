#!/usr/bin/env bash
set -euo pipefail

REPO="dannymaaz/SwitchCraft"
APP_NAME="SwitchCraft"

echo ""
echo "  ⚡ $APP_NAME Installer"
echo "  ─────────────────────────"
echo ""

# Detect OS and architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux*)
    case "$ARCH" in
      x86_64)  ASSET_PATTERN=".AppImage";;
      aarch64) ASSET_PATTERN="_aarch64.AppImage";;
      *)       echo "Unsupported architecture: $ARCH"; exit 1;;
    esac
    ;;
  Darwin*)
    case "$ARCH" in
      x86_64)  ASSET_PATTERN="_x64.dmg";;
      arm64)   ASSET_PATTERN="_aarch64.dmg";;
      *)       echo "Unsupported architecture: $ARCH"; exit 1;;
    esac
    ;;
  *)
    echo "Unsupported OS: $OS"
    echo "For Windows, use: irm https://raw.githubusercontent.com/$REPO/main/scripts/install.ps1 | iex"
    exit 1
    ;;
esac

echo "  Platform: $OS ($ARCH)"
echo ""

# Get latest release URL
RELEASE_URL="https://api.github.com/repos/$REPO/releases/latest"
echo "  Fetching latest release..."

DOWNLOAD_URL=$(curl -s "$RELEASE_URL" \
  | grep "browser_download_url" \
  | grep "$ASSET_PATTERN" \
  | head -1 \
  | cut -d '"' -f 4)

if [ -z "$DOWNLOAD_URL" ]; then
  echo "  Could not find a download for your platform."
  echo "  Visit https://github.com/$REPO/releases for manual download."
  exit 1
fi

FILENAME=$(basename "$DOWNLOAD_URL")
echo "  Downloading $FILENAME..."

curl -L -o "/tmp/$FILENAME" "$DOWNLOAD_URL"

case "$OS" in
  Linux*)
    INSTALL_DIR="${HOME}/.local/bin"
    mkdir -p "$INSTALL_DIR"
    mv "/tmp/$FILENAME" "$INSTALL_DIR/$APP_NAME"
    chmod +x "$INSTALL_DIR/$APP_NAME"
    echo ""
    echo "  Installed to $INSTALL_DIR/$APP_NAME"
    echo ""
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
      echo "  Add this to your shell profile:"
      echo "    export PATH=\"\$HOME/.local/bin:\$PATH\""
      echo ""
    fi
    echo "  Run with: $APP_NAME"
    ;;
  Darwin*)
    echo "  Mounting disk image..."
    hdiutil attach "/tmp/$FILENAME" -quiet
    VOLUME=$(ls /Volumes | grep -i "$APP_NAME" | head -1)
    if [ -n "$VOLUME" ]; then
      cp -R "/Volumes/$VOLUME/$APP_NAME.app" /Applications/ 2>/dev/null || true
      hdiutil detach "/Volumes/$VOLUME" -quiet
      echo ""
      echo "  Installed to /Applications/$APP_NAME.app"
    else
      echo "  DMG downloaded to /tmp/$FILENAME"
      echo "  Open it manually and drag to Applications."
    fi
    ;;
esac

echo ""
echo "  Done. Enjoy SwitchCraft!"
echo ""
