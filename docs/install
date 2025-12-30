#!/bin/sh
# Install script for remagnify
# Usage: curl -fsSL https://raw.githubusercontent.com/alfredodeza/remagnify/main/install.sh | sh

set -e

# Configuration
REPO="alfredodeza/remagnify"
BINARY_NAME="remagnify"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Helper functions
info() {
    printf "${GREEN}==>${NC} %s\n" "$1"
}

warn() {
    printf "${YELLOW}Warning:${NC} %s\n" "$1"
}

error() {
    printf "${RED}Error:${NC} %s\n" "$1" >&2
    exit 1
}

# Detect OS and architecture
detect_platform() {
    local os=$(uname -s | tr '[:upper:]' '[:lower:]')
    local arch=$(uname -m)

    case "$os" in
        linux*)
            OS="linux"
            ;;
        *)
            error "Unsupported operating system: $os. remagnify only supports Linux with Wayland."
            ;;
    esac

    case "$arch" in
        x86_64|amd64)
            ARCH="x86_64"
            ;;
        aarch64|arm64)
            ARCH="aarch64"
            ;;
        *)
            error "Unsupported architecture: $arch. Supported: x86_64, aarch64"
            ;;
    esac

    TARGET="${ARCH}-unknown-${OS}-gnu"
}

# Check if running on Wayland
check_wayland() {
    if [ -z "$WAYLAND_DISPLAY" ] && [ -z "$XDG_SESSION_TYPE" ]; then
        warn "Wayland environment not detected. remagnify requires a wlroots-based compositor."
    fi
}

# Get the latest release version from GitHub
get_latest_version() {
    info "Fetching latest release version..."

    if command -v curl > /dev/null 2>&1; then
        VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
    elif command -v wget > /dev/null 2>&1; then
        VERSION=$(wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
    else
        error "Neither curl nor wget found. Please install one of them."
    fi

    if [ -z "$VERSION" ]; then
        error "Failed to fetch latest version. Please check your internet connection."
    fi

    info "Latest version: $VERSION"
}

# Download and verify the binary
download_binary() {
    local archive_name="${BINARY_NAME}-${VERSION}-${TARGET}.tar.gz"
    local download_url="https://github.com/${REPO}/releases/download/${VERSION}/${archive_name}"
    local checksum_url="${download_url}.sha256"

    info "Downloading $archive_name..."

    local tmpdir=$(mktemp -d)
    cd "$tmpdir"

    if command -v curl > /dev/null 2>&1; then
        curl -fsSL -o "$archive_name" "$download_url" || error "Failed to download binary"
        curl -fsSL -o "${archive_name}.sha256" "$checksum_url" || error "Failed to download checksum"
    elif command -v wget > /dev/null 2>&1; then
        wget -q -O "$archive_name" "$download_url" || error "Failed to download binary"
        wget -q -O "${archive_name}.sha256" "$checksum_url" || error "Failed to download checksum"
    fi

    info "Verifying checksum..."
    if command -v sha256sum > /dev/null 2>&1; then
        sha256sum -c "${archive_name}.sha256" || error "Checksum verification failed"
    else
        warn "sha256sum not found, skipping checksum verification"
    fi

    info "Extracting binary..."
    tar xzf "$archive_name" || error "Failed to extract archive"

    BINARY_PATH="$tmpdir/$BINARY_NAME"
}

# Install the binary
install_binary() {
    local install_dir

    # Determine installation directory
    if [ -w "/usr/local/bin" ]; then
        install_dir="/usr/local/bin"
    elif [ -d "$HOME/.local/bin" ]; then
        install_dir="$HOME/.local/bin"
        # Ensure it's in PATH
        case ":$PATH:" in
            *":$HOME/.local/bin:"*) ;;
            *) warn "$HOME/.local/bin is not in your PATH. Add it to use remagnify." ;;
        esac
    else
        mkdir -p "$HOME/.local/bin"
        install_dir="$HOME/.local/bin"
        warn "Created $HOME/.local/bin. Add it to your PATH: export PATH=\"\$HOME/.local/bin:\$PATH\""
    fi

    info "Installing to $install_dir..."

    # Move binary
    if [ -w "$install_dir" ]; then
        mv "$BINARY_PATH" "$install_dir/$BINARY_NAME"
    else
        # Need sudo
        info "Requesting sudo permission to install to $install_dir..."
        sudo mv "$BINARY_PATH" "$install_dir/$BINARY_NAME"
    fi

    chmod +x "$install_dir/$BINARY_NAME"

    info "${GREEN}✓${NC} Installation complete!"
    info "Run '${BINARY_NAME} --version' to verify installation"
}

# Check system dependencies
check_dependencies() {
    info "Checking system dependencies..."

    local missing_deps=""

    # Check for required libraries (this is basic, actual check would need pkg-config)
    if ! ldconfig -p | grep -q libwayland-client; then
        missing_deps="${missing_deps}\n  - wayland (libwayland-client)"
    fi
    if ! ldconfig -p | grep -q libcairo; then
        missing_deps="${missing_deps}\n  - cairo (libcairo2)"
    fi
    if ! ldconfig -p | grep -q libpango; then
        missing_deps="${missing_deps}\n  - pango (libpango-1.0)"
    fi
    if ! ldconfig -p | grep -q libxkbcommon; then
        missing_deps="${missing_deps}\n  - xkbcommon (libxkbcommon)"
    fi

    if [ -n "$missing_deps" ]; then
        warn "Some system dependencies may be missing:${missing_deps}"
        printf "\nInstall them with:\n"
        printf "  Arch Linux:   sudo pacman -S wayland cairo pango libxkbcommon\n"
        printf "  Ubuntu/Debian: sudo apt install libwayland-client0 libcairo2 libpango-1.0-0 libxkbcommon0\n"
        printf "  Fedora:       sudo dnf install wayland cairo pango libxkbcommon\n\n"
    fi
}

# Cleanup
cleanup() {
    if [ -n "$tmpdir" ] && [ -d "$tmpdir" ]; then
        rm -rf "$tmpdir"
    fi
}

# Main installation flow
main() {
    info "Installing remagnify..."

    detect_platform
    check_wayland
    get_latest_version
    download_binary
    install_binary
    check_dependencies
    cleanup

    printf "\n${GREEN}✓ remagnify installed successfully!${NC}\n\n"
    printf "Quick start:\n"
    printf "  remagnify --help\n"
    printf "  remagnify -z 0.2 --exit-delay 500 --size 1200x600\n\n"
    printf "Add to Hyprland config:\n"
    printf "  bind = SUPER, M, exec, pkill remagnify || remagnify -z 0.2 --exit-delay 500 --size 1200x600\n\n"
}

# Set trap for cleanup
trap cleanup EXIT

# Run main function
main
