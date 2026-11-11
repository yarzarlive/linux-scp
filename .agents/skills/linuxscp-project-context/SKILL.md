---
name: linuxscp-project-context
description: >-
  Use this skill to understand the LinuxSCP product specification, architecture decisions,
  milestone roadmap, engineering rules, security boundaries, and project sources of truth.
---

# LinuxSCP Project Context & Architecture Guide

This skill serves as the foundational overview for the LinuxSCP project, outlining its design principles, sources of truth, milestone roadmap, and global engineering rules.

---

## 1. Product Overview & Principles

**LinuxSCP** is a native Linux desktop file-transfer client inspired by WinSCP's Commander interface.

### Core Principles
1. **WinSCP-Style Commander Workflow:** Familiar two-pane file management (local on left, remote on right).
2. **Deep Linux Desktop Integration:** Discovers installed desktop applications via XDG standards, launches external terminals, and respects desktop conventions.
3. **Protocol-Aware Capability Model:** Clean separation between protocols (SFTP first, FTP/FTPS and SCP later). UI dynamically reflects active protocol capabilities without silently falling back to unsafe operations.
4. **Safe by Default:** Strict SSH host-key verification, secure credential storage via the Linux Secret Service, and explicit visual warnings for elevated sessions.
5. **Separation of Concerns:** Rigid isolation between the React UI, Tauri IPC bridge, Rust domain services, and protocol engines.

---

## 2. Authoritative Sources of Truth

- **Product & Technical Specification:** `docs/LinuxSCP_Product_and_Technical_Specification.md` (Authoritative product blueprint).
- **Antigravity Development Prompts:** `docs/LinuxSCP_Antigravity_Step_by_Step_Development_Prompts.md` (Step-by-step milestone execution pack).

*Rule:* If any task prompt appears to conflict with the Product Specification, treat the specification as authoritative, identify the discrepancy, and request clarification before proceeding.

---

## 3. Technology Stack Reference

| Component | Technology | Role |
|---|---|---|
| **Desktop Shell** | Tauri 2 | Linux desktop windowing, system bridge, and capability management |
| **Frontend** | React + TypeScript + Tailwind CSS | UI components, table virtualization, and user interactions |
| **Backend** | Rust | Filesystem operations, protocols, and transfer engine |
| **Async Runtime** | Tokio | Asynchronous protocol streams and background transfer tasks |
| **Local Persistence** | SQLite (`sqlx` / `rusqlite`) | Connection profiles, directory state, app associations, preferences |
| **Credential Storage** | Linux Secret Service (`keyring`) | Secure storage of SSH passwords and private key passphrases |
| **SSH & SFTP** | Maintained Rust SSH crate | SSH transport, SFTP client, and session channels |
| **File Watcher** | `notify` crate | Monitoring temporary files for the remote edit workflow |
| **App Discovery** | XDG Desktop Entry parsing | Discovering installed desktop apps and reading `.desktop` files |
| **Terminal Launch** | OpenSSH in System Terminal | Launching external user terminal (`x-terminal-emulator`, `gnome-terminal`, etc.) |

---

## 4. Milestone Roadmap

Development follows a strict milestone progression:

- **M0 — Feasibility Prototypes:** Prove risky dependencies (Tauri 2, SSH library, host-key checks, sudo-SFTP channel, Secret Service, XDG discovery).
- **M1 — Stack Validation:** Summarize findings in `docs/TECHNICAL_SPIKE.md`.
- **M2 — Application Scaffolding:** Tauri 2 + React + Rust shell, CI, formatting, and linting.
- **M3 — Domain Models:** Data contracts, `RemoteFileSystem` trait, and capability definitions.
- **M4 — Local Filesystem:** Navigation, history, file table, and local file operations.
- **M5 — Connection Manager:** SQLite storage, Secret Service credential handling, and profile dialogs.
- **M6 — SFTP Client:** Connection lifecycle, strict host-key verification, and remote browsing.
- **M7 — Transfer Engine:** Streaming background transfers, chunked I/O, progress reporting, and queue UI.
- **M8 — Internal Drag-and-Drop:** Drag-and-drop between local and remote panes.
- **M9 — Remote File Operations:** Create, rename, delete, chmod, and chown.
- **M10 — Sudo-Enabled SFTP:** Separate SSH channel executing `sudo -n <sftp-server>` with visual indicators.
- **M11 — Open With & Remote Editing:** App discovery, file associations, file watching, and conflict detection.
- **M12 — External Terminal Launch:** Launching system OpenSSH client in preferred desktop terminal.
- **M13 — Hardening & Packaging:** Security audit, AppImage / deb distribution packaging.

---

## 5. Explicit Non-Goals & Boundaries

1. **No External Drag-and-Drop:** Do **not** implement drag-and-drop between LinuxSCP and external desktop applications or the desktop environment. Drag-and-drop is strictly internal (Local <-> Remote).
2. **No Embedded Terminal in Initial Release:** Launch the user's external terminal emulator rather than embedding a complex terminal widget.
3. **Never Auto-Accept Host Keys:** Never bypass or automatically trust unknown or changed SSH host keys.
4. **Never Store Secrets in Plaintext:** Passwords and passphrases must never be stored in SQLite, plain JSON files, or logs.
5. **No Shell String Concatenation:** Never interpolate user paths or application names into shell command strings.
6. **No Mock or Fake Data:** Never use mock timers or fake progress to simulate functionality.

---

## 6. Global Engineering Rules

- **Inspect First:** Always inspect the repository and verify existing code before introducing changes.
- **Bounded Changes:** Implement only the active milestone. Do not implement future milestone features prematurely.
- **Quality Gates:** Verify `cargo fmt`, `cargo clippy`, `npm run lint`, `tsc --noEmit`, and unit/integration tests before completing tasks.
- **Reporting:** Complete every task with a clear report detailing files changed, functionality implemented, test outputs, and manual verification instructions.

