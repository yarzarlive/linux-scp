# LinuxSCP — Step-by-Step Antigravity Development Prompts

**Purpose:** A sequential prompt pack for building LinuxSCP in Antigravity IDE with Gemini Flash 3.8.

**How to use:** Run these prompts in order, one at a time, in the same Antigravity project/chat context where possible. Do not paste all prompts at once. Each prompt asks the agent to inspect the existing project, implement one bounded milestone, run checks, and report what changed before proceeding.

**Required project reference:** Put `LinuxSCP_Product_and_Technical_Specification.md` in the project repository (for example, `/docs/`) and keep it available to Antigravity. The specification is the product source of truth. If a prompt conflicts with the specification, identify the conflict and ask before changing scope.

**Important:** The specification recommends Tauri 2 + Rust + React + TypeScript, but treats that stack as a proposal to validate. Prompt 1 therefore requires a technical feasibility check before committing to the architecture.

---

## How to work through this prompt pack

1. Start with Prompt 0.
2. Run each prompt only after the previous milestone's acceptance criteria pass.
3. Review the agent's change summary and test output. Do not accept a claim that a feature works without a build, test, or reproducible manual verification.
4. Commit or otherwise checkpoint the project after each successful milestone.
5. If Antigravity proposes a broad rewrite, stop it and ask for a smaller, incremental change.
6. Keep the specification and this prompt pack in the repository so the agent can refer to them.

### Global engineering rules

These rules apply to every prompt below:

- Read the product/technical specification before making implementation decisions.
- Inspect the existing repository before editing. Preserve working code and established conventions.
- Implement only the current milestone. Do not prematurely implement later features.
- Do not replace real functionality with mock data, hard-coded success responses, placeholder buttons, or fake progress.
- Keep frontend presentation separate from filesystem, protocol, persistence, and transfer logic.
- Use typed interfaces and explicit error handling across the frontend/backend boundary.
- Do not block the UI thread with filesystem, network, or long-running operations.
- Do not log passwords, private keys, passphrases, tokens, or other credentials.
- Never disable SSH host-key verification or automatically trust unknown/changed host keys.
- Never store credentials in plaintext SQLite or configuration files. Use an appropriate Linux Secret Service/keyring integration; if it is not ready, do not pretend credential persistence is secure.
- Do not use unrestricted passwordless sudo. Elevated SFTP must be an explicit, separately configured capability.
- Do not implement external desktop drag-and-drop. Drag-and-drop is strictly between LinuxSCP's own local and remote panes.
- Do not silently swallow errors. Show actionable errors and preserve enough technical detail for diagnostics without exposing secrets.
- Add or update tests for each meaningful behavior.
- Run formatting, linting, type checking, tests, and the relevant build before declaring a milestone complete.
- If a dependency's maintenance status, API, license, or required feature support is uncertain, verify it before adopting it. Do not invent APIs.
- If blocked by a decision that materially affects architecture or security, stop and explain the decision needed rather than guessing.

### Required completion report after every prompt

At the end of each milestone, report:
1. Files created or changed.
2. Functionality implemented.
3. Commands run and their actual results.
4. Tests added and their results.
5. Known limitations or deferred work.
6. Any decision requiring my approval.
7. Exact manual steps I should perform to verify the feature.

---

# Prompt 0 — Repository inspection and implementation plan

```text
You are the senior engineer responsible for implementing LinuxSCP.

First, read the complete LinuxSCP_Product_and_Technical_Specification.md. Treat it as the product source of truth. Then inspect the entire current repository, including manifests, source files, scripts, tests, and existing documentation.

Do not modify files yet.

Tasks:
1. Summarize the product architecture and the P0/P1/P2 scope from the specification.
2. Identify the repository's current state: empty, scaffolded, or partially implemented.
3. Identify the actual installed development environment and available build tools from the repository and terminal. Do not assume dependencies are installed.
4. Evaluate whether the proposed Tauri 2 + Rust + React + TypeScript stack is viable in this environment.
5. Identify the most important technical risks, especially:
   - Linux packaging and desktop integration.
   - Rust SSH/SFTP library support and maintenance.
   - SSH host-key verification.
   - Secure credential storage using Linux Secret Service.
   - Launching a sudo-enabled external SFTP server through an SSH session channel.
   - XDG/GIO application discovery and launch.
6. Propose a milestone plan that maps to the specification. Keep milestones small and independently testable.
7. Identify architectural decisions that must be validated with a prototype before they become permanent.

Output a concise but technically specific implementation plan. Do not write application code or install dependencies in this step. Ask me only about decisions that genuinely cannot be resolved from the specification or repository.
```

**Gate:** Approve the architecture and milestone plan before implementation begins.

---

# Prompt 1 — Validate the stack and prove the risky dependencies

```text
Use the approved implementation plan and the LinuxSCP specification.

Before building the application, perform a small technical feasibility spike. Do not build the full UI.

Investigate and, where practical, create minimal isolated prototypes for:
1. Tauri 2 running on the target Linux environment.
2. React + TypeScript frontend calling a typed Rust/Tauri command.
3. The selected Rust SSH library connecting to a test SSH server with strict host-key verification.
4. Listing a remote directory over SFTP and retrieving file metadata.
5. A separate SSH session channel capable of launching an external SFTP server process, as required for the proposed sudo-SFTP design. Do not use an elevated shell as a substitute for an elevated SFTP subsystem.
6. Linux Secret Service/keyring integration feasibility.
7. XDG desktop application discovery and launching a selected application.

For every dependency, verify current documentation, API shape, maintenance activity, license, and whether it supports the required behavior. Do not rely on guessed APIs.

Create a short `docs/TECHNICAL_SPIKE.md` containing:
- Dependency choices and versions.
- Evidence for compatibility.
- Prototype results.
- Unsupported or uncertain capabilities.
- Risks and fallback options.
- Recommended architecture decision.

Do not silently choose a workaround that weakens host-key verification or credential security. If sudo-SFTP cannot be proven, document the limitation and keep it behind a future capability boundary.

Run the prototypes and report actual results. Do not proceed to the full application until the critical choices are understood.
```

**Gate:** Confirm the dependency choices and any explicitly deferred capabilities.

---

# Prompt 2 — Scaffold the desktop application

```text
Implement only the LinuxSCP application scaffold using the approved stack and the results of the technical spike.

Requirements:
1. Create a clean Tauri 2 + Rust + React + TypeScript project structure.
2. Establish a clear frontend/backend boundary.
3. Configure formatting, linting, type checking, and tests.
4. Add a minimal application window with the LinuxSCP name and a basic application shell.
5. Add typed example Tauri commands and typed frontend invocation patterns.
6. Add structured logging that never records credentials or private key contents.
7. Add a developer README with exact setup, development, test, and build commands.
8. Add a basic CI workflow if the repository is configured for CI.
9. Do not add fake connection, transfer, or file-management functionality.

Keep modules small and responsibilities explicit. Do not implement the file manager yet.

Run the frontend checks, Rust checks, tests, and a desktop build. Fix all errors introduced by this milestone.
```

**Gate:** The application launches locally and all configured checks pass.

---

# Prompt 3 — Define domain models and capability-aware architecture

```text
Implement LinuxSCP's foundational domain model and backend architecture. Do not implement network connections yet.

Read the specification and create typed models for:
- Connection profile metadata.
- Protocol type: SFTP, FTP, FTPS, SCP.
- Authentication method and non-secret authentication metadata.
- Local and remote paths.
- File entries and metadata.
- File permissions and ownership metadata.
- Transfer jobs and transfer states.
- Connection/session state.
- Protocol capabilities.
- Structured application errors.

Design a protocol adapter boundary so that the UI and application services do not depend directly on a particular protocol library.

Important:
- Do not assume FTP, FTPS, SCP, and SFTP have identical capabilities.
- Represent unsupported operations explicitly.
- Distinguish unknown metadata from empty or zero values.
- Keep credentials and secret material out of ordinary profile serialization.
- Use stable identifiers for profiles, sessions, and transfer jobs.
- Use typed errors rather than stringly typed success/failure results.

Add unit tests for model invariants, serialization where appropriate, and capability decisions. Document the architecture in `docs/ARCHITECTURE.md`.

Do not implement UI features beyond what is needed to compile and test the models.
```

**Gate:** Domain types compile, tests pass, and protocol capability behavior is documented.

---

# Prompt 4 — Build the Commander-style dual-pane UI shell

```text
Implement the main LinuxSCP desktop interface based on the specification.

Create a functional layout with:
1. Main menu area.
2. Connection/profile toolbar.
3. File-operation toolbar.
4. Local filesystem pane on the left.
5. Remote filesystem pane on the right.
6. Transfer queue/manager area at the bottom.
7. Status bar.

For the initial UI:
- The local pane must be a real, usable local file browser.
- The remote pane should show a clear disconnected state, not fabricated files.
- Use the exact file-column order from the specification: Name, Size, Changed, Rights, Owner.
- Include path bars and navigation controls: Back, Forward, Parent, Root, Home, Refresh.
- Support resizing panes and the bottom transfer area.
- Include multi-selection, sorting, hidden-file visibility, and sensible empty/error/loading states where feasible.
- Make keyboard focus and selection behavior predictable.
- Use Linux desktop visual conventions; do not imitate Windows chrome literally.

Create reusable components rather than one giant component. Keep all filesystem work in the backend.

Do not implement network protocols, drag-and-drop, or transfers in this milestone. Use only genuine local filesystem data and explicit disconnected UI for the remote side.

Add UI tests for important rendering and interaction behavior. Run all checks and build the app.
```

**Gate:** Local pane works; remote pane is honestly disconnected; layout and column order match the specification.

---

# Prompt 5 — Implement local filesystem browsing and operations

```text
Implement the local filesystem service and connect it to the local pane.

Required behavior:
- List directory contents with metadata.
- Navigate into directories.
- Back, Forward, Parent, Root, Home, and Refresh.
- Show and hide dotfiles.
- Sort by each visible column.
- Multi-select files and directories.
- Create a directory.
- Create a new file.
- Rename files and directories.
- Delete selected items with confirmation.
- Handle permission errors, missing paths, invalid names, symlinks, and inaccessible directories.
- Display file size, modification time, permissions, and owner/group where available.

Security and correctness:
- Do not follow symlinks blindly during recursive operations.
- Do not allow path traversal outside the selected target through malformed path input.
- Do not perform destructive actions without confirmation.
- Use asynchronous backend operations.
- Do not block the UI thread.
- Keep local filesystem behavior separate from remote protocol behavior.

Add unit and integration tests, including error cases and symlink behavior where the platform permits. Connect the UI to real backend results; remove any placeholder local data.
```

**Gate:** Local browsing and basic file operations work against the actual filesystem.

---

# Prompt 6 — Implement connection profiles and secure credential handling

```text
Implement saved connection profiles and connection configuration UI.

Profile fields should follow the specification and include only applicable fields for each protocol. At minimum, support:
- Profile name.
- Hostname/IP and port.
- Username.
- Protocol.
- Authentication method.
- Optional identity/private-key file reference.
- Connection and timeout settings supported by the chosen backend.
- Initial remote directory.
- Other connection options only when supported by the selected protocol implementation.

Persistence:
- Store non-secret profile settings in SQLite.
- Store passwords, passphrases, and other secrets in Linux Secret Service/keyring.
- SQLite must contain only a secret reference or keyring lookup identifier, never the secret itself.
- If keyring access is unavailable, show a clear error and do not silently fall back to plaintext storage.
- Support create, edit, duplicate, delete, and select profile.
- Validate required fields and protocol-specific combinations.
- Do not log secret values.

Add migration handling and tests for profile persistence. Add UI tests for profile creation and validation.

Do not connect to servers in this milestone. Do not implement a fake “connected” state.
```

**Gate:** Profiles persist correctly, secrets are kept out of SQLite, and invalid configurations are rejected.

---

# Prompt 7 — Implement SSH host-key verification and SFTP connection lifecycle

```text
Implement the first real remote protocol: SFTP over SSH.

Use the dependency validated in the technical spike. Do not change libraries without explaining why.

Required behavior:
1. Connect using the selected saved profile.
2. Support the authentication methods proven by the spike.
3. Verify SSH host keys against a persistent known-hosts store.
4. For an unknown host key, show its fingerprint and require explicit user approval before saving it.
5. For a changed host key, block the connection by default and explain the mismatch. Never automatically replace the stored key.
6. Support disconnect and reconnect.
7. Track connection lifecycle states accurately.
8. Apply timeouts and cancellation where supported.
9. Keep secrets out of logs and error messages.
10. Surface actionable connection errors.

Add tests for host-key decisions and connection-state transitions. Where a real SSH test server is needed, document a reproducible local test setup.

Do not implement directory browsing in this milestone beyond the minimum needed to prove that the SFTP session is established. The remote pane must not report connected until the SFTP subsystem is actually ready.
```

**Gate:** A real SFTP connection works with strict host-key verification and explicit trust decisions.

---

# Prompt 8 — Implement remote browsing and navigation state

```text
Connect the remote pane to the real SFTP session.

Implement:
- Remote directory listing.
- Name, Size, Changed, Rights, Owner columns in the specified order.
- Back, Forward, Parent, Root `/`, Home, and Refresh.
- Editable remote path bar.
- Hidden-file visibility.
- Sorting and multi-selection.
- Loading, empty, permission-denied, disconnected, and network-error states.
- Per-connection remote navigation history.
- Restore the last remote directory for each saved connection.
- Correct handling of absolute paths, relative paths, spaces, Unicode names, and symbolic links.

Metadata:
- Use SFTP metadata where available.
- Distinguish unavailable owner/group names from numeric UID/GID.
- Do not invent Changed, Rights, or Owner values if the server does not provide them.

Keep the remote pane responsive. Directory listing and metadata resolution must not freeze the UI.

Add tests for path normalization, history behavior, metadata mapping, and failure states. Verify against a real test SFTP server.
```

**Gate:** Local and remote panes both browse real filesystems independently.

---

# Prompt 9 — Implement file transfers and the transfer manager

```text
Implement real local-to-remote uploads and remote-to-local downloads over SFTP.

Required features:
- Explicit Upload and Download commands.
- Transfer selected files and directories recursively.
- Transfer queue with queued, connecting, transferring, completed, failed, cancelled, and paused states if pause is genuinely supported.
- Per-job progress bars.
- Overall progress for directory transfers.
- Bytes transferred, total bytes when known, transfer speed, and estimated remaining time when meaningful.
- Cancellation.
- Retry failed jobs.
- Queue multiple transfers.
- Clear error reporting.
- Avoid loading entire large files into memory.
- Use streaming/chunked I/O.
- Preserve appropriate file timestamps and permissions only when supported and configured.
- Handle name conflicts with explicit user choices; never silently overwrite.
- Write downloads safely, avoiding partially downloaded files being mistaken for completed files.
- Ensure transfer events are associated with the correct job ID.

Use the backend transfer service and typed events. The UI must display real progress, not simulated progress.

Add tests for cancellation, failure, conflict handling, directory recursion, and progress accounting. Test with both small and large files against a real SFTP server.
```

**Gate:** Real uploads/downloads work and progress/cancellation reflect actual transfer state.

---

# Prompt 10 — Implement internal two-pane drag-and-drop

```text
Implement drag-and-drop only between LinuxSCP's own local and remote panes.

Strict product boundary:
- Local pane → remote pane means upload.
- Remote pane → local pane means download.
- No drag source or drop target integration with the external Linux desktop.
- Do not accept files dragged from external file managers or other applications.
- Do not enable dragging files from LinuxSCP onto the desktop or other applications.

Behavior:
- Support multi-selection.
- Show a clear drop target and transfer direction.
- Reject invalid targets, such as dropping onto an unsupported item or the same pane.
- Ask for conflict resolution before overwriting.
- Add accepted operations to the transfer queue.
- Preserve the selected destination directory.
- Provide accessible non-drag alternatives through Upload and Download commands.
- Avoid accidental transfers during ordinary selection and navigation.

Add UI tests for both directions and for rejected drops. Verify that no external desktop drag-and-drop path has been added.
```

**Gate:** Only local↔remote pane transfers are possible through drag-and-drop.

---

# Prompt 11 — Implement remote file operations and capability-aware behavior

```text
Implement remote file and directory operations over SFTP.

Required operations:
- Create directory.
- Create empty file where supported.
- Rename.
- Delete file.
- Delete directory.
- Recursive directory deletion only with explicit confirmation and clear target display.
- Refresh affected directory after successful operations.

Requirements:
- Respect server capabilities and permissions.
- Never claim an operation succeeded until the server confirms it.
- Handle permission denied, file exists, missing path, non-empty directory, broken connection, and unsupported operation errors.
- Do not silently retry destructive operations.
- Protect against accidental recursive deletion.
- Ensure the UI does not offer unsupported operations as if they were available.

Add tests and verify behavior against the test SFTP server. Keep protocol-specific operations behind the adapter boundary.
```

**Gate:** Remote file management works safely over SFTP.

---

# Prompt 12 — Implement permissions and ownership management

```text
Implement the remote Properties/Permissions workflow for SFTP.

Required UI:
- File Properties dialog.
- Unix permission matrix for owner, group, and others: read, write, execute.
- Octal mode display and editing.
- Special bits: SUID, SGID, and sticky bit, where supported.
- Owner and group display.
- Owner/group editing when the server and current user permit it.
- Clear indication when an operation is unavailable or denied.

Backend:
- Map permission changes to the correct SFTP operation.
- Treat ownership changes separately from permission changes.
- Use numeric UID/GID where required by the protocol.
- Resolve names with remote `getent` only through a controlled, optional mechanism; do not assume usernames are available from SFTP metadata.
- Validate octal input and permission combinations.
- Support multi-selection only when the semantics are clear.
- If recursive permission changes are implemented, require an explicit recursive option and confirmation.

Do not pretend a normal user can change ownership when the server will reject it. Do not add sudo behavior in this milestone.

Add tests for mode conversion, special bits, validation, and server errors.
```

**Gate:** Permission changes work where allowed and unavailable ownership changes are explained accurately.

---

# Prompt 13 — Implement SSH terminal launch

```text
Implement the external SSH terminal feature.

Requirements:
- Add a toolbar/menu action for opening a terminal for the selected connection.
- Launch the user's configured terminal application using a safe process API, not shell-string concatenation.
- Pass host, port, username, and identity-file options as distinct arguments.
- Use the system OpenSSH client where practical.
- Reuse the user's normal SSH configuration and known-hosts behavior.
- Do not expose passwords or passphrases in command-line arguments.
- Do not claim the terminal is authenticated merely because the process launched.
- Support a configurable terminal command or a documented list of supported terminal emulators.
- Show a useful error if no supported terminal is found.

The terminal must open a separate interactive SSH session and be ready for the user to enter commands after authentication. Do not embed a terminal emulator in this milestone.

Add tests for argument construction and terminal discovery. Manually verify with a test server.
```

**Gate:** A separate terminal opens using the selected profile without leaking credentials.

---

# Prompt 14 — Implement Open With for local files

```text
Implement the local-file Open With workflow using Linux desktop integration.

Requirements:
- Add Open and Open With actions to the local pane context menu.
- Discover installed applications through XDG desktop entries and/or GIO using the approach validated in the technical spike.
- Allow the user to manually choose an installed application.
- Remember a LinuxSCP-specific default application for each file extension.
- Keep the manual Open With option available even when a default exists.
- Handle extensions case-insensitively where appropriate.
- Handle files with no extension and unknown MIME types.
- Do not overwrite or modify system-wide MIME associations unless explicitly requested by a future feature.
- Launch applications safely without shell injection.
- Report launch failures clearly.

Add tests for application discovery, association persistence, and launch argument construction. Verify with multiple installed applications on the target Linux desktop.
```

**Gate:** Local Open With works and extension defaults are stored by LinuxSCP.

---

# Prompt 15 — Implement remote Open With and safe temporary editing

```text
Implement Open With for remote files using a managed local temporary-copy workflow.

Expected flow:
1. User selects a remote file and chooses Open or Open With.
2. LinuxSCP downloads it to a managed temporary directory.
3. LinuxSCP launches the selected local application.
4. LinuxSCP tracks the temporary file and detects local modifications.
5. When the user returns or the file changes, LinuxSCP offers to upload the modified file.
6. Before upload, LinuxSCP checks whether the remote file has changed since the download.
7. If the remote file changed, report a conflict and do not overwrite automatically.
8. Let the user explicitly choose whether to upload, keep local changes, or discard the temporary edit.
9. Clean up temporary files only when safe and according to a documented lifecycle.

Requirements:
- Reuse the local Open With application discovery and extension associations.
- Support manual application choice every time.
- Track remote identity and a useful version fingerprint (for example size + modification time, supplemented where possible).
- Do not assume an editor process exiting means the file was saved.
- Handle applications that keep running after the launcher returns.
- Avoid uploading unchanged files.
- Handle disconnects and failed uploads without losing the local edited copy.
- Never silently overwrite remote changes.

Document temporary-file cleanup behavior and add tests for modified, unchanged, conflicting, disconnected, and failed-upload cases.
```

**Gate:** Remote editing is safe against accidental overwrites and does not lose local edits.

---

# Prompt 16 — Prototype and implement sudo-enabled SFTP (security-critical)

```text
This is a security-critical, optional feature. Read the sudo-SFTP section of the specification and the technical spike report before changing code.

First verify the exact server-side mechanism supported by the selected SSH library:
- Authenticate through SSH as the configured normal user.
- Open a separate SSH session channel.
- Launch the configured external SFTP server binary through a narrowly authorized sudo command.
- Connect the SFTP client protocol to that process with protocol bytes on stdout and diagnostics on stderr.
- Do not substitute an elevated shell or an elevated terminal for an elevated SFTP subsystem.
- Do not assume `internal-sftp` can be launched as an external executable.

Implementation requirements:
1. Make elevated SFTP an explicit connection option.
2. Require the user to configure the exact remote SFTP server binary path.
3. Explain that server-side sudoers configuration is required.
4. Document a narrow sudoers rule for the exact executable; never recommend unrestricted `NOPASSWD: ALL`.
5. Do not attempt to modify sudoers automatically.
6. If sudo requires an interactive password and the implementation cannot safely support it, report that limitation clearly.
7. Display a persistent and unmistakable ELEVATED/ROOT indicator in the remote pane and status area.
8. Require additional confirmation for recursive deletion and other high-impact operations.
9. Preserve normal host-key verification and credential handling.
10. Ensure protocol stdout is not polluted by shell startup messages or diagnostics.
11. Detect unsupported server configurations and fail safely.

Do not implement this feature if the transport library cannot reliably support the required process/channel behavior. In that case, leave the feature disabled, document the blocker, and propose a safe alternative. Do not fake elevated access.

Add security-focused tests and a manual test procedure using a disposable test VM/container. Never test destructive operations against a production server.
```

**Gate:** Elevated SFTP is either demonstrably functional and safe, or explicitly disabled with a documented technical reason.

---

# Prompt 17 — Add FTP and FTPS through a separate adapter

```text
Implement FTP and FTPS as P2 protocol backends, using the dependency validated during the technical spike.

Do not force FTP behavior into the SFTP implementation.

Requirements:
- Add FTP and explicit/implicit FTPS only if the selected library and specification support the required modes.
- Support profile configuration and authentication.
- Implement directory listing, navigation, upload, download, rename, delete, and directory creation only where supported.
- Map FTP metadata carefully; permissions and ownership may be unavailable.
- Handle passive mode and TLS certificate validation correctly.
- Do not silently disable TLS certificate verification.
- Represent protocol limitations through the capability model.
- Keep the dual-pane UI and transfer manager protocol-agnostic.
- Ensure FTP does not expose SSH-specific sudo features.
- Add protocol-specific tests and a reproducible local FTP/FTPS test setup.

Do not begin SCP in this milestone.
```

**Gate:** FTP/FTPS work through their own adapter and accurately expose their capabilities.

---

# Prompt 18 — Add SCP only where its capabilities are honest

```text
Implement SCP support as a separate protocol backend only after reviewing the specification and the validated library support.

SCP is not equivalent to SFTP. Define and document exactly which operations are supported.

Requirements:
- Support connection and transfer operations that the selected SCP implementation can perform safely.
- Do not present unsupported directory browsing, metadata, permissions, ownership, or file-management actions as available.
- Use SSH host-key verification and secure credential handling consistently.
- Preserve transfer queue behavior where technically possible.
- Surface capability limitations in the UI.
- Do not silently fall back from SFTP to SCP or vice versa.
- Add tests for supported operations and explicit unsupported-operation responses.

If a robust SCP implementation is not feasible, keep SCP unavailable and document why rather than shipping a misleading partial implementation.
```

**Gate:** SCP is either honestly supported for its defined operations or explicitly deferred.

---

# Prompt 19 — Hardening, security review, and usability pass

```text
Perform a systematic quality and security pass across the application. Do not add major new features.

Review:
- SSH host-key verification and changed-key handling.
- Secret storage and accidental credential logging.
- Process launching and shell-injection risks.
- Path handling and traversal.
- Symlink behavior.
- Recursive copy/delete safety.
- Transfer cancellation and partial files.
- Remote edit conflict detection.
- Elevated SFTP state and dangerous-operation confirmations.
- TLS certificate validation for FTPS.
- UI behavior during disconnects and application shutdown.
- Temporary-file cleanup.
- SQLite migrations and data integrity.
- Error messages and diagnostic logging.

Usability review:
- Confirm the exact file-column order: Name, Size, Changed, Rights, Owner.
- Confirm navigation controls and per-connection directory restoration.
- Confirm local and remote panes remain independent.
- Confirm internal-only drag-and-drop.
- Confirm Open With defaults never remove the manual choice.
- Confirm transfer progress is real.
- Confirm elevated mode is visually unmistakable.

Fix defects found. Add regression tests for each fix. Write `docs/SECURITY_REVIEW.md` with findings, severity, remediation, and any remaining risks.

Run the full test suite, static analysis, formatting, frontend checks, and production build.
```

**Gate:** No unresolved critical/high security defect; all known limitations are documented.

---

# Prompt 20 — Packaging, installation, and release readiness

```text
Prepare LinuxSCP for a first usable Linux release.

Tasks:
1. Choose packaging formats supported by the project and target distributions (for example AppImage and/or native packages).
2. Add application icon, desktop entry, application metadata, and version information.
3. Verify runtime dependencies and Linux desktop integration.
4. Verify Secret Service access in the packaged application.
5. Verify launching installed applications through Open With.
6. Verify terminal discovery and launch.
7. Verify file chooser and temporary-directory behavior.
8. Add a clean install/uninstall procedure.
9. Document supported distributions and known limitations.
10. Add release build instructions and a reproducible release checklist.
11. Ensure no development-only paths or debug credentials are included.
12. Do not claim broad distribution compatibility without testing it.

Run a clean release build and test the packaged artifact on a clean Linux environment or VM. Create `docs/RELEASE_CHECKLIST.md` and update the README with installation and troubleshooting instructions.
```

**Gate:** A packaged build installs and runs on the explicitly tested Linux environment.

---

# Prompt 21 — Final acceptance audit against the specification

```text
Perform a final acceptance audit of the implemented LinuxSCP application against every requirement in LinuxSCP_Product_and_Technical_Specification.md.

Create a traceability matrix with columns:
- Requirement ID or section.
- Requirement.
- Priority.
- Status: implemented, partially implemented, deferred, unsupported, or not implemented.
- Evidence: test name, code location, or reproducible manual procedure.
- Known limitation.

Do not mark a feature implemented merely because a button or dialog exists. Require working behavior and evidence.

Specifically verify:
- Dual-pane local/remote browsing.
- Exact column order.
- Saved profiles and secure credential storage.
- SFTP host-key verification.
- Upload/download and transfer queue.
- Internal-only pane-to-pane drag-and-drop.
- Navigation history and last-directory restoration.
- File and directory creation, rename, and deletion.
- Permissions and ownership.
- Elevated SFTP security boundary.
- Local and remote Open With.
- Per-extension application defaults with manual override.
- Remote temporary editing and conflict detection.
- External SSH terminal launch.
- FTP/FTPS and SCP capability honesty.
- Packaging and Linux desktop integration.

Run all automated checks and list the manual acceptance tests that still require a human. Do not conceal incomplete features. Update the README and specification status only where evidence supports the change.

Finish with:
1. Release recommendation.
2. Blocking defects.
3. Deferred features.
4. Known platform limitations.
5. Exact commands to build and run the release.
```

**Gate:** The traceability matrix is complete and the release status is evidence-based.

---

## Optional follow-up prompts

Use these only when the relevant feature is already implemented and stable.

### Optional A — Add multiple connection tabs

```text
Add multiple connection tabs. Each tab must own an independent remote session, remote path, navigation history, selection state, and transfer context. Avoid sharing mutable session state across tabs. Transfers must continue safely if the user switches tabs. Add tests for tab switching, disconnecting one tab, and concurrent sessions.
```

### Optional B — Add bookmarks

```text
Add local and remote directory bookmarks. Remote bookmarks must be associated with a connection profile and must not be applied to a different server accidentally. Support add, rename, remove, and navigate. Persist bookmarks in SQLite and add migration/tests.
```

### Optional C — Add search

```text
Add filename search for the current local or remote directory tree. Keep local and remote search implementations separate. Make recursion depth, hidden-file inclusion, and cancellation explicit. Avoid overwhelming the UI with results; support navigation to the containing directory. Do not claim remote content search unless it is actually implemented.
```

---

## Practical operating advice

- Use one prompt per Antigravity task. Do not ask Gemini to implement the whole specification in one pass.
- Review diffs before accepting changes, especially in authentication, host-key verification, path handling, transfer code, and sudo-SFTP.
- Require the agent to run commands and show actual output. A statement such as “tests pass” without the command and result is insufficient.
- Keep a working commit after every gate.
- If a milestone is too large for one context window, split it into smaller implementation tasks while preserving the same gate and acceptance criteria.
- If the agent starts implementing a later milestone, tell it to stop and return to the current milestone.
- For bugs, give the agent the exact reproduction steps and ask it to add a regression test before fixing the implementation.
- Do not let the agent weaken security controls merely to make a demo work.
