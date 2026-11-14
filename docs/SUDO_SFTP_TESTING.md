# LinuxSCP — Sudo-Enabled SFTP Verification Guide & Test Procedure

This document provides a security-focused manual testing procedure for LinuxSCP's sudo-enabled SFTP feature using a disposable container or virtual machine.

---

## 1. Background & Architecture Verification

LinuxSCP supports elevated SFTP by:
1. Authenticating to SSH as an unprivileged login account (e.g., `ubuntu` or `deploy`).
2. Opening an independent SSH session channel via `channel_open_session()`.
3. Executing the external SFTP server binary with non-interactive sudo:
   ```bash
   sudo -n /usr/lib/openssh/sftp-server
   ```
4. Attaching the client's `SftpSession` protocol stream directly to that channel's stdout/stdin.
5. Displaying persistent `ELEVATED / ROOT` badges in both the Remote Filesystem pane header and the application status bar.
6. Guarding destructive operations (recursive directory deletions) with heightened confirmation dialogs.

### Why Non-Interactive `sudo -n` is Required
Binary SFTP client and server communicate via packetized protocol bytes. If `sudo` stops to prompt for an interactive password (e.g. `[sudo] password for user:`), or if a shell emits banner messages to stdout, the handshake bytes are corrupted and the connection fails. For this reason, `sudo -n` is mandatory.

---

## 2. Server-Side Sudoers Configuration

### Recommended Narrow Rule
On the target server/VM, create `/etc/sudoers.d/99-linuxscp-sftp` with permissions `0440`:

```sudoers
# Narrow rule: allow only the specific sftp-server binary without password
ubuntu ALL=(root) NOPASSWD: /usr/lib/openssh/sftp-server
```

*(For RHEL/Fedora/CentOS/Arch, substitute `/usr/libexec/openssh/sftp-server`)*

> [!WARNING]
> **Never configure broad passwordless sudo:**
> ```sudoers
> # DO NOT USE:
> ubuntu ALL=(ALL) NOPASSWD: ALL
> ```
> LinuxSCP strictly adheres to the principle of least privilege and never attempts to modify `/etc/sudoers` or `/etc/sudoers.d/` automatically.

---

## 3. Step-by-Step Test Procedure in a Disposable VM

### Phase 1: Environment Setup
1. Launch a disposable Ubuntu VM (e.g. `192.168.122.10`) with OpenSSH server running:
   ```bash
   sudo apt-get update && sudo apt-get install -y openssh-server
   ```
2. Create a protected root-only test area:
   ```bash
   sudo mkdir -p /root/protected_area
   sudo sh -c 'echo "root secret content" > /root/protected_area/secret.conf'
   sudo chmod 700 /root/protected_area
   sudo chmod 600 /root/protected_area/secret.conf
   ```

### Phase 2: Negative Test — Unconfigured Sudoers (Interactive Required)
1. Ensure the user does **not** have `NOPASSWD` configured for `sftp-server`.
2. In LinuxSCP Site Manager:
   - Configure connection to the VM with username `ubuntu` and key/password.
   - Check **Enable Sudo-SFTP External Server Elevation**.
   - Set binary path: `/usr/lib/openssh/sftp-server`.
   - Click **Connect**.
3. **Expected Behavior:**
   - LinuxSCP attempts the channel execution.
   - Because `sudo -n` exits with code 1 (`sudo: interactive authentication is required`), the SFTP handshake fails safely.
   - LinuxSCP displays an actionable error message detailing that non-interactive sudoers `NOPASSWD` is required, and does not hang the UI.

### Phase 3: Positive Test — Configured Narrow Sudoers
1. In the VM, add the narrow rule:
   ```bash
   echo "ubuntu ALL=(root) NOPASSWD: /usr/lib/openssh/sftp-server" | sudo tee /etc/sudoers.d/99-linuxscp-sftp
   sudo chmod 0440 /etc/sudoers.d/99-linuxscp-sftp
   ```
2. In LinuxSCP, reconnect to the profile.
3. **Expected Behavior:**
   - Connection succeeds immediately.
   - The Remote Filesystem pane header displays an unmistakable **`ELEVATED / ROOT (root)`** badge.
   - The status bar at the bottom displays **`ELEVATED / ROOT (root)`**.
   - Navigate to `/root/protected_area`:
     - LinuxSCP successfully lists the directory contents.
     - `secret.conf` is visible with permissions `0600` owned by `root`.
   - Open `secret.conf` via Open With / Remote Edit:
     - The file downloads safely into the managed temporary editing copy.
     - Saving local edits uploads the file back to `/root/protected_area/secret.conf` while preserving root ownership.

### Phase 4: Destructive Operation Safety Confirmation
1. In `/root/protected_area`, create a subfolder with a test file:
   ```bash
   sudo mkdir -p /root/protected_area/test_subfolder
   sudo touch /root/protected_area/test_subfolder/nested.txt
   ```
2. In LinuxSCP Remote Pane, select `test_subfolder` and press Delete / click Trash:
   - Non-recursive delete notices the directory is not empty and halts.
3. When recursive directory deletion is requested:
   - An explicit confirmation modal appears with a bold elevated warning:
     `"WARNING (ELEVATED ROOT SESSION): The following remote directory/directories are NOT empty. Confirming will permanently delete ALL nested files and subdirectories inside them."`
   - Confirming deletes the folder as root.

### Phase 5: Cleanup
1. In the VM:
   ```bash
   sudo rm -f /etc/sudoers.d/99-linuxscp-sftp
   sudo rm -rf /root/protected_area
   ```
