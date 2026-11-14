#!/usr/bin/env bash
set -euo pipefail

# LinuxSCP Uninstallation Script
# Supports:
#   System-wide uninstallation (root / sudo): removes from /usr/local
#   User-level uninstallation (--user): removes from ~/.local

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

PREFIX="/usr/local"
INSTALL_USER=false
PURGE_USER_DATA=false

print_usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Options:
  --user            Uninstall from user directory (~/.local)
  --prefix <PATH>   Custom installation prefix (default: /usr/local)
  --purge           Also remove user state, database, and cache (~/.local/share/linuxscp)
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
        --purge)
            PURGE_USER_DATA=true
            shift
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
    echo "Warning: Uninstalling from ${PREFIX} typically requires root permissions."
    echo "If this fails, rerun with sudo or use '--user' for a local user uninstall."
fi

BIN_DIR="${PREFIX}/bin"
APP_DIR="${PREFIX}/share/applications"
ICON_BASE="${PREFIX}/share/icons/hicolor"
METAINFO_DIR="${PREFIX}/share/metainfo"

echo "=== Uninstalling LinuxSCP ==="
echo "Target prefix: ${PREFIX}"

# 1. Remove binary
if [[ -f "${BIN_DIR}/linuxscp" ]]; then
    echo "Removing binary: ${BIN_DIR}/linuxscp..."
    rm -f "${BIN_DIR}/linuxscp"
fi

# 2. Remove desktop file
if [[ -f "${APP_DIR}/linuxscp.desktop" ]]; then
    echo "Removing desktop entry: ${APP_DIR}/linuxscp.desktop..."
    rm -f "${APP_DIR}/linuxscp.desktop"
fi

# 3. Remove icons
for size in 32x32 128x128 256x256; do
    icon_file="${ICON_BASE}/${size}/apps/linuxscp.png"
    if [[ -f "${icon_file}" ]]; then
        echo "Removing icon: ${icon_file}..."
        rm -f "${icon_file}"
    fi
done

# 4. Remove AppStream metadata
if [[ -f "${METAINFO_DIR}/com.linuxscp.client.metainfo.xml" ]]; then
    echo "Removing AppStream metadata: ${METAINFO_DIR}/com.linuxscp.client.metainfo.xml..."
    rm -f "${METAINFO_DIR}/com.linuxscp.client.metainfo.xml"
fi

# 5. Update desktop database and icon cache
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q "${APP_DIR}" || true
fi

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -t "${ICON_BASE}" || true
fi

# 6. Optional purge of persistent user data
if [[ "${PURGE_USER_DATA}" == true ]]; then
    DATA_DIR="${XDG_DATA_HOME:-${HOME}/.local/share}/linuxscp"
    STATE_DIR="${XDG_STATE_HOME:-${HOME}/.local/state}/linuxscp"
    CACHE_DIR="${XDG_CACHE_HOME:-${HOME}/.cache}/linuxscp"

    echo "Purging persistent user data directories..."
    rm -rf "${DATA_DIR}" "${STATE_DIR}" "${CACHE_DIR}"
    echo "User configuration, SQLite database, and temporary workspaces purged."
fi

echo "=== LinuxSCP uninstalled cleanly! ==="

