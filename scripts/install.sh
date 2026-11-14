#!/usr/bin/env bash
set -euo pipefail

# LinuxSCP Installation Script
# Supports:
#   System-wide installation (root / sudo): installs to /usr/local
#   User-level installation (--user): installs to ~/.local

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

PREFIX="/usr/local"
INSTALL_USER=false

print_usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Options:
  --user            Install for the current user only (~/.local)
  --prefix <PATH>   Custom installation prefix (default: /usr/local)
  -h, --help        Show this help message
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --user)
            INSTALL_USER=true
            PREFIX="${HOME}/.local"
            shift
            ;;
        --prefix)
            PREFIX="$2"
            shift 2
            ;;
        -h|--help)
            print_usage
            exit 0
            ;;
        *)
            echo "Error: Unknown option: $1" >&2
            print_usage
            exit 1
            ;;
    esac
done

if [[ "${INSTALL_USER}" == false && "$(id -u)" -ne 0 ]]; then
    echo "Warning: Installing system-wide to ${PREFIX} typically requires root permissions."
    echo "If this fails, rerun with sudo or use '--user' for a local user install."
fi

# Locate release binary
BINARY_SOURCE="${PROJECT_ROOT}/src-tauri/target/release/linuxscp"
if [[ ! -f "${BINARY_SOURCE}" ]]; then
    echo "Error: Release binary not found at '${BINARY_SOURCE}'."
    echo "Please build LinuxSCP first: cd ${PROJECT_ROOT} && npm run tauri build"
    exit 1
fi

BIN_DIR="${PREFIX}/bin"
APP_DIR="${PREFIX}/share/applications"
ICON_BASE="${PREFIX}/share/icons/hicolor"
METAINFO_DIR="${PREFIX}/share/metainfo"

echo "=== Installing LinuxSCP ==="
echo "Target prefix: ${PREFIX}"

mkdir -p "${BIN_DIR}" "${APP_DIR}" "${METAINFO_DIR}"

# 1. Install binary
echo "Installing binary to ${BIN_DIR}/linuxscp..."
install -m 755 "${BINARY_SOURCE}" "${BIN_DIR}/linuxscp"

# 2. Install desktop entry
echo "Installing desktop entry to ${APP_DIR}/linuxscp.desktop..."
install -m 644 "${PROJECT_ROOT}/assets/linuxscp.desktop" "${APP_DIR}/linuxscp.desktop"

# 3. Install icons into hicolor icon theme hierarchy
ICONS_SRC="${PROJECT_ROOT}/src-tauri/icons"

install_icon() {
    local src_file="$1"
    local size="$2"
    local dest_dir="${ICON_BASE}/${size}/apps"
    if [[ -f "${src_file}" ]]; then
        mkdir -p "${dest_dir}"
        install -m 644 "${src_file}" "${dest_dir}/linuxscp.png"
    fi
}

echo "Installing application icons to ${ICON_BASE}..."
install_icon "${ICONS_SRC}/32x32.png" "32x32"
install_icon "${ICONS_SRC}/128x128.png" "128x128"
install_icon "${ICONS_SRC}/128x128@2x.png" "256x256"

# 4. Install AppStream metadata
if [[ -f "${PROJECT_ROOT}/assets/com.linuxscp.client.metainfo.xml" ]]; then
    echo "Installing AppStream metadata to ${METAINFO_DIR}/..."
    install -m 644 "${PROJECT_ROOT}/assets/com.linuxscp.client.metainfo.xml" "${METAINFO_DIR}/com.linuxscp.client.metainfo.xml"
fi

# 5. Update desktop database and icon cache if tools are present
if command -v update-desktop-database >/dev/null 2>&1; then
    echo "Updating desktop database..."
    update-desktop-database -q "${APP_DIR}" || true
fi

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    echo "Updating GTK icon cache..."
    gtk-update-icon-cache -q -t "${ICON_BASE}" || true
fi

echo "=== LinuxSCP installed successfully! ==="
echo "You can launch it from your desktop application launcher or run: ${BIN_DIR}/linuxscp"
if [[ "${INSTALL_USER}" == true ]]; then
    if [[ ":$PATH:" != *":${HOME}/.local/bin:"* ]]; then
        echo "Note: Make sure '${HOME}/.local/bin' is included in your PATH environment variable."
    fi
fi

