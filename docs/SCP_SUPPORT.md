# LinuxSCP: SCP Protocol Implementation & Capability Boundaries

## 1. Executive Summary & Design Rationale

In LinuxSCP, **SCP (Secure Copy Protocol) is treated as a distinct, transfer-only legacy backend**, separate from SFTP (SSH File Transfer Protocol). 

Unlike modern file transfer protocols, SCP operates purely as an exec-channel streaming protocol over SSH (typically invoking remote commands such as `scp -t` for sink/upload and `scp -f` for source/download). It does not provide remote directory browsing, file attribute listing, filesystem metadata querying, renaming, deletion, permission editing (`chmod`), or ownership management (`chown`).

LinuxSCP strictly adheres to the principle of **honest capability modeling**:
1. It does not present unsupported directory browsing or file-management controls as available.
2. It does not attempt unsafe workarounds such as executing remote shell commands (`ls`, `rm`, `mv`, `stat`) over SSH to emulate a virtual filesystem.
3. It does not silently fall back from SFTP to SCP or from SCP to SFTP.
4. It enforces the same strict host-key verification and Linux Secret Service credential storage across both protocols.

---

## 2. Protocol Capabilities Matrix: SFTP vs. SCP

| Capability | SFTP (`ProtocolType::Sftp`) | SCP (`ProtocolType::Scp`) | LinuxSCP Enforcement |
| :--- | :---: | :---: | :--- |
| **Directory Browsing (`can_list_directory`)** | ✅ Supported | ❌ **Unsupported** | Rejects `list_directory` in `ScpFileSystemAdapter` with `AppError::Protocol`; UI displays Transfer-Only panel. |
| **Metadata / Stat (`can_stat_metadata`)** | ✅ Supported | ❌ **Unsupported** | Rejects `stat` in `ScpFileSystemAdapter`; direct downloads proceed without pre-stat probing. |
| **File Upload (`can_upload`)** | ✅ Supported | ✅ **Supported** | Executed via `scp -t` sink streaming with 64 KB chunking, progress updates, and cancellation. |
| **File Download (`can_download`)** | ✅ Supported | ✅ **Supported** | Executed via `scp -f` source streaming with atomic `.linuxscp-tmp-<uuid>` staging. |
| **File Creation (`can_create_file`)** | ✅ Supported | ❌ **Unsupported** | Rejects with `AppError::Protocol`; toolbar button hidden/disabled. |
| **Directory Creation (`can_create_directory`)** | ✅ Supported | ❌ **Unsupported** | Rejects with `AppError::Protocol`; toolbar button hidden/disabled. |
| **Rename Entry (`can_rename`)** | ✅ Supported | ❌ **Unsupported** | Rejects with `AppError::Protocol`; toolbar button hidden/disabled. |
| **Delete File (`can_delete_file`)** | ✅ Supported | ❌ **Unsupported** | Rejects with `AppError::Protocol`; toolbar button hidden/disabled. |
| **Delete Directory (`can_delete_directory`)** | ✅ Supported | ❌ **Unsupported** | Rejects with `AppError::Protocol`; toolbar button hidden/disabled. |
| **Chmod Permissions (`can_set_permissions`)** | ✅ Supported | ❌ **Unsupported** | Rejects with `AppError::Protocol`; Properties dialog disabled. |
| **Chown Ownership (`can_set_owner_group`)** | ✅ Supported | ❌ **Unsupported** | Rejects with `AppError::Protocol`; Properties dialog disabled. |
| **Sudo Elevation (`can_elevate_sudo`)** | ✅ Supported | ❌ **Unsupported** | Profile validation rejects profiles configured with both `Scp` and `ElevatedSudo`. |
| **Preserve Timestamps (`can_preserve_timestamps`)** | ✅ Supported | ✅ **Supported** | Sends and parses `T<mtime> 0 <atime> 0\n` records (`-p` flag). |
| **Transfer Queue Integration** | ✅ Full | ✅ **Full** | Enqueued via `TransferManager`; reports bytes, speed, ETA, and cancellation. |

---

## 3. Architecture & Security Implementation

### 3.1. Reusable SSH Transport Layer
The SSH transport setup is unified in `src-tauri/src/protocol/sftp.rs::establish_ssh_transport`. Both SFTP and SCP sessions route through the identical cryptographic validation pipeline:
- **Strict SSH Host-Key Verification**: Host keys are validated against SQLite `known_hosts` (`KnownHostsRepository`). Unknown or changed host keys trigger interactive approval or strict rejection; untrusted keys are never accepted automatically.
- **Secure Keyring Storage**: Passwords and private key passphrases are retrieved securely from the Linux Secret Service / Keyring API. Plaintext storage is forbidden.

### 3.2. SCP Wire Protocol Transfer Engine (`src-tauri/src/protocol/scp.rs`)
The SCP engine directly speaks the binary OpenSSH `scp` protocol over an SSH exec channel (`russh::Channel::into_stream()`):

- **Safe Command Construction & POSIX Quoting**:
  Remote commands are constructed using `posix_shell_quote` (single-quote wrapping with `'\''` escaping). Remote arguments are never passed raw to prevent shell command injection.
- **Upload (`upload_single_file_scp`)**:
  1. Opens an exec channel with `scp -t -p <remote_path>`.
  2. Awaits the remote server's `\0` null-byte acknowledgment.
  3. Sends timestamp header `T<mtime> 0 <atime> 0\n` and awaits `\0`.
  4. Sends file header `C<mode> <size> <filename>\n` and awaits `\0`.
  5. Streams file contents in 64 KB chunks, regularly checking for user cancellation (`cancel_rx`) and emitting progress events (`on_progress`).
  6. Sends terminal `\0` and verifies final remote status acknowledgment.
- **Download (`download_single_file_scp`)**:
  1. Opens an exec channel with `scp -f -p <remote_path>`.
  2. Sends initial `\0` to initiate remote stream.
  3. Parses incoming control messages:
     - `T`: parses modified and access timestamps.
     - `C`: parses octal permissions, file size, and remote filename.
     - `\x01` / `\x02`: parses remote error message and fails cleanly.
  4. Writes data into a temporary atomic staging file (`<target_local_path>.linuxscp-tmp-<uuid>`).
  5. On successful stream completion, atomically renames the staging file to the target local path and preserves mtime/atime. If cancelled or interrupted, the temporary file is deleted immediately.

### 3.3. Zero-Fallback Dispatching (`src-tauri/src/protocol/session_manager.rs`)
The `SessionManager` inspects `profile.protocol`:
- If `ProtocolType::Sftp`: invokes `establish_sftp_connection`.
- If `ProtocolType::Scp`: invokes `establish_scp_connection`.
- **Zero Fallback**: A failed SFTP connection will never attempt an automatic SCP fallback, and an SCP connection will never silently start SFTP. Connection modes are strictly explicit.

---

## 4. User Interface & Experience

When connected to an SCP session:
1. **Title Bar & Badges**: Displays `SCP (TRANSFER-ONLY)` badge in the Remote Pane title bar.
2. **Transfer-Only Panel (`ScpTransferPanel`)**:
   - Replaces the `FileTable` with an explanatory notice detailing supported transfers and explicitly unsupported browsing/management operations.
   - Shows the currently selected target remote upload directory (defaults to `~` or user-specified path).
   - Provides a prominent drag-and-drop drop zone that accepts dropped files and directories from the Local pane.
   - Provides a Direct Remote File Download form allowing direct input of remote file paths (e.g. `/etc/hosts` or `~/data.tar.gz`) for queuing downloads.
3. **Disabled Operations**:
   - Navigation buttons (Back, Forward, Up, Refresh, Toggle Hidden) are disabled in the address bar.
   - Toolbar buttons for New Folder, New File, Rename, Delete, and Properties are hidden or disabled.
4. **Status Bar**:
   - Surfaces: `SCP Mode • Direct transfer queue only (Browsing & file management unsupported)`.

---

## 5. Verification & Test Suite

The SCP implementation is verified by automated test suites in both Rust and TypeScript:

- **Rust Unit & Capability Tests (`src-tauri/src/protocol/scp.rs`)**:
  - `test_scp_capabilities_matrix_matches_honest_boundaries`: Verifies exact boolean flags of `ProtocolCapabilities::for_scp()`.
  - `test_scp_adapter_enforces_all_capability_rejections`: Confirms that `list_directory`, `stat`, `create_file`, `create_directory`, `rename`, `delete_file`, `delete_directory`, `set_permissions`, and `set_owner_group` all return typed `AppError::Protocol`.
  - `test_scp_rejects_non_scp_protocol_and_sudo_elevation`: Verifies profile rejection when protocol is not SCP or when sudo elevation is requested.
  - `test_live_scp_connection_and_transfers_against_test_vm`: Performs live SCP uploads and downloads against an OpenSSH VM (`192.168.122.10:22`), verifying round-trip data integrity, speed tracking, and atomic persistence.
- **Frontend Vitest Tests (`tests/unit/RemotePane.test.tsx`)**:
  - `renders honest transfer-only SCP view when connected via SCP without listing directory`: Verifies `listRemoteDirectory` is never invoked, the SCP badge and panel are rendered, unsupported toolbar buttons are omitted, and direct download queueing functions correctly.

