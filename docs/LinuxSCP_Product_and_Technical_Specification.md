# LinuxSCP — Product and Technical Specification

**Document type:** Product requirements and implementation blueprint  
**Status:** Draft for implementation planning  
**Product name:** LinuxSCP (working name)  
**Target platform:** Linux desktop  
**Primary design reference:** WinSCP Commander interface and workflow  
**Intended development environment:** Antigravity IDE with Gemini Flash 3.8

---

## 1. Product overview

LinuxSCP is a Linux desktop file-transfer client inspired by WinSCP. Its central experience is a two-pane file manager: one pane displays the local filesystem and the other displays a remote server filesystem. Users navigate both sides, transfer files between them, manage remote files and permissions, open remote files in locally installed applications, and launch an authenticated SSH terminal.

The goal is to reproduce the most useful parts of WinSCP's workflow without attempting to clone every WinSCP feature or port its entire codebase.

### 1.1 Core workflow

1. Select or create a saved connection.
2. Connect to a remote server.
3. Browse local and remote directories side by side.
4. Upload and download files through drag-and-drop or explicit commands.
5. Create, rename, delete and manage files and directories.
6. Change remote permissions and ownership when the server permits it.
7. Open local or remote files with locally installed desktop applications.
8. Launch an SSH terminal already authenticated to the selected server.

### 1.2 Product principles

- **WinSCP-style workflow:** familiar Commander-style dual-pane file management.
- **Linux integration:** use Linux desktop conventions and installed application discovery.
- **Protocol-aware behavior:** do not expose operations that a protocol or server cannot perform.
- **Safe by default:** verify SSH host keys, protect credentials and make elevated sessions obvious.
- **Separation of concerns:** keep UI, application services and protocol implementations independent.
- **Incremental development:** deliver a useful SFTP client before adding FTP and SCP.

---

## 2. Scope and priorities

### 2.1 Core features

| Feature | Priority | Notes |
|---|---:|---|
| WinSCP-inspired dual-pane file manager | P0 | Local and remote panes |
| Local filesystem browsing | P0 | Navigation, metadata and file operations |
| SFTP connections | P0 | First protocol to implement |
| Saved connection profiles | P0 | Persist connection settings |
| Remote directory browsing | P0 | Include hidden files and metadata |
| Upload and download | P0 | Files and recursive directories |
| Drag-and-drop between the two panes | P0 | Local-to-remote and remote-to-local only |
| Transfer queue and progress | P0 | Background operations, cancellation and errors |
| Directory navigation toolbar | P0 | Back, forward, parent, root, home and refresh |
| Per-connection last-directory restoration | P0 | Restore local and remote paths |
| Create files and directories | P0 | Local and remote |
| Rename and delete | P0 | Local and remote |
| Remote permissions and ownership | P1 | Subject to protocol/server privileges |
| SFTP server launched through sudo | P1 | Optional elevated SFTP mode |
| Open With application integration | P1 | Discover installed Linux applications |
| Remember default application by extension | P1 | LinuxSCP-specific association |
| Remote-file temporary editing workflow | P1 | Detect changes and upload back |
| SSH terminal launch | P1 | Launch an external terminal by default |
| FTP and FTPS | P2 | Separate protocol backend |
| SCP | P2 | Transfer-focused backend; capability limitations |
| Multiple connection tabs | P2 | Independent session state |
| Bookmarks/favorite directories | P2 | Local and remote paths |
| Packaging and distribution | P1 | AppImage and/or distribution packages |

### 2.2 Explicit non-goals for the initial versions

- Dragging files from LinuxSCP onto the desktop or another application.
- Accepting files dragged into LinuxSCP from external desktop applications.
- Implementing a complete terminal emulator inside LinuxSCP.
- Reproducing every WinSCP feature.
- Implementing synchronization, scripting, cloud storage or advanced automation before the core file manager is stable.
- Silently bypassing remote permission restrictions.
- Automatically accepting unknown or changed SSH host keys.

The only required drag-and-drop operation is between LinuxSCP's own local and remote panes.

---

## 3. Recommended technology stack

The following is the recommended baseline, not a user-confirmed framework decision. It should be validated with a small technical prototype before the full application is built.

| Layer | Recommended technology | Responsibility |
|---|---|---|
| Desktop framework | Tauri 2 | Linux desktop shell and frontend/backend bridge |
| Frontend | React + TypeScript | UI and interaction |
| Styling | Tailwind CSS or a consistent component system | Layout and visual styling |
| Backend | Rust | Filesystem, connections, protocols and transfers |
| Async runtime | Tokio | Concurrent connections and transfer tasks |
| Database | SQLite | Profiles, preferences, directory state and app associations |
| Database access | SQLx or another maintained Rust SQLite layer | Persistence |
| SSH/SFTP | A maintained Rust SSH/SFTP implementation | SSH transport and SFTP |
| FTP/FTPS | A maintained Rust FTP implementation | FTP transport |
| Local file monitoring | Rust filesystem watcher | Detect edits to temporary files |
| Linux application integration | XDG desktop entries and GIO where practical | Discover and launch installed applications |
| SSH terminal | System OpenSSH client in user's preferred terminal | Interactive remote shell |

### 3.1 Technology-selection requirements

Before committing to an SSH/SFTP crate, prototype and verify:

- SSH password authentication.
- SSH public-key authentication.
- SSH agent support, if available.
- Known-host verification and changed-host-key handling.
- SFTP directory listing and file metadata.
- File upload/download streaming.
- Rename, delete, mkdir and chmod.
- Numeric UID/GID ownership operations where supported.
- Opening an SSH session channel and executing a remote SFTP server process for sudo mode.
- Cancellation and reliable cleanup of interrupted operations.

Do not select a library solely because it supports a basic SFTP example. Sudo-enabled SFTP and channel management are important compatibility tests.

### 3.2 Alternative worth evaluating

A native Qt 6/C++ implementation may offer stronger native-widget integration. Existing projects such as FreeSCP should also be studied before implementation. Reuse or forking must be evaluated against licensing, maintenance, architecture and project goals.

---

## 4. Main application UI

### 4.1 Overall layout

The default interface should follow WinSCP's Commander layout:

1. Application menu.
2. Connection/session toolbar.
3. File-operation toolbar.
4. Local and remote panes displayed side by side.
5. Transfer queue/status panel at the bottom.
6. Status bar showing connection and operation information.

The two panes should have comparable functionality and independent navigation state.

### 4.2 Proposed layout

```text
+-----------------------------------------------------------------------+
| LinuxSCP     File  Commands  Options  Help                            |
+-----------------------------------------------------------------------+
| [New] [Open] [Disconnect] [Back] [Forward] [Parent] [Root] [Home]     |
| [Refresh] [New file] [New folder] [Properties] [Transfer]             |
+--------------------------------+--------------------------------------+
| LOCAL                          | REMOTE                               |
| /home/user/Projects            | /var/www/html                        |
+--------------------------------+--------------------------------------+
| Name       Size  Changed ...   | Name       Size  Changed ...         |
| ..                             | ..                                   |
| Documents                      | assets                               |
| Downloads                      | config                               |
| project.zip                    | index.php                            |
| notes.txt                      | .htaccess                            |
|                                |                                      |
+--------------------------------+--------------------------------------+
| Transfer queue                                                        |
| File                 Direction   Progress       Speed       Status    |
| website.zip          Upload      [======----]   4.2 MB/s    Active    |
+-----------------------------------------------------------------------+
| Connected: production-server (SFTP)             Elevated: No          |
+-----------------------------------------------------------------------+
```

The mock-up is conceptual. The real interface must support column resizing, sorting, selection, scrolling and configurable panel proportions.

### 4.3 Pane behavior

Each pane must support:

- Independent directory navigation.
- Independent Back and Forward history.
- Parent-directory navigation.
- Direct navigation to `/` on the remote side.
- Home-directory navigation.
- Refresh.
- Editable path/address bar.
- File and directory selection.
- Single-click selection and double-click activation.
- Ctrl-click multi-selection.
- Shift-click range selection.
- Sorting by visible columns.
- Hidden-file visibility, including dotfiles.
- Context menu on right-click.
- Keyboard navigation.
- Scroll and selection state that remain stable during refresh where practical.

The active pane should be visually distinguishable. Keyboard shortcuts and commands that operate on the active pane must be consistent.

### 4.4 File panel columns

The default columns, in this order, are:

1. **Name**
2. **Size**
3. **Changed**
4. **Rights**
5. **Owner**

Column requirements:

| Column | Local pane | Remote pane |
|---|---|---|
| Name | Filename, file type icon, directory/link indication | Same |
| Size | Human-readable file size | Human-readable file size |
| Changed | Last modified time | Remote modification time |
| Rights | Local Unix permissions when available | Remote Unix permissions when available |
| Owner | Local owner/group when available | Remote owner/group when resolvable |

Additional requirements:

- All columns must be sortable where meaningful.
- Column widths must be resizable.
- Column visibility may be configurable in a later version.
- Long names and owner values must be truncated visually without losing the underlying value.
- File sizes should use a consistent unit policy (B, KB, MB, GB, etc.).
- Directories should have a consistent size display, such as `—`, unless recursive size calculation is explicitly requested.
- Symlinks should be identified and their target should be available in properties or tooltips.
- Unknown metadata must be represented as unavailable, not fabricated.

The backend must preserve structured metadata such as numeric mode bits, UID, GID and timestamps. The frontend should format that metadata for display.

---

## 5. Navigation and directory state

### 5.1 Toolbar actions

| Action | Required behavior |
|---|---|
| Back | Navigate to the previous path in that pane's history |
| Forward | Navigate to the next path in that pane's history |
| Parent | Navigate to the parent directory |
| Root | Navigate to `/` on the remote filesystem; local root may also be supported |
| Home | Navigate to the relevant user's home directory |
| Refresh | Reload the current directory listing |
| New file | Create a new empty file in the current directory |
| New folder | Create a directory in the current directory |

Back and Forward must be independent for local and remote panes.

### 5.2 Address/path bar

Each pane should have an editable path bar.

Requirements:

- Display the current absolute path.
- Allow users to type or paste a path.
- Pressing Enter navigates to that path.
- Handle invalid paths with a clear error.
- Do not silently change to a different path if navigation fails.
- Support paths containing spaces and non-ASCII characters.

### 5.3 Per-connection directory persistence

Persist at least:

- Last local directory.
- Last remote directory.
- Last active pane.
- Optional Back/Forward history.
- Optional selected item.

On reconnect, restore the last directories if they still exist and are accessible. If the remote directory is unavailable, show a clear message and fall back to a sensible directory, such as the remote home directory.

Directory state belongs to the connection profile/session, not to a global remote-path setting.

---

## 6. Connection manager

### 6.1 Saved connections

Users must be able to:

- Create a connection.
- Edit a saved connection.
- Duplicate a connection.
- Delete a connection.
- Connect and disconnect.
- Organize connections into folders/groups in a later version.
- Set a display name independent of hostname.
- Store preferred local and remote starting directories.

### 6.2 Connection settings

The data model should accommodate these settings even if the initial dialog exposes only a subset.

**General**
- Display name.
- Protocol: SFTP, FTP, FTPS or SCP.
- Hostname/IP.
- Port.
- Username.
- Initial remote directory.
- Initial local directory.

**Authentication**
- Password.
- SSH private key path.
- Private-key passphrase.
- SSH agent, if supported.
- Authentication preference/order.

**SSH/SFTP**
- Host-key verification policy.
- Known-hosts source.
- Connection timeout.
- Keepalive interval.
- Compression, if supported.
- Remote SFTP server command for custom/elevated SFTP.

**Proxy/network**
- SOCKS proxy.
- HTTP proxy where supported.
- SSH jump host/ProxyJump where supported.
- Proxy authentication.

**FTP/FTPS**
- Active/passive mode.
- Explicit or implicit TLS where supported.
- Certificate verification.
- Transfer mode and encoding options as appropriate.

**Transfer**
- Default overwrite behavior.
- Transfer concurrency.
- Preserve timestamps where supported.
- Preserve permissions where supported.
- Temporary-file/atomic-rename behavior where supported.

### 6.3 SSH host-key security

- Verify host keys against known-hosts data.
- On first connection, show the presented fingerprint and require an explicit user decision.
- On changed host key, show a prominent warning and do not silently continue.
- Provide a way to inspect and manage saved host keys.
- Never use an automatic “accept all keys” policy as the default.

### 6.4 Credential storage

- Store non-secret connection metadata in SQLite.
- Store passwords and private-key passphrases in the Linux Secret Service/keyring where available.
- Do not store credentials in plaintext SQLite or application logs.
- Handle missing/unavailable keyring services explicitly.
- Do not log authentication payloads or secret values.

### 6.5 OpenSSH configuration

Importing settings from `~/.ssh/config` is a useful later enhancement. Do not make it a prerequisite for the first SFTP milestone.

---

## 7. Remote filesystem and protocol support

### 7.1 Protocol implementation order

1. SFTP — complete first.
2. FTP/FTPS — add after the core architecture is stable.
3. SCP — add as a transfer-focused backend with explicitly documented limitations.

SFTP is the first implementation target because it provides remote filesystem operations such as listing, metadata, upload, download, rename, delete, directory creation and permission changes.

SCP is not equivalent to SFTP. Its filesystem browsing and management capabilities may require remote shell commands and may vary by server. The UI must not promise unsupported SCP operations.

### 7.2 Common remote filesystem abstraction

All protocol backends should implement a common application-level contract, while exposing protocol capabilities explicitly.

Conceptual interface:

```rust
trait RemoteFileSystem {
    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;

    async fn list_directory(&self, path: &str)
        -> Result<Vec<RemoteFile>>;

    async fn stat(&self, path: &str)
        -> Result<RemoteMetadata>;

    async fn upload(&self, source: &Path, destination: &str)
        -> Result<TransferHandle>;

    async fn download(&self, source: &str, destination: &Path)
        -> Result<TransferHandle>;

    async fn create_file(&self, path: &str)
        -> Result<()>;

    async fn create_directory(&self, path: &str)
        -> Result<()>;

    async fn rename(&self, old_path: &str, new_path: &str)
        -> Result<()>;

    async fn delete_file(&self, path: &str)
        -> Result<()>;

    async fn delete_directory(&self, path: &str)
        -> Result<()>;

    async fn set_permissions(&self, path: &str, mode: u32)
        -> Result<()>;

    async fn set_owner_group(
        &self, path: &str, uid: Option<u32>, gid: Option<u32>
    ) -> Result<()>;
}
```

This is illustrative, not a final compilable interface. Streaming, cancellation, recursive operations, progress events and protocol-specific errors must be designed before implementation.

### 7.3 Capability model

The application must query or know the capabilities of the active backend/session.

Potential capabilities:

- List directory.
- Read metadata.
- Upload.
- Download.
- Create file.
- Create directory.
- Rename.
- Delete file.
- Delete directory.
- Change permissions.
- Change owner/group.
- Preserve timestamps.
- Recursive transfer.
- Symbolic-link operations.

Disable unavailable UI actions or explain why they cannot be used. Do not silently route an unsupported operation through an unsafe shell command.

### 7.4 FTP and FTPS

FTP support should use a dedicated backend. It must account for:

- Active and passive modes.
- Binary and ASCII transfer modes where relevant.
- Explicit/implicit TLS where supported.
- Certificate validation.
- FTP server-specific permission and metadata limitations.

The SSH sudo mechanism does not apply to FTP. FTP access rights are controlled by the FTP server and account configuration.

### 7.5 SCP

SCP support should be scoped carefully. At minimum, support upload/download where practical. Directory browsing and file management may rely on remote commands or server-specific behavior. Clearly identify unsupported features rather than presenting SCP as a full SFTP equivalent.

---

## 8. Sudo-enabled SFTP and elevated sessions

This is a key requirement. Users need to manage files in protected directories, including root-owned system files, where the remote server administrator has configured appropriate privileges.

### 8.1 Preferred model: normal SSH login, elevated SFTP server

The preferred workflow is:

1. Authenticate to SSH as a normal account, such as `deploy`.
2. Open a separate SSH session channel.
3. Execute the configured SFTP server process through sudo.
4. Connect the SFTP protocol stream to that process.
5. Perform remote filesystem operations through the elevated SFTP session.

Example remote command (path varies by distribution):

```bash
sudo -n /usr/lib/openssh/sftp-server
```

The exact command must be configurable per connection. Some systems use a different binary path.

The SFTP server's stdout must remain a clean SFTP protocol stream. Diagnostic output must not corrupt that stream.

### 8.2 Server-side sudo configuration

An administrator may configure a narrowly scoped sudoers rule. Example only; the executable path and account must be verified on the target server:

```sudoers
deploy ALL=(root) NOPASSWD: /usr/lib/openssh/sftp-server
```

Do not recommend unrestricted passwordless sudo as the default:

```sudoers
# Do not recommend this broad permission:
deploy ALL=(ALL) NOPASSWD: ALL
```

Use non-interactive sudo for the initial implementation so an unexpected password prompt cannot hang the SFTP protocol channel. If sudo is not configured correctly, return an actionable error explaining what the administrator needs to configure.

### 8.3 Compatibility limitations

- Some servers use `internal-sftp`, which is not an external executable that can simply be launched through sudo.
- External SFTP server paths vary across Linux distributions.
- The SSH library must support opening a session channel and executing a process whose stdin/stdout carry the SFTP protocol.
- The application must test this behavior with real OpenSSH servers before claiming support.
- Do not assume that elevating an SSH terminal also elevates an existing SFTP session. These are separate sessions/channels and processes.

### 8.4 Elevated-session UI and safety

- Show a persistent, prominent `ELEVATED` or `ROOT` indicator.
- Confirm before destructive recursive operations in elevated mode.
- Show the SSH login identity and effective SFTP identity when they can be determined.
- Make it clear that the SSH account and effective filesystem identity may differ.
- Do not automatically enable elevation for every connection.
- Never hide privilege errors or imply that all operations will succeed regardless of server security policy.

### 8.5 Ownership and permissions

Changing owner/group normally requires elevated privileges. SFTP ownership operations generally use numeric UID/GID values. Human-readable names may need remote resolution, for example through `getent`, if the server permits it.

The UI must distinguish:
- Numeric UID/GID.
- Resolved username/group name.
- Unresolved IDs.
- Permission denied.
- Unsupported protocol operation.

---

## 9. File operations

The following operations should be available where supported.

### 9.1 Create

- Create an empty file.
- Create a directory.
- Validate names.
- Handle existing names without overwriting silently.
- Refresh the pane after success.

### 9.2 Rename

- Rename files and directories.
- Validate destination names.
- Handle destination conflicts.
- Refresh the pane after success.

### 9.3 Delete

- Delete files.
- Delete directories.
- Support recursive directory deletion with an explicit confirmation.
- In elevated mode, make destructive-operation confirmation especially clear.
- Report partial failures in recursive operations.

### 9.4 Copy and transfer

- Local-to-remote upload.
- Remote-to-local download.
- Recursive directory transfer.
- Multi-file transfer.
- Conflict handling.
- Transfer cancellation.
- Transfer retry.
- Preserve timestamps and permissions where supported and requested.

Remote-to-remote copy may be added later. It is not required for the first release.

---

## 10. Drag-and-drop between panes

The required drag-and-drop scope is strictly internal to LinuxSCP.

| Source | Destination | Operation |
|---|---|---|
| Local pane | Remote pane | Upload |
| Remote pane | Local pane | Download |
| Local pane | Local pane | Optional local copy |
| Remote pane | Remote pane | Optional remote copy, if supported |

The first two are mandatory. Same-side copy operations are optional and can be deferred.

### 10.1 Interaction requirements

- Drag one or multiple selected items.
- Drag directories for recursive transfer.
- Highlight a valid target pane during dragging.
- Drop into the current directory or a directory row where supported.
- Show a transfer confirmation when the operation is potentially destructive or ambiguous.
- Add accepted operations to the central transfer queue.
- Never perform the file transfer in the React UI itself.

### 10.2 Implementation

React manages drag state, selected items and drop targets. The Rust backend performs filesystem and protocol operations. The UI sends a structured transfer request to the backend and subscribes to progress events.

External desktop drag-and-drop is not required.

---

## 11. Transfer manager

A persistent bottom panel should show active, queued, completed and failed transfers.

### 11.1 Per-transfer information

- Filename/path.
- Direction: upload or download.
- Source and destination.
- Progress percentage.
- Bytes transferred and total bytes, when known.
- Current speed.
- Estimated remaining time, when calculable.
- Status.
- Error details.

### 11.2 Required controls

- Cancel transfer.
- Retry failed transfer.
- Clear completed transfers.
- View transfer details.
- Pause/resume only when genuinely supported by the backend and protocol.
- Configure a reasonable transfer concurrency limit.

Do not show pause/resume controls if the implementation cannot safely pause and resume the operation.

### 11.3 Progress reporting

- Stream large files in chunks; do not load entire files into memory.
- Emit progress events from Rust to the frontend.
- Track progress per file and overall progress for recursive directory transfers.
- Handle unknown total size without displaying a false percentage.
- Distinguish queued, connecting, transferring, verifying, completed, cancelled and failed states.
- Keep the UI responsive during concurrent transfers.

### 11.4 Transfer safety

Where supported:

1. Transfer to a temporary destination filename.
2. Verify completion.
3. Rename the temporary file to the final destination.
4. Report failure and clean up temporary artifacts where safe.

This reduces the chance that a partially transferred file appears to be complete. Atomic rename behavior is server/filesystem dependent and must not be promised universally.

---

## 12. File permissions and ownership

### 12.1 Properties dialog

Right-click a local or remote item and choose Properties.

For remote files, provide a permissions/ownership editor when the active protocol supports it.

### 12.2 Unix permissions

Display and edit:

| Subject | Read | Write | Execute |
|---|---|---|---|
| Owner | Checkbox | Checkbox | Checkbox |
| Group | Checkbox | Checkbox | Checkbox |
| Others | Checkbox | Checkbox | Checkbox |

Also provide an octal field, for example `644` or `755`.

Requirements:

- Keep checkbox and octal representations synchronized.
- Validate octal input.
- Support special bits: SUID, SGID and sticky bit, where supported.
- Display the symbolic representation, such as `-rw-r--r--`.
- Apply changes to one or multiple selected items.
- Offer recursive application for directories, with explicit confirmation.
- Report permission failures accurately.

### 12.3 Ownership

Provide fields for:

- Owner username or numeric UID.
- Group name or numeric GID.

Requirements:

- Resolve names to numeric IDs where possible.
- Permit numeric IDs when name resolution is unavailable and the protocol supports them.
- Display unresolved IDs without inventing names.
- Explain when the current session lacks permission to change ownership.
- Apply to selected files where supported.

### 12.4 Local permissions

Local file permissions and ownership can also be displayed. Editing local ownership may require local OS privileges and should not silently invoke privilege escalation. Local permissions can be changed using normal filesystem APIs subject to the current user's rights.

---

## 13. Open With and Linux desktop application integration

Users must be able to right-click either a local or remote file and choose **Open With**.

LinuxSCP should discover installed desktop applications, let the user choose one, and remember a default application for each file extension.

### 13.1 Context-menu behavior

A proposed context menu:

```text
Open
Open With >
    Use remembered default
    [Previously used applications]
    Choose another application...
Download
Edit / Properties
Rename
Delete
```

The exact menu can be refined, but the required behavior is:

- User can choose Open With.
- User can manually select an installed application.
- User can set that application as the default for the file extension.
- LinuxSCP remembers its own association.
- The user can choose another application later.

### 13.2 Application discovery

Use Linux desktop application infrastructure rather than maintaining a hardcoded list.

Consider:

- XDG application directories.
- User-level desktop entries.
- System desktop entries.
- Flatpak-exported desktop entries.
- GIO/GDesktopAppInfo or an equivalent maintained integration.

Relevant locations commonly include:

- `/usr/share/applications/`
- `~/.local/share/applications/`
- XDG data directories and their `applications` subdirectories.
- Flatpak export directories, as exposed through the user's XDG environment.

Do not assume these are the only possible locations.

Display application name, icon and description where available. Provide search/filtering in the application picker.

Launching an application must respect its desktop-entry `Exec` field semantics and file-argument placeholders. Do not concatenate an executable and a filename into a shell command. Avoid shell interpolation and command injection.

An advanced manual executable option may be considered, but installed desktop applications are the primary selection mechanism.

### 13.3 Remembered application associations

Store LinuxSCP-specific associations in SQLite rather than changing system-wide MIME defaults.

Example conceptual table:

```sql
CREATE TABLE file_app_associations (
    extension TEXT PRIMARY KEY,
    desktop_app_id TEXT NOT NULL,
    created_at TEXT NOT NULL
);
```

The association should use a stable desktop application identifier where possible. Avoid depending only on an executable path, because desktop applications may be packaged or launched through wrappers.

Requirements:

- Normalize extensions consistently (for example, lowercase and leading dot).
- Handle filenames with multiple dots.
- Handle files without extensions separately.
- If the saved application is no longer installed, show a useful fallback and allow reassignment.
- Allow the user to choose a different application even when a default exists.
- Make the remembered default behavior clear in the UI.

### 13.4 Local file opening

For a local file:

1. User selects Open or Open With.
2. LinuxSCP resolves the remembered association or displays the application picker.
3. Launch the application with the local file path.
4. Do not copy the local file unnecessarily.

### 13.5 Remote file opening and editing

A remote file cannot generally be passed directly to a local desktop application as a normal local path. Use a temporary local working copy.

Workflow:

1. User selects a remote file and chooses Open With.
2. LinuxSCP downloads it into a managed temporary working directory.
3. LinuxSCP launches the selected local application with the temporary path.
4. LinuxSCP monitors the temporary file for modifications.
5. When a modification is detected, LinuxSCP offers or performs the configured upload-back behavior.
6. Before upload, check whether the remote file changed since the download.
7. If a conflict is detected, do not silently overwrite the remote file.
8. Clean up temporary files only when it is safe to do so.

The application should support multiple remote editing sessions at once.

### 13.6 Remote edit conflict detection

Record available remote metadata when downloading, such as:

- Size.
- Modification timestamp.
- Other stable metadata available from the protocol.

Before uploading a modified temporary file, compare the current remote metadata with the recorded baseline. A remote hash may be used when practical, but should not be assumed available through every protocol.

If the remote file has changed, offer choices such as:
- Keep remote version.
- Overwrite remote version.
- Download the newer remote version.
- Save the local temporary copy under another name for manual comparison.

Filesystem watchers can report multiple events for a single save. Debounce events and avoid uploading partially written files. Consider detecting stable file size/mtime before upload.

---

## 14. SSH terminal integration

The initial implementation should launch the user's existing Linux terminal rather than embedding a terminal emulator.

### 14.1 Required behavior

- User selects a connection and chooses Open Terminal.
- LinuxSCP launches an available terminal application.
- The terminal starts an SSH session using the selected connection's host, port and username.
- Authentication should use the configured key, agent or other supported OpenSSH configuration.
- The shell should be ready for commands after login.
- The terminal should use the connection's saved SSH settings where practical.

### 14.2 Implementation considerations

- Prefer the system OpenSSH client for terminal sessions.
- Do not expose passwords in process arguments or logs.
- Do not assume every Linux desktop uses the same terminal emulator.
- Provide a configurable terminal command or discover available terminal applications.
- Prefer SSH agent or key-based authentication for seamless login.
- Handle host-key verification consistently with the connection's security policy.

An embedded terminal can be considered later. It is not a requirement for the initial release.

### 14.3 Elevated terminal

The SSH terminal and elevated SFTP session are separate concerns. A normal SSH terminal should not be described as root merely because the SFTP server runs through sudo. If an elevated shell is added later, it must be an explicit user action and must follow the server's sudo policy.

---

## 15. Data model and persistence

SQLite should store application state and non-secret connection information. Secrets belong in the OS keyring.

A preliminary schema:

### 15.1 Connections

```sql
CREATE TABLE connections (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    protocol TEXT NOT NULL,
    host TEXT NOT NULL,
    port INTEGER NOT NULL,
    username TEXT NOT NULL,
    auth_method TEXT,
    private_key_path TEXT,
    initial_local_directory TEXT,
    initial_remote_directory TEXT,
    sudo_sftp_enabled INTEGER NOT NULL DEFAULT 0,
    sudo_sftp_command TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

Do not store passwords or private-key passphrases in this table.

### 15.2 Connection state

```sql
CREATE TABLE connection_state (
    connection_id TEXT PRIMARY KEY,
    last_local_directory TEXT,
    last_remote_directory TEXT,
    last_active_pane TEXT,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(connection_id) REFERENCES connections(id)
);
```

Optional directory-history tables can be added if history persistence is required.

### 15.3 File application associations

```sql
CREATE TABLE file_app_associations (
    extension TEXT PRIMARY KEY,
    desktop_app_id TEXT NOT NULL,
    created_at TEXT NOT NULL
);
```

### 15.4 Preferences

Store user preferences such as:

- Theme.
- Panel widths.
- Visible columns.
- Default transfer behavior.
- Transfer concurrency.
- Hidden-file visibility.
- Terminal application preference.
- Temporary-file retention policy.

Schema migrations must be versioned. Do not let UI components access SQLite directly.

---

## 16. Backend/frontend architecture

The frontend must not communicate directly with SSH, SFTP, FTP or filesystem libraries. All operations should go through application services exposed by the Tauri bridge.

Conceptual architecture:

```text
React + TypeScript UI
    |
    | Tauri commands and events
    v
Application services (Rust)
    |
    +-- Connection service
    |
    +-- Navigation/session service
    |
    +-- Local filesystem service
    |
    +-- Remote filesystem service
    |       |
    |       +-- SFTP backend
    |       +-- FTP/FTPS backend
    |       +-- SCP backend
    |
    +-- Elevated SFTP launcher
    |
    +-- Transfer manager
    |
    +-- Permissions/metadata service
    |
    +-- Remote editor service
    |
    +-- Desktop application integration
    |
    +-- Storage service
            |
            +-- SQLite
            +-- Linux Secret Service/keyring
```

### 16.1 Suggested project structure

```text
linuxscp/
├── src/
│   ├── app/
│   ├── components/
│   │   ├── FilePanel/
│   │   ├── NavigationToolbar/
│   │   ├── ConnectionDialog/
│   │   ├── ConnectionManager/
│   │   ├── ContextMenu/
│   │   ├── PropertiesDialog/
│   │   ├── OpenWithDialog/
│   │   └── TransferQueue/
│   ├── hooks/
│   ├── stores/
│   ├── services/
│   └── types/
│
├── src-tauri/
│   └── src/
│       ├── main.rs
│       ├── commands/
│       ├── connections/
│       ├── navigation/
│       ├── filesystem/
│       │   ├── local/
│       │   └── remote/
│       ├── protocols/
│       │   ├── sftp/
│       │   ├── ftp/
│       │   └── scp/
│       ├── elevation/
│       ├── transfers/
│       ├── metadata/
│       ├── remote_editor/
│       ├── desktop_integration/
│       ├── terminal/
│       └── storage/
│
├── migrations/
├── tests/
├── docs/
│   ├── PROJECT_SPEC.md
│   ├── ARCHITECTURE.md
│   ├── UI_SPEC.md
│   ├── PROTOCOL_SPEC.md
│   ├── DATABASE_SCHEMA.md
│   └── IMPLEMENTATION_ROADMAP.md
│
└── README.md
```

The structure may evolve, but the separation of responsibilities should remain.

### 16.2 Frontend/backend communication

Use typed request/response contracts for commands and typed events for long-running operations.

Examples of command categories:

- `connect`
- `disconnect`
- `list_local_directory`
- `list_remote_directory`
- `navigate_remote`
- `create_remote_file`
- `create_remote_directory`
- `rename_remote_item`
- `delete_remote_item`
- `get_remote_properties`
- `set_remote_permissions`
- `set_remote_owner_group`
- `start_upload`
- `start_download`
- `cancel_transfer`
- `launch_terminal`
- `discover_desktop_applications`
- `open_local_file`
- `open_remote_file`

Examples of backend events:

- Connection state changed.
- Directory listing completed.
- Transfer progress updated.
- Transfer completed.
- Transfer failed.
- Remote edit changed.
- Remote edit conflict detected.

These names are conceptual. Define and document the final API before implementing dependent UI components.

### 16.3 Concurrency and session ownership

- Each connection session owns its protocol client and remote working state.
- Multiple connection tabs must not share mutable protocol state accidentally.
- Long-running transfers must not block the UI thread.
- Transfers should hold the resources they need for their lifetime.
- Disconnect behavior must be defined for active transfers.
- Cancelled tasks must release channels, temporary files and other resources.
- Backend events should identify the connection and transfer they belong to.

---

## 17. Error handling and reliability

Errors must be actionable and must not be swallowed.

Examples:

- Host key unknown or changed.
- Authentication failed.
- Connection timeout.
- Remote directory does not exist.
- Permission denied.
- Sudo is unavailable or not permitted.
- SFTP server command not found.
- Unsupported server operation.
- Destination already exists.
- Disk full.
- Network connection interrupted.
- Transfer cancelled.
- Remote file changed during editing.
- Selected desktop application is no longer installed.

For recursive operations, report partial completion and identify failed items. Do not report an entire directory transfer as successful if some files failed.

Refresh directory contents after successful create, rename, delete, permission or ownership operations.

---

## 18. Security requirements

Security is a core product requirement.

1. Never silently accept unknown or changed SSH host keys.
2. Never store passwords or passphrases in plaintext.
3. Never interpolate untrusted filenames or paths into shell commands.
4. Do not use shell execution to launch desktop applications with concatenated user-controlled arguments.
5. Keep SFTP protocol stdout free of diagnostic text.
6. Treat remote filenames and metadata as untrusted input.
7. Validate paths and names without incorrectly rejecting valid Unix filenames.
8. Do not log secrets, authentication payloads or private-key contents.
9. Make elevated sessions visually obvious.
10. Confirm destructive recursive operations, especially in elevated sessions.
11. Use least-privilege sudo configuration and explain server-side requirements.
12. Clean up temporary editing files safely, without deleting user data unexpectedly.
13. Detect remote edit conflicts before overwriting where possible.
14. Handle symlinks carefully to avoid unintended traversal or deletion outside the intended scope.

---

## 19. Testing strategy

Testing should begin with the first milestone, not be postponed until the application is feature-complete.

### 19.1 Unit tests

- Path navigation and history.
- File metadata formatting.
- Permission-bit conversion between octal and symbolic forms.
- Extension normalization and application association lookup.
- Transfer state transitions.
- Conflict detection.
- Connection-profile validation.

### 19.2 Integration tests

Use controllable local test environments, including containers or dedicated test VMs, with:

- OpenSSH SFTP.
- OpenSSH external `sftp-server`.
- A normal unprivileged account.
- A sudo-configured account.
- Root-owned files and directories.
- Permission-denied directories.
- FTP/FTPS server for later protocol work.

### 19.3 Required SFTP tests

- Connect with password.
- Connect with SSH key.
- Reject an unknown host key until explicitly accepted.
- Detect a changed host key.
- Browse directories and hidden files.
- Upload and download files.
- Upload and download directories recursively.
- Create, rename and delete files/directories.
- Change permissions.
- Change owner/group when privileged.
- Cancel a transfer.
- Recover from network interruption.
- Use sudo-enabled external SFTP server.
- Report unsupported `internal-sftp` elevation configuration accurately.

### 19.4 UI tests

- Independent pane navigation.
- Back/Forward history.
- Root and Home navigation.
- Drag-and-drop local-to-remote.
- Drag-and-drop remote-to-local.
- Multi-file selection.
- Context-menu operations.
- Transfer queue progress.
- Application picker and remembered extension association.
- Remote edit conflict handling.

---

## 20. Development roadmap

Build one milestone at a time. Each milestone must have a working application state and acceptance criteria.

### M0 — Technical feasibility prototypes

Before building the full UI, validate the riskiest dependencies:

- Tauri 2 desktop shell on the target Linux environment.
- Rust SSH/SFTP library capabilities.
- Known-host verification.
- External SFTP server launched through sudo over an SSH channel.
- XDG/GIO application discovery and launch.
- Filesystem watcher behavior for temporary remote-edit files.

**Exit criteria:** The selected stack and libraries can support the product's critical requirements. Record limitations and decisions.

### M1 — Application shell and local pane

- Initialize Tauri, React, TypeScript and Rust.
- Build the Commander-style window layout.
- Implement local filesystem browsing.
- Implement the five-column file list.
- Implement selection and sorting.
- Implement navigation toolbar and path bar for local paths.
- Implement local create, rename and delete.
- Implement local context menu.

**Exit criteria:** Local file management works reliably without a remote connection.

### M2 — SFTP connection and remote pane

- Implement saved connection profiles.
- Implement SSH authentication and host-key verification.
- Implement SFTP connection lifecycle.
- Implement remote directory listing and metadata.
- Implement remote pane and address bar.
- Implement Back, Forward, Parent, Root, Home and Refresh.
- Persist the last local and remote directories.

**Exit criteria:** Users can reconnect to a saved server and browse remote directories.

### M3 — Transfers

- Implement upload and download.
- Implement recursive directory transfers.
- Implement internal cross-pane drag-and-drop.
- Implement transfer queue.
- Implement progress, speed and status.
- Implement cancellation and error reporting.
- Implement overwrite handling.

**Exit criteria:** Users can reliably transfer files and directories in both directions.

### M4 — Remote file operations and properties

- Create remote files and directories.
- Rename and delete remote items.
- Display remote rights and owner.
- Implement permissions editor.
- Implement ownership editor.
- Support multi-selection operations where appropriate.
- Implement recursive permission changes with confirmation.

**Exit criteria:** Remote file management works within the active account's permissions.

### M5 — Elevated SFTP

- Add per-connection sudo SFTP configuration.
- Launch external SFTP server through a dedicated SSH session channel.
- Validate protocol stream handling.
- Display elevated identity/state.
- Add actionable sudo configuration errors.
- Test root-owned files and protected directories.
- Test permission and ownership changes.

**Exit criteria:** A configured normal SSH account can use an elevated SFTP process without corrupting the SFTP stream or silently bypassing security.

### M6 — Open With and remote editing

- Discover installed desktop applications.
- Build application picker.
- Launch local files.
- Persist extension-to-application associations.
- Download remote files to managed temporary storage.
- Launch remote temporary copies in selected applications.
- Monitor edits and debounce save events.
- Upload changes back.
- Detect remote conflicts.
- Clean up temporary files safely.

**Exit criteria:** Users can open local and remote files in selected desktop applications and safely send remote edits back.

### M7 — SSH terminal

- Discover/configure terminal application.
- Launch OpenSSH using selected connection settings.
- Ensure authentication is ready for interactive shell use.
- Handle terminal launch failures.
- Keep terminal identity distinct from elevated SFTP identity.

**Exit criteria:** A user can launch a terminal already connected to the selected server.

### M8 — FTP/FTPS and SCP

- Add FTP/FTPS backend.
- Add protocol-specific connection settings.
- Implement capability-aware UI.
- Add SCP transfer support within its actual capabilities.
- Test protocol-specific failures and edge cases.

**Exit criteria:** Additional protocols work without destabilizing SFTP or misrepresenting unsupported operations.

### M9 — Polish and distribution

- Multiple connection tabs.
- Bookmarks/favorites.
- Search/filtering.
- Theme and layout preferences.
- Keyboard shortcuts.
- Packaging.
- Documentation.
- Upgrade/migration tests.
- Final security and reliability review.

---

## 21. Antigravity and Gemini development rules

The coding agent must work from the written specification and the architecture documents, not improvise a new architecture for each task.

### 21.1 Project instructions

1. Read the project specification and architecture documents before implementation.
2. Implement one milestone or small, explicitly scoped task at a time.
3. Do not implement future milestones prematurely.
4. Do not change core interfaces without explaining the reason and updating documentation.
5. Keep frontend components independent of protocol implementations.
6. Keep protocol-specific behavior inside backend adapters.
7. Use typed data structures for files, metadata, capabilities, connections and transfer events.
8. Handle errors explicitly.
9. Write tests for backend operations and critical state transitions.
10. Do not use mock data as if it were real server data.
11. Use mock backends deliberately for frontend development and testing.
12. Do not silently weaken host-key verification or credential handling to make a demo work.
13. Do not claim sudo support until it has been tested against a real OpenSSH external SFTP server.
14. Do not claim an operation is supported by every protocol unless the backend capability supports it.
15. Update documentation when architecture or behavior changes.

### 21.2 Recommended agent workflow

For each task:

1. Read relevant specifications.
2. Inspect the existing code before editing.
3. State the implementation plan and files expected to change.
4. Implement only the agreed task.
5. Run formatter, linter, type checks and relevant tests.
6. Test the feature manually where practical.
7. Report changed files, test results, limitations and follow-up work.
8. Update documentation if necessary.

### 21.3 Definition of done

A feature is not complete merely because the UI renders.

It is complete when:

- The user workflow works.
- Backend behavior is implemented.
- Errors are handled.
- Relevant tests pass.
- No existing milestone regresses.
- Documentation is updated.
- Unsupported cases are communicated accurately.

---

## 22. Licensing and project identity

LinuxSCP may reproduce general file-manager workflows and interaction patterns, but should use its own name, icons, visual assets and branding.

WinSCP is GPL-3.0-or-later. If code is copied, adapted or linked in a way that creates a derivative work, review the applicable license obligations. Assets and trademarks may have separate terms. Review the licenses of all reused libraries and reference projects as well.

An independent implementation should not copy WinSCP's source code or artwork unless the project deliberately chooses to comply with the relevant licensing terms.

---

## 23. Open decisions to resolve before implementation

These decisions should be confirmed during M0 rather than guessed by the coding agent:

1. **Desktop framework:** Tauri 2 + Rust + React is the recommended baseline; Qt 6 is the native alternative.
2. **SSH/SFTP library:** select only after verifying host-key handling, authentication, channel execution and sudo SFTP feasibility.
3. **Sudo compatibility:** decide which external SFTP server commands and server configurations will be supported initially.
4. **Remote edit behavior:** choose whether upload-back is automatic after a stable save or asks for confirmation by default. A confirmation-first default is safer.
5. **Transfer concurrency:** choose a conservative default and make it configurable.
6. **Terminal discovery:** decide which terminal launch methods to support across target Linux desktops.
7. **Distribution targets:** decide which Linux distributions and packaging formats are in the first release.
8. **Same-pane copy:** decide whether local-to-local and remote-to-remote copy are included in the first release or deferred.

---

## 24. Initial release acceptance criteria

The first useful release should satisfy all of the following:

- The application opens on Linux and presents a WinSCP-inspired dual-pane interface.
- The local pane browses the local filesystem.
- The user can save an SFTP connection and reconnect later.
- SSH host keys are verified and changed keys are not silently accepted.
- The remote pane browses the server filesystem.
- Each pane supports Back, Forward, Parent, Root, Home and Refresh.
- The last local and remote directories are restored per connection.
- Both panes display Name, Size, Changed, Rights and Owner columns where metadata is available.
- Users can upload and download files and directories.
- Users can drag selected items from the local pane to the remote pane and vice versa.
- Transfers run in the background and show progress.
- Users can create, rename and delete files/directories where permitted.
- Users can inspect and change remote permissions where supported.
- Users can change remote ownership when the server and account permit it.
- A configured sudo-enabled SFTP connection can manage protected files.
- Users can right-click local or remote files and choose Open With.
- LinuxSCP discovers installed desktop applications and remembers defaults by extension.
- Remote files open through a managed temporary local copy, with change detection and conflict protection.
- Users can launch an external SSH terminal already connected to the selected server.
- FTP/FTPS and SCP are either implemented in a later release or clearly identified as not yet supported; they must not be simulated.

---

## 25. Reference projects and documentation

Study these projects and standards during M0. Treat them as references, not automatic dependencies.

- WinSCP: https://github.com/winscp/winscp
- FreeSCP: https://github.com/oegea/FreeSCP
- Cross-SCP: https://github.com/pat229988/Cross-SCP
- Tauri: https://tauri.app/
- OpenSSH: https://www.openssh.com/
- XDG Base Directory Specification: https://specifications.freedesktop.org/basedir-spec/latest/
- Desktop Entry Specification: https://specifications.freedesktop.org/desktop-entry/latest/
- GIO documentation: https://docs.gtk.org/gio/
- SQLite: https://www.sqlite.org/docs.html

---

## Final product definition

**LinuxSCP is a WinSCP-inspired Linux desktop application centered on a Commander-style dual-pane file manager. It provides local-to-remote and remote-to-local file transfers, saved server connections, independent directory navigation, remote file and directory management, Unix permissions and ownership editing, optional sudo-enabled SFTP access, Linux Open With integration, remote-file editing through temporary local copies, and an automatically authenticated external SSH terminal.**

The implementation should prioritize a secure, reliable SFTP experience first, then add FTP/FTPS and SCP through capability-aware protocol adapters.
