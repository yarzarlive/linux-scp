# LinuxSCP Security Review and Hardening Audit

**Date:** 2026-11-14  
**Application Version:** 0.1.0  
**Scope:** Architecture, Backend (Rust/Tauri), Frontend (React/TypeScript), IPC Boundaries, Cryptography, Desktop Integration, and Usability Controls.

---

## 1. Executive Summary

A comprehensive quality and security audit was performed across the LinuxSCP codebase. LinuxSCP is a dual-pane graphical file manager built on Tauri 2 and React, specifically designed for Linux desktop environments with strict adherence to security boundaries.

### Summary of Posture:
- **Critical / High Defects:** 0 unresolved.
- **Strict Host-Key Verification:** Verified and enforced; changed host keys are strictly blocked without auto-trust or in-band replacement bypass.
- **Credential Storage:** Secrets (passwords and passphrases) are stored exclusively in the system Linux Secret Service keyring (`org.linuxscp.client`); never in plaintext files or SQLite.
- **Process Launching:** Direct argv vectors via `std::process::Command`; zero invocation of shell interpreters (`sh -c`) and strict option-injection sanitization.
- **Filesystem Safety:** Atomic staging (`.linuxscp-tmp-<uuid>`), immediate cleanup of partial files on error or cancellation, strict symlink unlinking (never followed on delete/recurse), and protected root directory prevention.
- **Elevation Control:** Non-interactive sudo-SFTP is an explicit, opt-in capability with unambiguous visual indicators and confirmation dialogs.
- **FTPS Protocol:** Safely rejected at connection establishment pending future milestone implementation; security specifications for strict TLS certificate validation defined.

---

## 2. Security Review Matrix & Findings

### 2.1 SSH Host-Key Verification and Changed-Key Handling
- **Severity:** High (Standard Risk Area)
- **Status:** **PASS / Hardened**
- **Analysis:**
  - When connecting to any SSH server (SFTP or SCP), the server host key is evaluated before user credentials are sent over the wire (`SshClientHandler::check_server_key`).
  - Known hosts are stored in SQLite table `known_hosts` (host, port, algorithm, SHA-256 fingerprint, OpenSSH public key format).
  - If a key is unknown, the handshake is aborted with `ConnectOutcome::HostKeyUnknown` and presented to the user with the algorithm and SHA-256 fingerprint.
  - If a key has changed for a known host, `HostKeyDecision::Changed` is triggered. The handshake is aborted immediately.
  - In `HostKeyPromptModal.tsx`, a changed key dialog displays a prominent security warning with expected vs. presented fingerprints and only offers **"Abort & Close"**. LinuxSCP strictly prohibits automatic or in-band replacement of changed keys.
  - External SSH terminal commands do not pass `-o StrictHostKeyChecking=no` and preserve OpenSSH's native `~/.ssh/known_hosts`.

### 2.2 Secret Storage and Accidental Credential Logging
- **Severity:** High (Standard Risk Area)
- **Status:** **PASS / Hardened**
- **Analysis:**
  - `ConnectionProfile` data structures and the SQLite `connection_profiles` table contain only connection metadata. `AuthMethod::Password` contains no secret field; `AuthMethod::PublicKey` contains only file path and a boolean `requires_passphrase`.
  - Secret persistence is handled exclusively by `KeyringService` via the Linux Secret Service D-Bus API (`keyring` crate).
  - When profiles are deleted, `delete_secret` purges the corresponding keyring entry.
  - Logging is configured in `logging.rs` with `sanitize_log_message` redacting keywords (`password`, `passphrase`, `private_key`, `secret`, `token`, `credential`, and PEM headers).
  - Codebase audit confirmed that no log statement prints secrets or debug-prints profile structures with secret content. Frontend console logs do not log passwords.

### 2.3 Process Launching and Shell-Injection Risks
- **Severity:** High (Standard Risk Area)
- **Status:** **PASS / Hardened**
- **Analysis:**
  - Process spawning occurs in only two subsystems:
    1. `desktop_integration/mod.rs` (Open With external applications)
    2. `terminal/mod.rs` (External SSH terminal emulators)
  - Both subsystems use `std::process::Command::new(&exec).args(&args)` with discrete string vectors. Neither invokes `sh -c`, `bash -c`, or standard shell interpolation.
  - Terminal arguments are validated via `build_ssh_args()`: hostnames and usernames starting with `-` or containing whitespace or control characters are rejected to prevent option injection.
  - Child processes are reaped asynchronously via `std::thread::spawn(move || { let _ = child.wait(); })` to prevent zombie `<defunct>` processes.

### 2.4 Path Handling, Traversal, and Symlink Behavior
- **Severity:** Medium-High
- **Status:** **PASS / Hardened**
- **Analysis:**
  - Path inputs from UI and remote streams are validated against directory traversal dots (`.` and `..`), null bytes (`\0`), and path separators (`validate_entry_name` and `validate_remote_path`).
  - System roots and critical directories (`/`, `/bin`, `/boot`, `/dev`, `/etc`, `/lib`, `/proc`, `/sys`, `/usr`, `/root`, `/home`) are guarded against accidental deletion (`is_protected_system_path`).
  - For symlinks:
    - In local filesystem operations, `fs::symlink_metadata` is strictly used rather than `fs::metadata`.
    - Deletion of a symlink performs `fs::remove_file` (unlinking the symbolic link itself) and never deletes the symlink's target directory or file.
    - In remote SFTP recursive deletion, `if attrs.is_dir() && !attrs.is_symlink()` is enforced; symlinks to directories are unlinked as files and never recursed into.

### 2.5 Transfer Cancellation, Partial Files, and Atomic Staging
- **Severity:** Medium
- **Status:** **PASS / Hardened**
- **Analysis:**
  - Both SFTP and SCP transfers stage downloads and uploads through temporary unique files:
    - Download: `<dest_dir>/.linuxscp-tmp-<uuid>`
    - Upload: `<remote_dest>.linuxscp-tmp-<uuid>`
  - Streaming loops listen on `tokio::select!` channels for `watch::Receiver<bool>` cancellation signals.
  - If a transfer is cancelled or encounters an I/O or network failure, the active file handle is dropped and the temporary file is unlinked immediately (`tokio::fs::remove_file` locally or `sftp.remove_file` remotely). Partial corrupted files are never left in the target destination.
  - Completed files are verified for size match and atomically promoted using POSIX rename.

### 2.6 Remote Edit Conflict Detection and Workspace Hygiene
- **Severity:** Medium
- **Status:** **PASS / Hardened**
- **Analysis:**
  - Remote edit working copies are stored in a managed, dedicated state directory (`$XDG_STATE_HOME/linuxscp/remote-edits/<session_uuid>/`).
  - `ensure_private_dir` enforces mode `0700` and validates that the directory is owned by the current user's EUID.
  - Remote file manifests store SHA-256 content hashes and metadata fingerprints (size, mtime).
  - If a remote file changes before local edits are saved back, LinuxSCP detects the conflict and prevents overwriting without explicit user confirmation.
  - On application shutdown, `cleanup_on_exit()` verifies that clean sessions without unsaved edits are unlinked from disk, while modified sessions are preserved across restarts.

### 2.7 Elevated SFTP (sudo -n) and Dangerous Operations
- **Severity:** Medium-High
- **Status:** **PASS / Hardened**
- **Analysis:**
  - Passwordless sudo is not run indiscriminately; sudo elevation is an explicit, per-connection opt-in setting (`profile.sudo_sftp.enabled`).
  - It executes `sudo -n <sftp_server_path>` over the SSH exec channel.
  - If the server configuration requires interactive password input, LinuxSCP aborts the handshake with an actionable diagnostic message detailing the required narrow sudoers rule (`<user> ALL=(root) NOPASSWD: <binary>`).
  - The UI displays persistent, high-visibility warning badges in both the Remote Pane header and Status Bar when connected in elevated mode.
  - Deletion of files and recursive deletion of directories trigger confirmation dialogs with explicit "ELEVATED (root)" warnings.

### 2.8 TLS Certificate Validation for FTPS (Deferred Implementation)
- **Severity:** Informational / Standard Architecture Requirement
- **Status:** **PASS (Properly Deferred & Guarded)**
- **Analysis:**
  - `ProtocolType::Ftps` and `ProtocolType::Ftp` are defined in profile types.
  - Any connection attempt returns `AppError::Protocol("Protocol FTPS is not yet supported for connection establishment")`.
  - No partial, insecure, or mock TLS implementation exists.
  - **Required Security Invariants for Future FTPS Milestone:**
    1. System CA root certificates must be loaded (via `rustls-native-certs` or `webpki-roots`).
    2. Server certificate hostname verification (SNI and Subject Alternative Name / Common Name validation) must be strictly enforced.
    3. Revocation checking / TLS 1.2+ minimum version must be enforced.
    4. Reject any configuration flag disabling certificate verification (`insecure = false`).

### 2.9 SQLite Migrations and Data Integrity
- **Severity:** Low-Medium
- **Status:** **PASS**
- **Analysis:**
  - Schema migrations are tracked in `schema_migrations` and executed inside individual database transactions (`apply_migration_1` through `apply_migration_5`).
  - Unit tests verify that applying migrations in memory and re-running them is idempotent and schema-consistent.

---

## 3. Usability Verification Pass

| Usability Requirement | Expected Behavior | Observed Code & UI Behavior | Status |
| :--- | :--- | :--- | :--- |
| **File-Column Order** | `Name, Size, Changed, Rights, Owner` | `ColumnHeader.tsx` and `FileRow.tsx` define and render exact order: 1. Name, 2. Size, 3. Changed, 4. Rights, 5. Owner. | **CONFIRMED** |
| **Navigation Controls** | Back, Forward, Up, Home, Root, Refresh, Direct Path | `AddressBar.tsx` contains interactive controls with keyboard shortcuts (Alt+Left, Alt+Right, Alt+Up) and escape recovery. | **CONFIRMED** |
| **Directory Restoration** | Restore last visited remote directory per profile | Stored in `connection_remote_state` table; restored during connection handshake with fallback to initial directory or home. | **CONFIRMED** |
| **Pane Independence** | Local and Remote panes maintain isolated state | `LocalPane` and `RemotePane` maintain completely separate React state (paths, selection, history, sorting). | **CONFIRMED** |
| **Drag-and-Drop** | Internal-only drag-and-drop between panes | `usePaneDragDrop.ts` and `dragDrop.ts` enforce `INTERNAL_DRAG_MIME`; drops with external files or within the same pane are rejected. | **CONFIRMED** |
| **Open With Defaults** | Default app never removes manual choice | Context menu retains "Open With..."; `OpenWithDialog` provides manual selection search and default-clearing trash icon. | **CONFIRMED** |
| **Real Transfer Progress** | Authentic byte-driven progress | Chunk-based streaming (`CHUNK_SIZE`) updates progress directly from read/write counts without artificial delays. | **CONFIRMED** |
| **Elevated Mode Notice** | High-contrast visual indication | Prominent amber `ELEVATED / ROOT` badge in pane header and status bar; warnings in confirmation dialogs. | **CONFIRMED** |

---

## 4. Defects Found & Remediations Applied

1. **Defect:** Explicit auto-deref clippy warnings in `src-tauri/src/protocol/scp.rs` (`&*handle_guard` -> `&handle_guard`).
   - **Severity:** Low (Code Quality / Compiler Hygiene)
   - **Remediation:** Corrected references in test harness; clean build with `-D warnings`.
2. **Defect:** Inconsistent Prettier code style in `src/components/FilePanel/RemotePane.tsx`.
   - **Severity:** Low (Code Style)
   - **Remediation:** Formatted with Prettier to conform to project standards.
3. **Defect:** Lack of automated test coverage verifying that unsupported protocols (FTP/FTPS) fail safely with explicit protocol errors rather than panics or silent fallbacks.
   - **Severity:** Medium (Safety Verification)
   - **Remediation:** Added `test_unsupported_protocols_fail_safely_without_unverified_fallback` to `src-tauri/src/protocol/session_manager.rs`.

---

## 5. Known Limitations & Remaining Risks

1. **Headless Linux Environments without Secret Service:**
   - LinuxSCP depends on the Linux Secret Service D-Bus specification (e.g. `gnome-keyring`, `ksecretsservice`, or `keepassxc`).
   - In environments where no Secret Service daemon is running, attempting to save or retrieve credentials returns an explicit `AppError::Security` error. Plaintext fallback is intentionally disallowed.
2. **SCP Recursive Transfers:**
   - As documented in `docs/SCP_SUPPORT.md`, the legacy SCP protocol lacks a standardized machine-readable directory metadata protocol. LinuxSCP supports atomic SCP file uploads and downloads, but marks directory browsing and recursive operations as unsupported when connected via SCP.
3. **FTPS Protocol:**
   - FTPS is defined in data models but currently deferred. Connections will fail safely until a validated TLS backend is integrated in a future milestone.

