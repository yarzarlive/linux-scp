# LinuxSCP — System Architecture & Domain Model Blueprint

**Status:** Approved Architectural Baseline  
**Revision:** 1.0 (Milestone 2)  
**Target Environment:** Linux Desktop (Tauri 2, Rust, React, TypeScript, Tokio)  

---

## 1. Architectural Overview & Boundaries

LinuxSCP follows a strict layered topology. The React frontend never interacts directly with low-level protocol crates or the underlying filesystem. All interactions pass through typed IPC commands and events into a unified domain service layer.

```text
+-------------------------------------------------------------------------+
|                  React 18 + TypeScript Presentation Layer               |
|      Dual Panes       Navigation Toolbar       Transfer Queue UI        |
+------------------------------------+------------------------------------+
                                     | Typed Tauri IPC (Commands & Events)
+------------------------------------v------------------------------------+
|                         Tauri 2 IPC Command Boundary                    |
|   commands/connection   commands/filesystem   commands/transfers        |
+------------------------------------+------------------------------------+
                                     |
+------------------------------------v------------------------------------+
|                         Application Service Layer                       |
|   Session Registry     Local Filesystem Engine     Transfer Queue       |
+------------------------------------+------------------------------------+
                                     |
                     +---------------+---------------+
                     |                               |
+--------------------v--------------------+   +------v--------------------+
|         RemoteFileSystem Trait          |   |      Persistence Layer    |
|   (Async Domain Protocol Abstraction)   |   |                           |
|  +------------+------------+---------+  |   |  - SQLite (Metadata)      |
|  |    SFTP    |  FTP/FTPS  |   SCP   |  |   |  - Secret Service (DBus)  |
|  +------------+------------+---------+  |   |                           |
+-----------------------------------------+   +---------------------------+
```

---

## 2. Protocol Capability Model & Boundaries

Different protocols offer vastly different filesystem and administrative operations. LinuxSCP rejects the anti-pattern of assuming all protocols are identical, and strictly forbids silently executing shell commands to compensate for protocol deficits.

### 2.1 Capability Matrix

| Capability | SFTP | FTP | FTPS | SCP | Notes |
|---|:---:|:---:|:---:|:---:|---|
| **List Directory** | ✅ | ✅ | ✅ | ❌ | SCP does not support standard interactive directory browsing. |
| **Read Metadata (stat)** | ✅ | ✅ | ✅ | ❌ | SFTP returns rich numeric UID/GID and mode bits. |
| **Upload / Download** | ✅ | ✅ | ✅ | ✅ | All protocols support file streaming. |
| **Create File / Dir** | ✅ | ✅ | ✅ | ❌ | Native SFTP/FTP commands; SCP relies on batch file copies. |
| **Rename / Move** | ✅ | ✅ | ✅ | ❌ | Atomic rename where supported by remote server. |
| **Delete (File / Dir)** | ✅ | ✅ | ✅ | ❌ | Interactive delete is native in SFTP and FTP. |
| **Set Permissions (chmod)** | ✅ | ❌ | ❌ | ❌ | SFTP supports `fchmod`/`setstat`. FTP `SITE CHMOD` is unstandardized. |
| **Set Owner / Group (chown)**| ✅ | ❌ | ❌ | ❌ | SFTP supports numeric UID/GID modification when elevated. |
| **Sudo Elevation** | ✅ | ❌ | ❌ | ❌ | Executes `sudo -n <sftp-server>` over dedicated SSH channel. |
| **Resume Interrupted Transfer**| ✅ | ✅ | ✅ | ❌ | Supported via byte offset seeking. |
| **Symlink Resolution & Creation**| ✅ | ❌ | ❌ | ❌ | SFTP provides explicit symlink and readlink packets. |

### 2.2 Capability Enforcement Strategy
Before invoking any remote operation, the application calls:
```rust
self.capabilities().verify_operation("set_permissions")?;
```
If unsupported, an explicit `CapabilityError::UnsupportedOperation` is returned, preventing unexpected failures or corrupted state.

---

## 3. Core Domain Models

### 3.1 Connection Profile & Secrets Isolation
- **Non-Secret Profile:** All profile metadata (hostname, port, username, authentication type, directory restoration paths) is serialized into SQLite.
- **Zero-Plaintext Credential Invariant:** Passwords, private-key passphrases, and tokens are **never** stored in SQLite or application configuration. They are resolved at connection time through the Linux Secret Service over DBus (`org.freedesktop.secrets`).

### 3.2 Filesystem & Metadata Representation
- **Explicit Unknowns:** Unknown metadata is represented as `None`, explicitly distinct from `Some(0)` (e.g. unknown file size vs an empty 0-byte file).
- **Unix Permissions Matrix:**
  - 12 distinct flags: read/write/execute for User, Group, Others, plus SUID, SGID, and Sticky bit.
  - Bidirectional lossless conversion between numeric mode (`0o755`), octal string (`"0755"`), and symbolic representation (`"rwxr-xr-x"`).
- **Ownership:** Preserves raw numeric UID/GID alongside resolved human-readable username/group name.

### 3.3 Transfer Jobs & Concurrency
- **Stable Identifiers:** Every transfer job receives a unique UUID (`TransferId`).
- **State Machine:**
  ```text
  [Queued] ---> [Connecting] ---> [Transferring] ---> [Verifying] ---> [Completed]
      |              |                  |                  |
      +--------------+------------------+------------------+---------> [Cancelled]
      |              |                  |                  |
      +--------------+------------------+------------------+---------> [Failed]
  ```
- **Conflict Policies:**
  - `Ask`: Prompt user with metadata diff.
  - `Overwrite`: Replace destination.
  - `OverwriteIfNewer`: Compare timestamps.
  - `Skip`: Retain existing destination.
  - `Rename`: Append numbered suffix (`file (1).ext`).
- **Atomic Safety:** Stream into temporary file (`.linuxscp-tmp-<uuid>`) before final rename, ensuring partial transfers are never mistaken for completed files.

---

## 4. Security & Safety Invariants

1. **Host-Key Verification:** Host keys are strictly verified against `~/.ssh/known_hosts`. Unknown keys require explicit SHA256 fingerprint confirmation. Changed keys abort immediately with security warnings.
2. **Credential Scrubbing:** Sensitive strings in memory are wrapped in zeroizing buffers and excluded from debug/tracing logs.
3. **No Shell Interpolation:** Desktop applications and external commands are spawned via `std::process::Command` using structured argument vectors, eliminating shell injection vulnerabilities.
4. **Internal Drag-and-Drop Only:** Drag-and-drop operations are bounded strictly between LinuxSCP's local and remote panes; desktop file dropping is unsupported.
5. **Elevated (Sudo) SFTP Protocol Channel Isolation:**
   - When elevated SFTP is enabled for a profile, LinuxSCP opens an independent SSH session channel and executes the configured external binary path using non-interactive sudo (`sudo -n <sftp_server_path>`).
   - The SFTP protocol stream is attached strictly to this process channel. Non-interactive execution is enforced (`-n`) because interactive password prompts or arbitrary shell MOTD/startup messages corrupt the binary SFTP handshake.
   - `internal-sftp` daemon mode cannot be elevated externally and is rejected during validation.
   - Persistent, unmistakable `ELEVATED / ROOT` badges are displayed in the remote filesystem pane and application status bar.
   - Recursive deletion and destructive operations require additional explicit confirmation under elevated sessions.
   - An administrator must configure a narrowly scoped rule in `/etc/sudoers.d/`, e.g. `<user> ALL=(root) NOPASSWD: <sftp_server_path>`. LinuxSCP never attempts to modify sudoers automatically and never recommends `NOPASSWD: ALL`.

---

## 5. Remote File Editing & Temporary Working-Copy Lifecycle

When a user selects **Open** or **Open With...** on a remote file, LinuxSCP manages a private local working copy through `RemoteEditManager`:

### 5.1 Workspace Layout & Permissions
Working copies live under `$XDG_STATE_HOME/linuxscp/remote-edits/<session-uuid>/` (falling back to `~/.local/share/linuxscp/remote-edits/<session-uuid>/`):
- All directories are created and verified with Unix mode `0700` owned by the current UID; symlinks and foreign-owned directories are rejected.
- `session.json` (mode `0600`): non-secret manifest recording remote endpoint identity (`host`, `port`, `username`, `elevated`), canonical `remote_path`, baseline `RemoteFileFingerprint` (`size`, `modified_at_secs`, `permissions_mode`, `uid`, `gid`), and `baseline_sha256`.
- `work/<filename>` (mode `0600`): the local working copy passed to the selected desktop application.
- `scratch/` (mode `0700`): transient staging files for atomic downloads, upload snapshots, and content verification, removed immediately after each operation.

`$XDG_STATE_HOME` is chosen deliberately over `$XDG_RUNTIME_DIR` (`tmpfs`) so that unsynced local edits survive unexpected logouts, disconnects, or crashes.

### 5.2 Change Detection & Conflict Safety
- **Process-Independent Detection:** LinuxSCP never assumes that a desktop launcher process exiting means the file was saved or closed (e.g., VS Code or GNOME Text Editor single-instance launchers exit immediately while the window stays open). Instead, it monitors `work/<filename>` via background stat/hash polling (with a stability debounce window so atomic-rename saves and multi-write saves settle first) plus an immediate scan whenever the LinuxSCP window regains focus.
- **Upload Mode (`Auto` by default, configurable to `Prompt`):** When a local modification settles and `RemoteEditUploadMode` is `Auto` (persisted in SQLite `app_preferences` under `remote_edit.upload_mode`), `scan_local_changes` automatically triggers a conflict-checked upload (`upload(..., overwrite_remote_changes = false)`). Users can switch to `Prompt` mode via the **Auto-upload on save** toggle in the `RemoteEditPanel` header if they prefer manual confirmation before each upload.
- **No Unchanged Uploads:** If the working copy's SHA-256 equals `baseline_sha256` (including when an editor re-saves identical content), the session remains `Clean` and no upload occurs.
- **Endpoint Binding:** Before uploading, LinuxSCP verifies that the active session matches the recorded `RemoteEndpointIdentity` (`host`, `port`, `username`, `elevated`), preventing a working copy from being uploaded to a different server after a reconnect.
- **Remote Conflict Check:** Before uploading (unless the user explicitly confirms overwriting a reported conflict), LinuxSCP re-stats the remote path and compares `(size, modified_at_secs)` against the baseline, supplemented by a remote content SHA-256 check for files up to 16 MiB. If the remote file changed, was deleted, or lacks verifiable metadata, the session enters `Conflict` and is **never** overwritten automatically.
- **Re-opening & Reloading Remote Changes:** When a user opens a remote file that already has an active session (`refresh_existing`), LinuxSCP first scans `work/<filename>` for local edits. If the local working copy is `Clean`, it runs `verify_remote_unchanged` (fingerprint + content SHA-256) and automatically re-downloads the newer remote content into `work/<filename>` before launching the editor. If the local working copy has unsynced edits and the remote file also changed externally, the session transitions to `Conflict` (preserving the local working copy) and offers **Load Remote Version** (`reload_remote_edit_from_remote`) to pull the latest remote file and reopen it in the editor.

### 5.3 Temporary-File Cleanup Lifecycle
1. **Failed or Cancelled Download:** If the initial download or baseline verification fails, `<session-uuid>/` is removed immediately before any application is launched.
2. **Scratch Files:** `scratch/*.part` files created during download, upload snapshotting, or remote hash verification are removed in a `finally`-style cleanup block as soon as the operation finishes.
3. **Explicit Close / Discard:**
   - If the session is `Clean` (`local_sha256 == baseline_sha256`), **Close & Clean Up** removes `<session-uuid>/` immediately without prompting.
   - If the session has unsynced local changes (`Modified`, `Conflict`, `UploadFailed`, or `LocalMissing`), `discard` refuses unless the user explicitly confirms `discard_local_changes = true`.
4. **Application Exit (`cleanup_on_exit`):** When the LinuxSCP window closes, every session marked `Clean` has its working copy re-hashed on disk; only sessions whose on-disk SHA-256 still matches `baseline_sha256` are deleted. Any session with unsynced local edits, a conflict, or a failed upload is **preserved** on disk.
5. **Application Startup (`restore_from_disk`):** On startup, LinuxSCP scans `remote-edits/`. Directories without a `session.json` manifest (incomplete downloads) are removed; valid sessions are restored with `recovered = true` and immediately scanned so the user can upload, save a copy, or discard their preserved edits.


