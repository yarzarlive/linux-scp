# LinuxSCP

> A modern, WinSCP-inspired dual-pane file transfer client for the Linux desktop.

Built with **Tauri 2**, **Rust**, **Tokio**, **React 18**, **TypeScript**, and **Tailwind CSS**.

---

## 1. Supported Distributions & Requirements

### Tested & Supported Linux Environments
- **Zorin OS 18.1 (Noble)** (Explicitly built and tested)
- **Ubuntu 24.04 LTS (Noble Numbat)** / **Ubuntu 22.04 LTS (Jammy Jellyfish)**
- **Debian 12 (Bookworm)**
- **Linux Mint 21 / 22**, **Pop!_OS 22.04 / 24.04**
- Modern glibc x86_64 Linux distributions via portable **AppImage**

### System Runtime Dependencies
- `libwebkit2gtk-4.1-0` (or `libwebkit2gtk-4.0-37`)
- `libgtk-3-0`
- `libsecret-1-0` (Linux Secret Service / D-Bus Keyring)
- `openssh-client` (for external terminal sessions)

---

## 2. Installation Options

### Option A: Debian / Ubuntu Package (`.deb`)
Download or build `LinuxSCP_0.1.0_amd64.deb`:

```bash
# Install with apt (automatically resolves dependencies)
sudo apt install ./src-tauri/target/release/bundle/deb/LinuxSCP_0.1.0_amd64.deb

# Launch LinuxSCP
linuxscp
```

### Option B: Portable AppImage (No Installation Required)
Download or build `LinuxSCP_0.1.0_amd64.AppImage`:

```bash
chmod +x ./src-tauri/target/release/bundle/appimage/LinuxSCP_0.1.0_amd64.AppImage
./src-tauri/target/release/bundle/appimage/LinuxSCP_0.1.0_amd64.AppImage
```

### Option C: Clean Rootless User Installation (`~/.local`)
Install directly without requiring `sudo` privileges:

```bash
./scripts/install.sh --user
```
This installs the binary to `~/.local/bin/linuxscp`, registers the desktop launcher in `~/.local/share/applications/`, and updates the icon cache.

### Option D: System-Wide Script Installation (`/usr/local`)
```bash
sudo ./scripts/install.sh
```

---

## 3. Uninstallation

### Debian / Ubuntu (`.deb`)
```bash
# Remove application
sudo apt remove linux-scp

# Remove application and purge all persistent databases and state
sudo apt purge linux-scp
```

### Script Uninstallation
```bash
# If installed with --user:
./scripts/uninstall.sh --user

# If installed with --user and wishing to wipe all user databases/keyrings:
./scripts/uninstall.sh --user --purge

# If installed system-wide:
sudo ./scripts/uninstall.sh
```

---

## 4. Building from Source

### Build Prerequisites
```bash
sudo apt update && sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libdbus-1-dev \
  build-essential \
  curl \
  wget \
  file \
  pkg-config
```

- **Rust:** `1.80+` (via `rustup`)
- **Node.js:** `v20+` or `v22+` (with `npm`)

### Build Commands
```bash
# 1. Install frontend dependencies
npm ci

# 2. Compile frontend and build release packages (.deb and .AppImage)
npm run build
npx tauri build -b deb,appimage
```

Packaged release artifacts are located in:
- `src-tauri/target/release/bundle/deb/LinuxSCP_0.1.0_amd64.deb`
- `src-tauri/target/release/bundle/appimage/LinuxSCP_0.1.0_amd64.AppImage`

For the step-by-step release audit checklist, see [RELEASE_CHECKLIST.md](docs/RELEASE_CHECKLIST.md).

---

## 5. Development & Testing Commands

### Run Desktop App in Development Mode
```bash
npm run tauri dev
```

### Quality & Safety Verification Gates
```bash
# Frontend type check, linting, code formatting, and Vitest unit tests
npm run typecheck
npm run lint
npm run format:check
npm test

# Rust formatting, Clippy (zero warnings), and unit/integration tests
cd src-tauri
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cd ..
```

---

## 6. Security & Architectural Principles

1. **Zero Plaintext Secrets:** Passwords, private-key passphrases, and tokens are stored exclusively in the Linux Secret Service (via D-Bus) and never written to SQLite, plaintext files, or application logs.
2. **Log Redaction:** Structured logging actively filters and redacts credentials, private keys, and PEM blocks (`[REDACTED_SENSITIVE]`).
3. **Strict Host-Key Verification:** Host keys are verified explicitly; changed host keys trigger an immediate block with fingerprint comparison and cannot be bypassed in-band.
4. **Internal Drag-and-Drop:** Drag-and-drop is restricted strictly between LinuxSCP's local and remote panes; desktop file dropping is unsupported.
5. **No Shell Interpolation:** External commands and desktop entries are launched with explicit argument vectors using `std::process::Command`, preventing command injection vulnerabilities.
6. **Atomic File Staging:** Downloads and uploads stream into `.linuxscp-tmp-<uuid>` files and are atomically promoted upon verification; aborted transfers are immediately cleaned up.

For the full security audit and vulnerability assessment, see [SECURITY_REVIEW.md](docs/SECURITY_REVIEW.md).

---

## 7. Troubleshooting & FAQ

### Secret Service / Keyring Unavailable
- **Symptom:** Error saving credentials or `Linux Secret Service failure during secret storage`.
- **Solution:** Ensure a Secret Service provider daemon is running on your desktop session bus (e.g. `gnome-keyring-daemon`, `keepassxc`, or `ksecretservice`). In headless/container environments, ensure `dbus-run-session` is initialized.

### Sudo-SFTP Handshake Timeout
- **Symptom:** `Timed out waiting for elevated SFTP protocol handshake`.
- **Solution:** Sudo elevation requires non-interactive execution. Ensure your server-side `/etc/sudoers` configuration allows passwordless execution of the SFTP server binary:
  ```text
  username ALL=(root) NOPASSWD: /usr/lib/openssh/sftp-server
  ```

### Wayland Scaling / Rendering
- If encountering WebKit rendering issues under certain Wayland compositors, run with:
  ```bash
  GDK_BACKEND=x11 linuxscp
  ```
