---
name: software-testing
description: >-
  Use this skill when designing, writing, or running tests for LinuxSCP, including Rust unit and integration
  tests, React frontend tests, containerized OpenSSH test environments, and milestone verification gates.
---

# Software Testing & Verification for LinuxSCP

This skill outlines the testing methodology, verification gates, and testbed configurations required for LinuxSCP.

---

## 1. Testing Pyramid & Strategy

LinuxSCP combines frontend unit tests, Rust backend service tests, and real containerized protocol integration tests:

```text
       / \
      / E2E \       Containerized OpenSSH & Desktop Integration
     /-------\
    /  Integ  \     SQLite Migrations, Secret Service, SFTP Engine
   /-----------\
  /    Unit     \   Path parsing, permissions, sort algorithms, UI components
 /---------------\
```

---

## 2. Unit Testing Scope

### Rust Backend Tests
- **Path Navigation:** Relative path resolution, root boundary checks, parent navigation (`..`).
- **Metadata Formatting:** Conversion between numeric mode bits (`0o755`) and symbolic representations (`rwxr-xr-x`).
- **Extension Normalization:** Handling compound extensions (`.tar.gz`), case insensitivity, dotfile edge cases (`.bashrc`).
- **Transfer Mathematics:** Accurate rolling-average speed calculations, byte formatting, percentage, and ETA calculations.
- **Run with:**
  ```bash
  cargo test
  ```

### React & TypeScript Frontend Tests
- **Pane History:** Back and forward stack push/pop behavior and boundary conditions.
- **Selection Mechanics:** Single selection, multi-selection (Ctrl+click), and range selection (Shift+click).
- **Table Sorting:** Ascending/descending sort across Name, Size, Date, Rights, and Owner.
- **Run with:**
  ```bash
  npm test
  ```

---

## 3. Containerized OpenSSH Testbed

Do not rely solely on mock objects for SSH/SFTP protocols. Test against a dedicated, isolated OpenSSH container:

### Container Requirements
Use Docker or Podman to spin up an OpenSSH server with:
1. **Users:**
   - `testuser`: Standard unprivileged account (password + public key).
   - `deploy`: Account configured for elevated SFTP testing.
2. **Sudoers Rule:**
   ```sudoers
   deploy ALL=(root) NOPASSWD: /usr/lib/openssh/sftp-server
   ```
3. **Fixture Filesystem:**
   - Root-owned directory (`/protected`, mode `0700`).
   - Standard user directory (`/home/testuser/files`).
   - Symlinks (valid and broken targets).
   - Dotfiles and files with spaces/special characters.

### Mandatory Protocol Test Cases
1. **Host-Key Verification:**
   - Connect to unknown host -> verify fingerprint prompt is returned.
   - Connect after trusting -> verify connection succeeds.
   - Alter container host key -> verify connection is immediately aborted with MITM warning.
2. **SFTP File Operations:**
   - Recursive upload and download of directory trees.
   - Create empty file, create directory, rename, delete.
   - Set permissions (`chmod`) and verify on disk.
3. **Elevated SFTP:**
   - Connect as `deploy` with sudo-SFTP enabled.
   - Successfully list, upload to, and delete files inside `/protected`.
   - Verify unprivileged sessions fail on `/protected` with Permission Denied.
4. **Interruption & Cancellation:**
   - Cancel large file transfer mid-stream; verify temporary file cleanup and connection integrity.

---

## 4. Verification Gates (Mandatory Before Milestone Completion)

Every milestone must satisfy all verification gates prior to approval:

```bash
# 1. Rust Code Quality
cargo fmt --check
cargo clippy --all-targets -- -D warnings

# 2. Frontend Code Quality
npm run lint
npx tsc --noEmit

# 3. Unit and Integration Test Execution
cargo test
npm test

# 4. Compilation & Build Verification
cargo check --manifest-path src-tauri/Cargo.toml
npm run build
```

---

## 5. Common Testing Pitfalls

- **Mocking Out Real Protocol Bugs:** Avoid writing tests that only mock protocol responses. Real network timing and channel behaviors surface only with real SSH daemons.
- **Flaky Async Timers:** Avoid arbitrary `sleep` calls in tests. Use async notification primitives (`tokio::sync::Notify`, channels) to synchronize test events.
- **Testing Only Happy Paths:** Always include tests for connection drops, permission denied (`EACCES`), read-only filesystems, and disk exhaustion.

---

## 6. Milestone Completion Reporting

When completing a task or milestone, report:
1. Files created or modified.
2. Functionality implemented.
3. Exact commands executed and their output.
4. Test results (pass/fail count).
5. Known limitations or deferred items.
6. Step-by-step instructions for manual verification.
- Reference documentation: `docs/LinuxSCP_Product_and_Technical_Specification.md` Section 19 and `docs/LinuxSCP_Antigravity_Step_by_Step_Development_Prompts.md`.

