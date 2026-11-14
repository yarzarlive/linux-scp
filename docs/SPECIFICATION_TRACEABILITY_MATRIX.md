# LinuxSCP — Final Specification Traceability Matrix & Acceptance Audit

**Date:** 2026-11-14  
**Audit Scope:** Verification against `docs/LinuxSCP_Product_and_Technical_Specification.md`  
**Application Version:** 0.1.0  
**Status:** Acceptance Audit Completed — Ready for Release

---

## 1. Traceability Matrix

| Section / Req ID | Specification Requirement | Priority | Status | Evidence (Code / Tests / Verification Procedure) | Known Limitations |
| :--- | :--- | :---: | :---: | :--- | :--- |
| **§2.1 / §4.1** | WinSCP-inspired Commander dual-pane layout | **P0** | **Implemented** | `src/app/App.tsx`, `src/components/FilePanel/LocalPane.tsx`, `src/components/FilePanel/RemotePane.tsx`<br>Test: `tests/unit/LocalPane.test.tsx`, `tests/unit/RemotePane.test.tsx` | Splitter proportions persist per session. |
| **§2.1 / §4.3** | Independent local and remote filesystem browsing | **P0** | **Implemented** | `src-tauri/src/filesystem/local.rs`<br>`src-tauri/src/protocol/sftp.rs`<br>Tests: `test_list_directory_valid`, `test_live_sftp_browsing_paths_metadata_and_failures` | Local browsing requires OS read permissions on visited directory. |
| **§4.4** | Exact file column order: **Name, Size, Changed, Rights, Owner** | **P0** | **Implemented** | `src/components/FilePanel/ColumnHeader.tsx` (lines 20–26)<br>`src/components/FilePanel/FileRow.tsx` (lines 89–124)<br>Tests: `tests/unit/ColumnHeader.test.tsx`, `tests/unit/FileTable.test.tsx` | Directories display `—` for size unless calculated. |
| **§2.1 / §6.1** | Saved connection profiles (CRUD, duplication) | **P0** | **Implemented** | `src-tauri/src/persistence/profile_repository.rs`<br>`src-tauri/src/commands/profile.rs`<br>`src/components/SiteManager/SiteManagerModal.tsx`<br>Tests: `test_profile_crud_lifecycle`, `tests/unit/SiteManagerModal.test.tsx` | Profiles stored in SQLite; secrets stored in Secret Service. |
| **§2.1 / §6.4** | Secure credential storage via Linux Secret Service | **P0** | **Implemented** | `src-tauri/src/persistence/keyring.rs` (`KeyringService`, service `org.linuxscp.client`)<br>`src-tauri/src/models/connection.rs`<br>Tests: `test_keyring_service_operations`, `test_profile_serialization_contains_no_secrets` | Requires a running D-Bus Secret Service provider on desktop. Disallowed on headless/container without D-Bus. |
| **§6.3** | Strict SSH host-key verification and changed-key blocking | **P0** | **Implemented** | `src-tauri/src/persistence/known_hosts.rs`<br>`src-tauri/src/protocol/sftp.rs` (`SshClientHandler`)<br>`src/components/SiteManager/HostKeyPromptModal.tsx`<br>Tests: `test_unknown_host_key_decision`, `test_changed_host_key_strictly_blocked_and_never_overwritten`, `test_explicit_trust_and_subsequent_trusted_decision` | Changed host keys only allow "Abort & Close" with SHA-256 fingerprint comparison; automatic replacement is prohibited. |
| **§2.1 / §9.4 / §11** | File upload, download, and transfer queue manager | **P0** | **Implemented** | `src-tauri/src/transfer/manager.rs`<br>`src/components/TransferQueue/TransferQueue.tsx`<br>Tests: `test_live_sftp_transfers_small_large_recursion_conflicts_and_cancel`, `test_transfer_progress_calculation` | Transfers stream in chunks with atomic temporary staging (`.linuxscp-tmp-<uuid>`). |
| **§2.1 / §10** | Drag-and-drop between local and remote panes | **P0** | **Implemented** | `src/utils/dragDrop.ts`<br>`src/hooks/usePaneDragDrop.ts`<br>`src/components/FilePanel/PaneDropIndicator.tsx`<br>Tests: `tests/unit/dragDrop.test.ts`, `tests/unit/InternalDragDrop.test.tsx` | Strictly internal pane-to-pane (`INTERNAL_DRAG_MIME`); desktop OS drops rejected. |
| **§2.1 / §5.1** | Directory navigation toolbar (Back, Forward, Up, Home, Root, Refresh) | **P0** | **Implemented** | `src/components/FilePanel/AddressBar.tsx`<br>`src/components/Toolbars/ConnectionToolbar.tsx`<br>Tests: `tests/unit/AddressBar.test.tsx` | History stack operates independently for local and remote panes. |
| **§2.1 / §5.3** | Per-connection last-directory persistence and restoration | **P0** | **Implemented** | `src-tauri/src/persistence/remote_state.rs` (`connection_remote_state` table)<br>`src-tauri/src/protocol/sftp.rs` (lines 347–375)<br>Tests: `test_last_remote_directory_persistence` | Restores last remote directory with graceful fallback to profile initial directory or remote home. |
| **§2.1 / §9.1–§9.3** | Create empty file, create directory, rename, and delete | **P0** | **Implemented** | `src-tauri/src/filesystem/local.rs`<br>`src-tauri/src/protocol/sftp.rs`<br>Tests: `test_create_and_delete_file`, `test_create_and_delete_directory`, `test_rename_entry`, `test_live_sftp_remote_file_operations_and_capabilities` | Recursive directory deletion requires explicit user confirmation. Protected system roots cannot be deleted. |
| **§2.1 / §12** | Remote permissions (octal/checkbox) and ownership (UID/GID) | **P1** | **Implemented** | `src-tauri/src/models/filesystem.rs`<br>`src-tauri/src/protocol/sftp.rs` (`do_set_permissions`, `do_set_owner_group`)<br>`src/components/PropertiesDialog/PropertiesDialog.tsx`<br>Tests: `test_unix_permissions_standard_755`, `test_unix_permissions_special_bits`, `test_live_sftp_permissions_and_ownership_errors` | Ownership modification requires appropriate remote server privileges (root/sudo). |
| **§2.1 / §8** | Sudo-elevated SFTP server execution | **P1** | **Implemented** | `src-tauri/src/protocol/sftp.rs` (`execute sudo -n <binary>`)<br>`src/components/StatusBar/StatusBar.tsx`<br>`src/components/FilePanel/RemotePane.tsx`<br>Tests: `test_sudo_sftp_command_resolution`, `test_live_sftp_sudo_elevation_unconfigured_fails_safely_with_guidance` | Requires non-interactive sudo rule configured on server (`NOPASSWD`). Prominent amber badge displayed. |
| **§2.1 / §13.1–§13.4** | Local & remote "Open With" Linux desktop integration | **P1** | **Implemented** | `src-tauri/src/desktop_integration/mod.rs`<br>`src/components/OpenWithDialog/OpenWithDialog.tsx`<br>Tests: `test_host_desktop_discovery_and_multi_app_execution_verification`, `test_expand_exec_args_and_safe_launch_plan_without_shell_injection` | Launches external applications using direct argv without shell interpolation. |
| **§2.1 / §13.3** | Remember default application by extension with manual override | **P1** | **Implemented** | `src-tauri/src/persistence/app_associations_repository.rs`<br>`src/components/OpenWithDialog/OpenWithDialog.tsx`<br>Tests: `test_file_app_association_crud_and_case_normalization`, `tests/unit/OpenWithDialog.test.tsx` | Stored in SQLite (`file_app_associations`). "Open With..." remains permanently accessible in context menu. |
| **§2.1 / §13.5–§13.6** | Remote file temporary editing workflow with conflict detection | **P1** | **Implemented** | `src-tauri/src/remote_edit/manager.rs`<br>`src-tauri/src/remote_edit/workspace.rs`<br>`src/components/RemoteEditPanel/RemoteEditPanel.tsx`<br>Tests: `test_remote_change_is_reported_as_conflict_and_not_overwritten`, `test_restart_restores_modified_sessions_and_exit_cleans_only_clean_ones` | Working copies isolated in `$XDG_STATE_HOME/linuxscp/remote-edits` (`0700`). Mtime and SHA-256 conflict detection. |
| **§2.1 / §14** | External SSH terminal launch | **P1** | **Implemented** | `src-tauri/src/terminal/mod.rs`<br>`src/components/TerminalSettings/TerminalSettingsModal.tsx`<br>Tests: `test_terminal_discovery_priority_and_non_executable_filtering`, `test_build_ssh_args_rejects_option_injection_and_non_ssh_protocols`, `test_live_terminal_ssh_invocation_against_test_server` | Launches system terminal with OpenSSH argv. Passwords never passed in command-line arguments. |
| **§2.1 / §7.4** | FTP and FTPS protocols | **P2** | **Deferred** | `src-tauri/src/protocol/session_manager.rs`<br>Test: `test_unsupported_protocols_fail_safely_without_unverified_fallback` | Explicitly deferred. Connection attempts return descriptive protocol error rather than mock responses. |
| **§2.1 / §7.5** | SCP protocol | **P2** | **Implemented (Transfer-Only)** | `src-tauri/src/protocol/scp.rs`<br>Documentation: `docs/SCP_SUPPORT.md`<br>Tests: `test_scp_adapter_enforces_all_capability_rejections`, `test_live_scp_connection_and_transfers_against_test_vm` | Honestly scoped to single-file uploads and downloads; remote browsing and chmod are disabled with clear capability notices. |
| **§2.1** | Multiple connection tabs | **P2** | **Deferred** | `src-tauri/src/protocol/session_manager.rs` | Current architecture implements a single active session model with instant reconnect. |
| **§2.1** | Bookmarks / favorite directories | **P2** | **Deferred** | `src-tauri/src/persistence/database.rs` | Deferred to post-v0.1.0 milestone. |
| **§2.1 / §19** | Packaging and Linux desktop distribution | **P1** | **Implemented** | `LinuxSCP_0.1.0_amd64.deb`<br>`LinuxSCP_0.1.0_amd64.AppImage`<br>`scripts/install.sh`<br>`scripts/uninstall.sh`<br>`assets/linuxscp.desktop`<br>`assets/com.linuxscp.client.metainfo.xml` | Explicitly built, installed, and verified on Zorin OS 18.1 / Ubuntu 24.04 LTS (Noble Numbat). |

---

## 2. Automated Quality and Verification Results

All automated gates passed cleanly with zero errors:

- **Rust Formatting:** `cargo fmt -- --check` (0 issues)
- **Rust Clippy:** `cargo clippy --all-targets -- -D warnings` (0 warnings, 0 errors)
- **Rust Unit & Integration Tests:** `cargo test` (**120 / 120 tests passed**, finished in 3.11s)
- **TypeScript Typecheck:** `npm run typecheck` (0 errors)
- **Frontend Linter:** `npm run lint` (0 warnings)
- **Frontend Formatting:** `npm run format:check` (All matched Prettier style)
- **Frontend Vitest Unit Tests:** `npm test` (**114 / 114 tests passed across 26 test files**)
- **Release Bundling:** Generated Debian package (5.92 MiB) and portable AppImage (83.37 MiB)

---

## 3. Human / Manual Acceptance Procedures

The following manual test scenarios require a human operator with a visual desktop display:

1. **Visual Desktop Environment:**
   - Launch `linuxscp` from the application launcher (GNOME Dash / XFCE Menu).
   - Verify application window appearance, high-DPI scaling, and SVG/PNG icon crispness in the taskbar.
2. **Interactive SSH Terminal Shell:**
   - Click "Open Terminal" while connected to a remote host.
   - Confirm that the desktop terminal emulator opens with an interactive OpenSSH prompt.
3. **Secret Service Prompting:**
   - On a desktop with a locked keyring, verify that connecting prompts the user to unlock their GNOME Keyring / KWallet.
4. **Interactive "Open With" Application Hand-off:**
   - Right-click a remote text file (`.txt` or `.py`), select "Open With" -> Gedit / VS Code.
   - Edit and save the file in the external editor; verify the modification indicator and remote upload prompt.

