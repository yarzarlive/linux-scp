---
name: secure-file-transfer
description: >-
  Use this skill when developing, refactoring, or reviewing the transfer engine, chunked streaming,
  transfer queue and progress reporting, atomic transfer safety, conflict policies,
  and Linux Secret Service credential storage for LinuxSCP.
---

# Secure File Transfer & Credential Storage for LinuxSCP

This skill provides guidelines and security requirements for building LinuxSCP's background file transfer engine, atomic transfer workflows, progress reporting, and secure credential handling.

---

## 1. Streaming I/O & Memory Safety

1. **Chunked Streaming:**
   - Always transfer files using bounded chunks (e.g., 32 KB to 256 KB).
   - **NEVER load entire files into memory buffers (`Vec<u8>`).**
   - Use Tokio async stream interfaces (`tokio::io::AsyncRead`, `AsyncWrite`) bridged to protocol channels.
2. **Resource Throttling & Queue Concurrency:**
   - Manage transfers through a central queue with a configurable concurrency limit (default: 2–4 concurrent files).
   - Queue states must follow explicit transitions:
     `Queued -> Connecting -> Transferring -> Verifying -> Completed | Cancelled | Failed`
3. **Throttled Progress Events:**
   - Do not emit a Tauri IPC event for every read/write chunk.
   - Throttle progress event emissions to 50–100ms intervals to keep the UI smooth and responsive without IPC bus saturation.
   - Calculate transferred bytes, total size, current speed (rolling average over 1–2 seconds), and estimated time remaining (ETA).

---

## 2. Transfer Safety & Atomic Completion

Where the filesystem and protocol permit:

```text
1. Open transfer to a temporary target name:
   destination/filename.ext.linuxscp-tmp-<uuid>
2. Stream file chunks and verify written byte count against source
3. Flush/close remote or local file handle
4. Rename temporary file to target destination filename:
   destination/filename.ext
5. On cancellation or failure:
   Cleanly remove the temporary file to prevent partial corrupt artifacts
```

*Note:* Atomic rename behavior is protocol- and server-dependent. Never present atomic rename guarantees on protocols or filesystems that cannot support it.

---

## 3. Conflict Resolution Policies

When a destination file already exists, LinuxSCP enforces explicit conflict policies:
- **Ask (Default):** Emit an event to prompt the user with source and target metadata (size, modification time).
- **Overwrite:** Replace the existing file.
- **Overwrite if Newer:** Compare modification timestamps; overwrite only if source is newer.
- **Resume:** Continue transferring from the existing byte offset if supported by the protocol.
- **Skip / Rename:** Skip transfer or save as `filename (1).ext`.

---

## 4. Recursive Directory Transfers

- Recursively scan directory trees with depth limits to avoid symlink cycles.
- Create parent directories before transferring child files.
- **Partial Failure Reporting:** If individual files fail in a multi-file or directory transfer:
  - Record the error for each specific failed file.
  - Continue or halt based on user policy.
  - Never report a batch transfer as complete success if any file failed.

---

## 5. Credential Storage via Linux Secret Service

### Security Requirements
1. **Never Store Passwords in Plaintext:**
   - Passwords and private key passphrases must **never** be saved in SQLite, JSON files, environment variables, or application logs.
2. **OS Keyring Integration:**
   - Use the Linux Secret Service DBus API (via crates such as `keyring`).
   - Store credentials using standardized attributes:
     - Service: `com.linuxscp.LinuxSCP`
     - Account: `connection-<connection_id>-password` or `connection-<connection_id>-passphrase`
3. **Missing Keyring Handling:**
   - If Secret Service daemon (e.g., GNOME Keyring, KWallet) is unavailable:
     - Warn the user explicitly.
     - Keep credentials only in volatile session memory.
     - Do not silently fall back to plaintext file storage.
4. **Memory Scrubbing:**
   - Zero out in-memory credential buffers (`zeroize` crate) when sessions terminate.

---

## 6. Common Pitfalls to Avoid

- **Memory Exhaustion:** Using `tokio::fs::read` or reading entire streams into memory causes crashes on large files.
- **Orphaned Temporary Files:** Failing to delete partial `.linuxscp-tmp` files when transfers are cancelled or interrupted.
- **Symlink Recursion Loops:** Traversing circular symbolic links during recursive transfers.
- **Leaking Credentials in IPC:** Passing passwords in unredacted IPC command arguments or debug messages.

---

## 7. Testing & Verification

- Test chunked streaming with large simulated files (e.g., 1GB+) verifying stable memory usage.
- Test cancellation during active transfers to verify prompt termination and temporary file cleanup.
- Test Secret Service integration with both available and unavailable keyring daemons.
- Reference documentation: `docs/LinuxSCP_Product_and_Technical_Specification.md` Sections 6.4, 9.4, 11, and 18.

