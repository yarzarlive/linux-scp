---
name: tauri-rust-development
description: >-
  Use this skill when developing, refactoring, or reviewing Tauri 2 desktop shell code,
  Rust backend services, IPC commands and events, Tokio async runtime concurrency,
  and SQLite persistence for LinuxSCP.
---

# Tauri 2 & Rust Backend Development for LinuxSCP

This skill provides architectural rules, implementation patterns, and security constraints for developing the Rust backend and Tauri 2 desktop integration in LinuxSCP.

---

## 1. Architectural Boundaries

LinuxSCP maintains strict separation between the desktop bridge, domain services, and protocol backends:

```text
src-tauri/
├── src/
│   ├── main.rs                 # Tauri application setup, plugins, and builder
│   ├── commands/               # Thin IPC command handlers (parameter parsing & delegation)
│   ├── connections/            # Connection session lifecycle and registry
│   ├── navigation/             # Pane navigation state and history
│   ├── filesystem/
│   │   ├── local/              # Local filesystem provider (async via Tokio)
│   │   └── remote/             # Remote filesystem traits and abstractions
│   ├── protocols/              # Protocol backends (SFTP, FTP/FTPS, SCP)
│   ├── elevation/              # Sudo-enabled SFTP session channel manager
│   ├── transfers/              # Transfer queue, chunk streaming, and progress
│   ├── metadata/               # Unix permissions, ownership, and stat parsing
│   ├── remote_editor/          # Temp file lifecycle, file watching, and conflict detection
│   ├── desktop_integration/    # XDG .desktop parsing and application launcher
│   ├── terminal/               # External SSH terminal emulator launcher
│   └── storage/                # SQLite connection and migration management
└── tauri.conf.json             # Tauri 2 window configuration and capabilities
```

### Core Architecture Rules
1. **Thin Command Layer:** Functions in `src-tauri/src/commands/` must only extract arguments from Tauri commands, validate basic input types, forward requests to underlying domain services, and return serializable responses (`Result<T, AppError>`).
2. **No Direct I/O in Handlers:** Never perform direct filesystem I/O, network requests, or database queries inside Tauri command functions.
3. **Domain Layer Independence:** Core domain logic (`filesystem`, `protocols`, `transfers`) must not depend on `tauri::AppHandle` unless strictly required for event dispatching.
4. **Reference:** Consult `docs/LinuxSCP_Product_and_Technical_Specification.md` Section 16 for system architecture.

---

## 2. Tauri 2 Patterns & IPC Contracts

### Command Declaration
- Use Tauri 2 attribute macros: `#[tauri::command]`
- Commands returning results should use a unified typed error `AppError` implementing `serde::Serialize`:

```rust
#[derive(Debug, thiserror::Error, serde::Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum AppError {
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Filesystem error: {0}")]
    Filesystem(String),
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Authentication error: {0}")]
    Authentication(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

#[tauri::command]
pub async fn list_local_directory(
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<FileEntry>, AppError> {
    state.local_fs.list_directory(&path).await.map_err(Into::into)
}
```

### Event Emission
- Emit events to the frontend via `AppHandle::emit` or `WebviewWindow::emit` with typed payloads.
- Define event name constants to avoid string typos across the IPC boundary:
  - `EVENT_TRANSFER_PROGRESS = "transfer://progress"`
  - `EVENT_TRANSFER_STATUS = "transfer://status"`
  - `EVENT_REMOTE_EDIT_CONFLICT = "remote-edit://conflict"`

---

## 3. Tokio Async Concurrency & Session Management

1. **Non-blocking Main Thread:** The Tauri UI thread and Tokio async worker threads must never be blocked by synchronous operations.
   - Use `tokio::task::spawn_blocking` for blocking operations (e.g., synchronous file I/O, heavy cryptographic hashing).
2. **Session Ownership:**
   - Each connection session owns its protocol client and navigation state.
   - Store sessions in a thread-safe registry: `Arc<RwLock<HashMap<SessionId, Session>>>`.
   - Never share mutable protocol streams between concurrent operations without synchronization.
3. **Cooperative Cancellation:**
   - Transfer tasks and directory scans must accept a `tokio_util::sync::CancellationToken`.
   - Polling loops and streaming chunks must check `cancellation_token.is_cancelled()` regularly and cleanly release network and file resources upon cancellation.

---

## 4. SQLite Persistence & Storage Layer

- **Engine:** SQLite with versioned migrations (`sqlx` or `rusqlite`).
- **Data Scope:**
  - Store connection profiles, pane state (last directories), remembered file-application associations, and user preferences.
  - **NEVER** store plain-text secrets, passwords, or private key passphrases in SQLite. Store secret references or credential IDs mapped to the Linux Secret Service.
- **Transactions:** Use explicit transactions for atomic batch operations.
- **Reference:** See `docs/LinuxSCP_Product_and_Technical_Specification.md` Section 15 for SQL schema specifications.

---

## 5. Security & Error Handling

- **Credential Redaction:** Use custom `Debug` implementations on structures carrying credentials or private keys, replacing values with `[REDACTED]`.
- **Structured Logging:** Use the `tracing` crate. Configure logging levels cleanly: `info` for high-level operations, `debug` for detailed transitions. Never log raw network buffers or sensitive arguments.
- **Safe Path Normalization:** Resolve and validate all paths before performing operations. Check for path traversal (`..`) when receiving untrusted remote paths.

---

## 6. Common Pitfalls to Avoid

- **Tauri 1 vs Tauri 2 Incompatibilities:** Do not use deprecated Tauri 1 APIs (such as old plugin systems or Tauri 1 window event APIs). Follow Tauri 2 official documentation.
- **Global Mutable State:** Avoid `lazy_static` or global `static mut`. Rely on `tauri::manage` and `tauri::State`.
- **Swallowing Errors:** Always propagate errors with contextual information; never use unhandled `.unwrap()` or `.expect()` in production paths.

---

## 7. Testing & Verification Requirements

- Run unit tests for command parameter validation, state transitions, and error serialization:
  ```bash
  cargo test
  ```
- Run formatting and linter checks:
  ```bash
  cargo fmt --check
  cargo clippy --all-targets -- -D warnings
  ```
- Verify desktop compilation:
  ```bash
  cargo check --manifest-path src-tauri/Cargo.toml
  ```

