#!/bin/sh
# Granary installation script for Unix-like systems (macOS, Linux)
# Usage: curl -sSfL https://raw.githubusercontent.com/speakeasy-api/granary/main/scripts/install.sh | sh

set -e

REPO="speakeasy-api/granary"
BINARY_NAME="granary"
INSTALL_DIR="${GRANARY_INSTALL_DIR:-$HOME/.granary/bin}"

# Colors for output (if terminal supports it)
if [ -t 1 ]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    BLUE='\033[0;34m'
    NC='\033[0m' # No Color
else
    RED=''
    GREEN=''
    YELLOW=''
    BLUE=''
    NC=''
fi

info() {
    printf "${BLUE}info${NC}: %s\n" "$1"
}

success() {
    printf "${GREEN}success${NC}: %s\n" "$1"
}

warn() {
    printf "${YELLOW}warning${NC}: %s\n" "$1"
}

error() {
    printf "${RED}error${NC}: %s\n" "$1" >&2
    exit 1
}

# Detect OS
detect_os() {
    case "$(uname -s)" in
        Linux*)  echo "unknown-linux-gnu" ;;
        Darwin*) echo "apple-darwin" ;;
        *)       error "Unsupported operating system: $(uname -s)" ;;
    esac
}

# Detect architecture
detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64)   echo "x86_64" ;;
        aarch64|arm64)  echo "aarch64" ;;
        *)              error "Unsupported architecture: $(uname -m)" ;;
    esac
}

# Check for required commands
check_dependencies() {
    if ! command -v curl >/dev/null 2>&1 && ! command -v wget >/dev/null 2>&1; then
        error "curl or wget is required but not installed"
    fi
    if ! command -v tar >/dev/null 2>&1; then
        error "tar is required but not installed"
    fi
}

# Download file using curl or wget
download() {
    url="$1"
    output="$2"

    if command -v curl >/dev/null 2>&1; then
        curl -sSfL "$url" -o "$output"
    elif command -v wget >/dev/null 2>&1; then
        wget -q "$url" -O "$output"
    fi
}

# Check if a version string is a pre-release (contains '-')
is_prerelease() {
    case "$1" in
        *-*) return 0 ;;  # Contains '-', is pre-release
        *)   return 1 ;;  # No '-', is stable
    esac
}

# Extract tag names from GitHub releases JSON
# Uses jq if available, falls back to sed
extract_tag_names() {
    if command -v jq >/dev/null 2>&1; then
        jq -r '.[].tag_name'
    else
        # Fallback: use sed to extract tag_name values
        # Works with both pretty-printed and compact JSON
        sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p'
    fi
}

# Get latest stable release version from GitHub (excludes pre-releases)
get_latest_version() {
    url="https://api.github.com/repos/${REPO}/releases"
    if command -v curl >/dev/null 2>&1; then
        releases=$(curl -sSf "$url")
    else
        releases=$(wget -qO- "$url")
    fi

    if [ -z "$releases" ]; then
        error "Could not fetch releases. Check your internet connection or visit https://github.com/${REPO}/releases"
    fi

    # Extract all tag names and find first non-prerelease
    # Use printf instead of echo to avoid escape sequence interpretation
    version=""
    for tag in $(printf '%s' "$releases" | extract_tag_names); do
        if ! is_prerelease "$tag"; then
            version="$tag"
            break
        fi
    done

    if [ -z "$version" ]; then
        error "Could not determine latest stable version. Check https://github.com/${REPO}/releases"
    fi

    echo "$version"
}

main() {
    info "Installing granary..."

    check_dependencies

    OS=$(detect_os)
    ARCH=$(detect_arch)
    TARGET="${ARCH}-${OS}"

    info "Detected platform: ${TARGET}"

    # Use GRANARY_VERSION env var if set, otherwise fetch latest stable
    if [ -n "${GRANARY_VERSION:-}" ]; then
        VERSION="${GRANARY_VERSION}"
        # Add 'v' prefix if not present (GitHub tags use 'v' prefix)
        case "$VERSION" in
            v*) ;;
            *)  VERSION="v${VERSION}" ;;
        esac
        info "Installing requested version: ${VERSION}"
    else
        VERSION=$(get_latest_version)
        info "Latest version: ${VERSION}"
    fi

    # Construct download URL
    ARCHIVE_NAME="${BINARY_NAME}-${TARGET}.tar.gz"
    DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE_NAME}"

    # Create temp directory
    TMP_DIR=$(mktemp -d)
    trap 'rm -rf "$TMP_DIR"' EXIT

    info "Downloading ${ARCHIVE_NAME}..."
    download "$DOWNLOAD_URL" "$TMP_DIR/$ARCHIVE_NAME" || error "Failed to download from ${DOWNLOAD_URL}"

    info "Extracting..."
    tar -xzf "$TMP_DIR/$ARCHIVE_NAME" -C "$TMP_DIR" || error "Failed to extract archive"

    # Create install directory if it doesn't exist
    mkdir -p "$INSTALL_DIR"

    # Install binary
    info "Installing to ${INSTALL_DIR}..."
    mv "$TMP_DIR/$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME"
    chmod +x "$INSTALL_DIR/$BINARY_NAME"

    success "Granary ${VERSION} installed successfully!"

    # Check if install directory is in PATH
    case ":$PATH:" in
        *":$INSTALL_DIR:"*)
            info "Installation directory is already in your PATH"
            ;;
        *)
            warn "Add the following to your shell configuration file (.bashrc, .zshrc, etc.):"
            echo ""
            echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
            echo ""
            warn "Then restart your shell or run: source ~/.bashrc (or ~/.zshrc)"
            ;;
    esac

    echo ""
    info "Get started with: granary --help"
}

main
