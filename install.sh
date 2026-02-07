#!/bin/bash
set -e

# Cartridge Controller CLI Installer
# Usage: curl -fsSL https://raw.githubusercontent.com/cartridge-gg/controller/main/controller-cli/install.sh | bash

REPO="broody/controller-cli"
BINARY_NAME="controller-cli"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# Detect platform
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux*)
        PLATFORM="unknown-linux-gnu"
        ;;
    Darwin*)
        PLATFORM="apple-darwin"
        ;;
    *)
        echo "Unsupported operating system: $OS"
        exit 1
        ;;
esac

case "$ARCH" in
    x86_64)
        ARCH="x86_64"
        ;;
    arm64|aarch64)
        ARCH="aarch64"
        ;;
    *)
        echo "Unsupported architecture: $ARCH"
        exit 1
        ;;
esac

TARGET="${ARCH}-${PLATFORM}"
echo "Detected platform: $TARGET"

# Get latest release
echo "Fetching latest release..."
LATEST_RELEASE=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$LATEST_RELEASE" ]; then
    echo "Failed to fetch latest release"
    exit 1
fi

echo "Latest release: $LATEST_RELEASE"

# Download URL
DOWNLOAD_URL="https://github.com/$REPO/releases/download/$LATEST_RELEASE/${BINARY_NAME}-${TARGET}.tar.gz"

echo "Downloading from: $DOWNLOAD_URL"

# Create temp directory
TMP_DIR=$(mktemp -d)
trap "rm -rf $TMP_DIR" EXIT

# Download and extract
cd "$TMP_DIR"
curl -fsSL "$DOWNLOAD_URL" | tar xz

# Create install directory if it doesn't exist
mkdir -p "$INSTALL_DIR"

# Install binary
mv "$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME"
chmod +x "$INSTALL_DIR/$BINARY_NAME"

echo ""
echo "✅ $BINARY_NAME installed successfully to $INSTALL_DIR"
echo ""

# Check if install dir is in PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo "⚠️  Add $INSTALL_DIR to your PATH:"
    echo ""
    echo "    export PATH=\"\$PATH:$INSTALL_DIR\""
    echo ""
    echo "Add this line to your ~/.bashrc, ~/.zshrc, or ~/.profile"
    echo ""
fi

# Verify installation
if command -v "$BINARY_NAME" &> /dev/null; then
    echo "Run '$BINARY_NAME --help' to get started"
else
    echo "Run '$INSTALL_DIR/$BINARY_NAME --help' to get started"
fi
