---
name: ssh-sftp-development
description: >-
  Use this skill when implementing or modifying SSH connections, SFTP filesystem operations,
  SSH authentication (password, public key, agent), strict host-key verification,
  and sudo-elevated SFTP server execution for LinuxSCP.
---

# SSH & SFTP Protocol Development for LinuxSCP

This skill provides the technical implementation requirements, security constraints, and architecture for SSH and SFTP protocol handling in LinuxSCP.

---

## 1. Protocol Architecture & Abstraction

LinuxSCP implements a unified `RemoteFileSystem` trait across protocols, with SFTP as the primary P0/P1 protocol:

```rust
#[async_trait::async_trait]
pub trait RemoteFileSystem: Send + Sync {
    async fn connect(&mut self) -> Result<(), ProtocolError>;
    async fn disconnect(&mut self) -> Result<(), ProtocolError>;
    async fn list_directory(&self, path: &str) -> Result<Vec<RemoteFile>, ProtocolError>;
    async fn stat(&self, path: &str) -> Result<RemoteMetadata, ProtocolError>;
    async fn create_file(&self, path: &str) -> Result<(), ProtocolError>;
    async fn create_directory(&self, path: &str) -> Result<(), ProtocolError>;
    async fn rename(&self, old_path: &str, new_path: &str) -> Result<(), ProtocolError>;
    async fn delete_file(&self, path: &str) -> Result<(), ProtocolError>;
    async fn delete_directory(&self, path: &str) -> Result<(), ProtocolError>;
    async fn set_permissions(&self, path: &str, mode: u32) -> Result<(), ProtocolError>;
    async fn set_owner_group(&self, path: &str, uid: Option<u32>, gid: Option<u32>) -> Result<(), ProtocolError>;
}
```

### Capability Matrix
Different servers and protocols offer varying features. Always verify capabilities before executing operations:
- Query support for numeric UID/GID, chmod, symlinks, and recursive stat.
- Do not silently fall back to shell command execution when a protocol operation is unsupported. Return explicit capability errors.

---

## 2. SSH Host-Key Verification (Strict Security)

LinuxSCP requires strict SSH host-key verification by default:

1. **Known-Hosts Verification:**
   - Check presented public keys against the user's `~/.ssh/known_hosts` or a dedicated LinuxSCP known-hosts store.
2. **First-Connection Prompt:**
   - If a host key is unknown, compute the SHA256 fingerprint and return a verification prompt to the user with the algorithm and fingerprint.
   - Only store and trust the key after explicit user confirmation.
3. **Changed Host Key (Strict Block):**
   - If a host key has changed from the stored fingerprint, **immediately abort the connection**.
   - Raise a prominent security warning to protect against Man-in-the-Middle (MITM) attacks.
   - Never allow automatic or silent trust of changed host keys.
4. **Policy Enforcement:**
   - Under no circumstances allow a default "accept-all-keys" policy in production code.

---

## 3. Authentication Mechanisms

Support standard OpenSSH authentication modes:
1. **Password Authentication:** Prompted or fetched from the Linux Secret Service. Never stored in plaintext.
2. **Public Key Authentication:** Load OpenSSH or PKCS#8 private keys with optional passphrase decryption.
3. **SSH Agent Support:** Query standard `SSH_AUTH_SOCK` for active keys where supported.
4. **Order of Preference:** Public Key -> SSH Agent -> Password (configurable per profile).

---

## 4. Sudo-Enabled SFTP & Elevated Sessions

A critical differentiator for LinuxSCP is managing root-owned and restricted remote files via an elevated SFTP session:

### Preferred Execution Flow
```text
1. Authenticate normal SSH session (e.g., user 'deploy')
2. Open a dedicated SSH session channel (Channel Type: session)
3. Execute remote command via sudo:
   sudo -n /usr/lib/openssh/sftp-server
4. Attach SFTP protocol client to stdin/stdout of this remote process
5. All SFTP packet operations now execute with root/elevated privileges
```

### Technical Requirements & Constraints
1. **Stream Cleanliness:** The remote process's standard output must be a pure, uncorrupted binary SFTP stream. Any MOTD banners, shell login greetings, or diagnostic printouts will corrupt the SFTP packet stream.
2. **Non-Interactive Sudo (`-n`):**
   - Execute with `sudo -n` to prevent blocking the channel waiting for an interactive password prompt.
   - If the remote user requires interactive password entry for sudo, return a clear, actionable error instructing the user on server sudoers requirements.
3. **Configurable Server Path:**
   - The path to `sftp-server` varies across Linux distributions:
     - Debian/Ubuntu: `/usr/lib/openssh/sftp-server`
     - RHEL/CentOS/Fedora: `/usr/libexec/openssh/sftp-server`
     - Arch Linux: `/usr/lib/ssh/sftp-server`
   - Allow setting a custom path per connection profile.
4. **`internal-sftp` Incompatibility:**
   - OpenSSH configurations using `Subsystem sftp internal-sftp` cannot be invoked as an external binary through sudo.
   - Detect and report this scenario clearly to the user.
5. **Separation from SSH Terminal:**
   - An elevated SFTP channel does **not** alter or elevate an existing or subsequent SSH shell terminal. They are distinct sessions and processes.

---

## 5. File Permissions & Metadata Handling

- **Mode Bits:** Preserve Unix mode bits (`st_mode`), extracting file type and permission flags (`rwxrwxrwx`, SUID, SGID, Sticky bit).
- **UID / GID Operations:**
  - SFTP transfers numeric UID/GID values.
  - Present numeric IDs if remote user/group names cannot be resolved.
  - When setting ownership, handle lack of privilege gracefully (`EPERM` / Permission Denied).

---

## 6. Common Pitfalls to Avoid

- **Inventing Crate APIs:** Do not guess async methods or channel traits for Rust SSH crates (`russh`, `ssh2`, etc.). Always verify method signatures against the tested version during technical spikes.
- **Leaking Secrets in Errors:** Do not format passwords, private key buffers, or passphrases into error strings or logs.
- **Unbounded Buffering:** Avoid reading entire remote files into in-memory byte arrays. Always use chunked streams.

---

## 7. Testing & Verification

- Test against a local containerized OpenSSH instance:
  - Standard user login with key and password.
  - Sudoer user configured with `deploy ALL=(root) NOPASSWD: /usr/lib/openssh/sftp-server`.
  - Verify directory listing, upload, download, and permission modification on root-owned paths.
- Reference documentation: `docs/LinuxSCP_Product_and_Technical_Specification.md` Sections 6, 7, and 8.

