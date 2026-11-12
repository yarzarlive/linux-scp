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

