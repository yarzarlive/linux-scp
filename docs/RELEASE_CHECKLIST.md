# LinuxSCP Release Checklist & Packaging Guide

**Version:** 0.1.0  
**Target Architecture:** Linux x86_64 (`x86_64-unknown-linux-gnu`)  
**Tested Distributions:**
- **Zorin OS 18.1 / Ubuntu 24.04 LTS (Noble Numbat)** (Explicitly built and tested)
- **Ubuntu 22.04 LTS / Debian 12 (Bookworm)** (Compatible ABI / glibc target)

---

## 1. Supported Packaging Formats

LinuxSCP supports two primary packaging formats for Linux desktop systems:

| Package Format | Target Distributions | Artifact Location | Notes |
| :--- | :--- | :--- | :--- |
| **Debian Package (`.deb`)** | Ubuntu, Debian, Linux Mint, Zorin OS, Pop!_OS | `src-tauri/target/release/bundle/deb/LinuxSCP_0.1.0_amd64.deb` | Native system packaging with dependency resolution and XDG desktop/icon integration. |
| **AppImage (`.AppImage`)** | Any modern glibc Linux distribution (Fedora, Arch, openSUSE, Debian/Ubuntu) | `src-tauri/target/release/bundle/appimage/LinuxSCP_0.1.0_amd64.AppImage` | Portable single-binary bundle with bundled desktop runtime and libraries. |
| **Direct Script (`install.sh`)** | Any Linux distribution | `scripts/install.sh` | Supports both system-wide (`/usr/local`) and rootless per-user (`~/.local`) installation. |

---

## 2. Runtime Dependencies & Desktop Integration

### Native Runtime Dependencies (`.deb` / Script Install)
The native binary relies on standard desktop Linux libraries provided by modern distributions:
- `libwebkit2gtk-4.1-0` (or `libwebkit2gtk-4.0-37`) — Desktop WebView rendering
- `libgtk-3-0` — GTK+ 3 desktop windowing and file chooser dialogs
- `libsecret-1-0` — Secret Service credential manager access
- `openssh-client` — OpenSSH binary for launching external SSH terminals
- `xdg-utils` — XDG application launching and desktop integration

### Desktop Standards Adherence
- **Desktop Entry:** `assets/linuxscp.desktop` adheres to XDG Desktop Entry Specification v1.5 with `Categories=Network;FileTransfer;Utility;` and `MimeType=x-scheme-handler/sftp;x-scheme-handler/scp;`.
- **AppStream Metadata:** `assets/com.linuxscp.client.metainfo.xml` conforms to AppStream specification for software centers (GNOME Software, Discover).
- **Hicolor Icons:** 32x32, 128x128, and 256x256 PNG icons placed in standard `$PREFIX/share/icons/hicolor/<size>/apps/`.

---

## 3. Desktop Subsystem Verification

1. **Linux Secret Service:**
   - LinuxSCP communicates over the D-Bus session bus with the desktop Secret Service daemon (`gnome-keyring-daemon`, `keepassxc`, or `ksecretservice`).
   - Verified that credentials are stored securely under collection `org.linuxscp.client`.
   - In minimal/headless containers without a running D-Bus session bus, attempts to persist credentials return a descriptive `AppError::Security` error.
2. **Open With (XDG Application Discovery):**
   - Discovers applications from `/usr/share/applications`, `/usr/local/share/applications`, and `~/.local/share/applications`.
   - Executes target applications using direct `argv` arrays via `std::process::Command` without shell expansion.
3. **Terminal Discovery and Launching:**
   - Discovers installed terminals in prioritized order (`x-terminal-emulator`, `gnome-terminal`, `ptyxis`, `konsole`, `xfce4-terminal`, etc.).
   - Launches independent SSH sessions using native OpenSSH arguments, never disabling host-key checking and never injecting passwords into arguments.
4. **Temporary Files & Workspace Hygiene:**
   - Working copies are stored in `$XDG_STATE_HOME/linuxscp/remote-edits/<uuid>` with restrictive `0700` permissions.
   - Clean edits are unlinked on application exit.
   - Partial downloads and uploads are unlinked immediately on cancellation or connection drops.

---

## 4. Reproducible Release Build Procedure

### Build Environment Preparation
```bash
# 1. Ensure Node.js and Rust are available
node -v   # v20+ or v22+
cargo -V  # 1.80+

# 2. Install frontend dependencies
npm ci
```

### Verification Pipeline
Before creating a release bundle, run all automated verification checks:
```bash
# 1. TypeScript and frontend checks
npm run typecheck
npm run lint
npm run format:check
npm test

# 2. Rust checks and test suite
cd src-tauri
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cd ..
```

### Release Packaging
Generate release artifacts (`.deb` and `.AppImage`):
```bash
# Build production bundle
npm run build
npx tauri build -b deb,appimage
```

Artifacts are produced in:
- Debian Package: `src-tauri/target/release/bundle/deb/LinuxSCP_0.1.0_amd64.deb`
- AppImage: `src-tauri/target/release/bundle/appimage/LinuxSCP_0.1.0_amd64.AppImage`
- Binary: `src-tauri/target/release/linuxscp`

---

## 5. Clean Install & Uninstall Verification

### Option A: Debian / Ubuntu (`.deb`)
```bash
# Install
sudo apt install ./src-tauri/target/release/bundle/deb/LinuxSCP_0.1.0_amd64.deb

# Verify execution
linuxscp --help

# Uninstall
sudo apt remove linux-scp
# Or to purge configuration
sudo apt purge linux-scp
```

### Option B: User-Level Script Install (No Root Required)
```bash
# Install to ~/.local
./scripts/install.sh --user

# Verify execution
~/.local/bin/linuxscp

# Uninstall
./scripts/uninstall.sh --user
```

### Option C: System-Wide Script Install
```bash
# Install to /usr/local
sudo ./scripts/install.sh

# Uninstall
sudo ./scripts/uninstall.sh
```

### Option D: Portable AppImage
```bash
chmod +x src-tauri/target/release/bundle/appimage/LinuxSCP_0.1.0_amd64.AppImage
./src-tauri/target/release/bundle/appimage/LinuxSCP_0.1.0_amd64.AppImage
```

---

## 6. Pre-Release Verification Checklist

- [x] Application identifier is normalized (`com.linuxscp.client`).
- [x] Application categories configured (`Network;FileTransfer;Utility;`).
- [x] Standard icons installed in 32x32, 128x128, and 256x256 resolutions.
- [x] AppStream metadata XML validated.
- [x] Tested native execution of release binary and AppImage on Linux (Zorin OS 18.1 / Ubuntu 24.04).
- [x] Tested clean install and uninstall procedures (`install.sh --user` and `uninstall.sh --user`).
- [x] Verified zero development credentials, plaintext secrets, or test keys bundled into release artifacts.
- [x] Strict host-key verification and secret storage verified in packaged build.
- [x] All 120 Rust tests and 114 frontend tests pass without failure.

