---
name: linux-desktop-integration
description: >-
  Use this skill when implementing Linux desktop features, including XDG application discovery,
  "Open With" file associations, external SSH terminal launching, and the temporary remote file
  editing and change-detection workflow.
---

# Linux Desktop Integration for LinuxSCP

This skill provides guidelines and security practices for integrating LinuxSCP with the Linux desktop environment, external desktop applications, file monitors, and system terminals.

---

## 1. Application Discovery via XDG Desktop Entries

LinuxSCP discovers installed desktop applications so users can open local and remote files with their preferred tools:

### Discovery Paths
Search in order according to the XDG Base Directory specification:
1. `~/.local/share/applications/` (User custom and overrides)
2. `$XDG_DATA_HOME/applications/` (if defined)
3. `/usr/local/share/applications/`
4. `/usr/share/applications/`
5. `$XDG_DATA_DIRS/applications/` (e.g., `/var/lib/flatpak/exports/share/applications`)

### Parsing `.desktop` Files
- Extract: `Name`, `Exec`, `Icon`, `MimeType`, `NoDisplay`, `Terminal`.
- Filter out entries where `NoDisplay=true` or `Type` is not `Application`.
- Use a stable desktop application ID (e.g., `code.desktop`, `org.gnome.TextEditor.desktop`).

---

## 2. Safe Application Launching (No Shell Injection)

### Exec Field Code Handling
Desktop entries specify argument handling using percent-codes:
- `%f`: A single file name/path.
- `%F`: Multiple file names/paths.
- `%u` / `%U`: URLs.
- Expand or strip field codes safely:
  - Replace `%f` with the single target file path.
  - Replace `%F` with the array of selected file paths.
  - Remove unsupported codes (`%i`, `%c`, `%k`).

### Security Rule: Direct Process Spawning
**NEVER execute applications through a shell string (e.g., `sh -c "$EXEC $FILE"`).**
Always launch using direct argument vectors:
```rust
use std::process::Command;

pub fn launch_application(executable: &str, args: &[String]) -> Result<(), IntegrationError> {
    Command::new(executable)
        .args(args)
        .spawn()
        .map_err(|e| IntegrationError::LaunchFailed(e.to_string()))?;
    Ok(())
}
```

---

## 3. Remembered File-Application Associations

### Application-Scoped Defaults
- LinuxSCP stores its own default application associations per extension.
- **Do NOT modify system-wide MIME configurations** (e.g., `mimeapps.list`). Store preferences inside LinuxSCP's SQLite database (`file_app_associations`).

### Extension Normalization
- Strip path, convert to lowercase, and preserve compound extensions where appropriate (e.g., `.tar.gz` vs `.gz`).
- Handle extensionless files with a dedicated fallback category.
- If a remembered application is uninstalled, gracefully prompt the user to choose another application.

---

## 4. Temporary Remote-File Editing Workflow

When a user selects **Open** or **Open With** on a remote file:

```text
[Remote Server]
       |
       | 1. Download file & record baseline metadata (mtime, size)
       v
[Local Temp Dir: $XDG_RUNTIME_DIR/linuxscp/edits/<id>/filename]
       |
       | 2. Launch desktop application (e.g., gedit, VS Code)
       | 3. File Watcher listens for modifications
       v
[File Saved by Local Editor]
       |
       | 4. Debounce event (e.g., 500ms stable window)
       | 5. Compare current remote stat with baseline metadata
       +---> If unchanged: Upload back to remote server
       +---> If changed: Raise Conflict Dialog to user
```

### Critical Implementation Details
1. **Managed Temporary Directory:** Place files in `$XDG_RUNTIME_DIR/linuxscp/edits/` (or `/tmp/linuxscp-<uuid>/` with `0700` permissions).
2. **Event Debouncing:** Many text editors write multiple times, use atomic write-and-rename, or touch backup files. Use the `notify` crate and debounce events by verifying file stability before triggering upload.
3. **Conflict Detection:** Always re-check the remote file's modification time and size before uploading back.
4. **Conflict Options:** If the remote file changed during local editing, prompt:
   - *Overwrite remote version*
   - *Keep remote version (discard local edits)*
   - *Download newer remote version*
   - *Save local copy under a new name*
5. **Session Cleanup:** Clean up temporary files when the editing session is closed or when the application exits safely.

---

## 5. External SSH Terminal Integration

### Terminal Discovery & Launch
LinuxSCP launches an external terminal emulator running an interactive SSH session:
1. Identify available terminal emulator:
   - Check user's preferred terminal setting.
   - Fall back to standard Linux desktop terminals: `x-terminal-emulator`, `gnome-terminal`, `ptyxis`, `konsole`, `alacritty`, `kitty`, `xfce4-terminal`.
2. Construct OpenSSH command:
   ```bash
   ssh -p <port> -i <identity_file> <user>@<host>
   ```
3. **Security:** Never pass passwords as command-line arguments. Rely on SSH key authentication, SSH agent (`SSH_AUTH_SOCK`), or the terminal's native password prompt.
4. Note that this terminal shell is an independent session from any elevated SFTP channel.

---

## 6. Common Pitfalls to Avoid

- **Shell Interpolation:** Formatting `exec` strings with `format!("{} {}", exec, path)` allows malicious filenames to execute arbitrary shell commands.
- **Multiple Watcher Triggers:** Uploading on the first write event without debouncing can upload half-written files.
- **Ignoring $XDG_RUNTIME_DIR:** Creating temp files in insecure world-readable locations without restricting Unix permissions to `0700`.

---

## 7. Testing & Verification

- Test desktop entry parser against sample `.desktop` files from `/usr/share/applications/`.
- Test remote edit lifecycle with simulated local saves and file watcher events.
- Test external terminal launch invocation with various desktop terminal emulators.
- Reference documentation: `docs/LinuxSCP_Product_and_Technical_Specification.md` Sections 13 and 14.

