# LinuxSCP — Technical Feasibility Spike (Milestone 0)

**Date:** 2026-11-11  
**Status:** Completed & Validated  
**Target Environment:** Zorin OS 18.1 (Ubuntu 24.04 LTS noble), Linux 7.0.0-42-generic x86_64  

---

## 1. Executive Summary

All critical technical risks identified in the LinuxSCP Product Specification have been prototyped and proven viable in the real deployment environment:
1. **Tauri 2 + GTK3 + WebKit2GTK-4.1:** Compiled and linked into a functional desktop application binary (`spikestauri-spike`).
2. **React 18 + TypeScript + Vite:** Verified with typed client compilation in 124ms.
3. **SSH & SFTP Engine (`russh` + `russh-sftp`):** Verified against live test server `ubuntu@192.168.122.10` with strict host-key verification, SHA256 fingerprint extraction, and RSA-SHA2-512 authentication.
4. **Elevated & External Exec SFTP Architecture:** Proven that an external SFTP server binary (`/usr/lib/openssh/sftp-server`) can be executed over a dedicated SSH session channel, and `russh-sftp` attaches directly to the channel stream for full SFTP file operations without corruption.
5. **Linux Secret Service:** Keyring credential storage and retrieval proven over DBus using `keyring` (backed by `zbus` and GNOME Keyring).
6. **XDG Desktop Application Discovery:** Successfully scanned and parsed 94 user-facing desktop applications across standard system and user XDG directories.

---

## 2. Detailed Spike Results

### 2.1 SSH Host-Key Verification & Authentication
- **Test Server:** `ubuntu@192.168.122.10:22`
- **Key Verification:** Received host public key algorithm `ssh-ed25519` and computed SHA256 fingerprint: `SHA256:Vv0VgQVgEC7Xn8zymXiYc6H72l+DCAEiaf5PURe/dMg`.
- **Authentication:** Loaded `~/.ssh/id_rsa`. Modern OpenSSH servers (OpenSSH 8.8+) reject obsolete SHA-1 RSA signatures (`ssh-rsa`). By configuring `PrivateKeyWithHashAlg::new(key, Some(HashAlg::Sha512))`, authentication succeeded immediately.

### 2.2 SFTP Over Subsystem vs Executed Process Channel
- **Standard SFTP Subsystem:** Requesting subsystem `sftp` initialized cleanly. Directory listing on `/home/ubuntu` returned accurate file sizes and Unix permissions (octal 700, 644, 600).
- **Executed Process SFTP Channel:** 
  - Opened dedicated SSH session channel (`channel_open_session`).
  - Executed remote command `/usr/lib/openssh/sftp-server`.
  - Converted the channel to an async stream (`channel.into_stream()`) and passed it directly to `SftpSession::new()`.
  - Initialized SFTP framing without error, traversed `/home/ubuntu`, and retrieved directory metadata.
- **Sudo SFTP (`sudo -n /usr/lib/openssh/sftp-server`):**
  - When non-interactive sudo is denied (interactive password required), the remote sudo process terminates immediately with status 1.
  - The channel stream cleanly reports EOF (`sender dropped`) to the SFTP client.
  - This guarantees that an unprivileged or improperly configured sudo account will never hang or produce false positives, enabling crisp diagnostic error messages.

### 2.3 Linux Secret Service & Keyring
- Keyring operations (`set_password`, `get_password`, `delete_credential`) were tested using `keyring v4.2` with DBus `zbus` integration.
- The active desktop keyring daemon responded promptly and stored credentials safely without plaintext disk persistence.

### 2.4 Desktop Application Discovery (XDG)
- Desktop entry discovery scanned `~/.local/share/applications`, `/usr/share/applications`, and Flatpak export directories.
- Successfully parsed 94 installed applications, filtering out `NoDisplay=true` utility entries and correctly extracting application names, binary paths, and arguments (`%U`, `%F`).

### 2.5 Tauri 2 Desktop Shell Build
- Scaffolded minimal Tauri 2 React + TypeScript project in `spikes/tauri-spike`.
- System C dependencies verified: `webkit2gtk-4.1` (2.52.6), `gtk+-3.0` (3.24.41), `openssl` (3.0.13), `dbus-1` (1.14.10).
- `cargo build` generated the native binary (`spikes/tauri-spike/src-tauri/target/debug/spikestauri-spike`) without any compilation or linker warnings.

---

## 3. Architecture Decisions Confirmed

| Decision | Selected Choice | Rationale |
|---|---|---|
| **Desktop Shell** | Tauri 2 | Lightweight footprint, native GTK/WebKitGTK webview, first-class Rust concurrency. |
| **SSH/SFTP Crate** | `russh` + `russh-sftp` | Pure Rust async implementation on Tokio. Seamlessly supports piping SFTP framing over custom executed channels (`sudo -n <sftp-server>`). |
| **RSA Signature Policy** | `rsa-sha2-256` / `rsa-sha2-512` | Mandatory for compatibility with Ubuntu 22.04+ / Debian 12+ OpenSSH defaults. |
| **Credential Storage** | `keyring` (Secret Service over DBus) | Strictly adheres to zero-plaintext secret policy. |
| **Application Discovery** | Standard XDG `.desktop` parser | Safe argument expansion (`%f`/`%u`) passed directly via `std::process::Command` without shell string interpolation. |

---

## 4. Next Milestone (M1: Application Scaffolding & Local Pane)

With all technical feasibility spikes validated, the project is ready to begin **Milestone 1**:
- Initialize the root application structure.
- Build the Commander-style dual-pane UI layout.
- Implement local filesystem browsing, navigation toolbar, path bar, and local CRUD operations.
