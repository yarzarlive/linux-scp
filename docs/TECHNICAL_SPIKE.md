# LinuxSCP — Technical Feasibility Spike & Dependency Verification

**Milestone:** M0 (Prompt 1)  
**Date:** 2026-11-12  
**Status:** All Spikes Complete & Validated  
**Target Environment:** Zorin OS 18.1 (Ubuntu 24.04 LTS noble), Linux 7.0.0-42-generic x86_64  

---

## 1. Verified Dependency Choices & Versions

| Layer / Capability | Dependency | Version | License | Maintenance & API Status |
|---|---|---|---|---|
| **Desktop Shell Bridge** | `tauri` | `2.12.5` | Apache-2.0 / MIT | Highly active; native WebKit2GTK-4.1 (2.52.6) and GTK 3.24 support. |
| **Frontend Framework** | `react` / `react-dom` | `18.3.1` | MIT | Active standard; paired with Vite 8.3 & TypeScript 5.8. |
| **SSH Client Transport** | `russh` | `0.65.1` | Apache-2.0 | Actively maintained Tokio-native SSH client; full public key and callback API. |
| **SFTP Protocol Engine** | `russh-sftp` | `3.0.1` | Apache-2.0 | Actively maintained; attaches to any `AsyncRead + AsyncWrite` stream. |
| **Credential Storage** | `keyring` | `4.2.0` | MIT / Apache-2.0 | Modern `zbus` Secret Service backend over DBus; zero plaintext disk storage. |
| **Desktop App Parsing** | `shlex` + standard I/O | `2.0.1` | Apache-2.0 / MIT | Robust quote-aware CLI tokenization for desktop argument expansion. |
| **Key & SSH Encodings** | `ssh-key` | `0.7.0` | Apache-2.0 / MIT | RustCrypto OpenSSH key parser, supports SHA256 fingerprinting. |

---

## 2. Technical Feasibility Prototypes & Actual Results

### Prototype 1 & 2: Tauri 2 Linux Shell & Typed React IPC
* **Directory:** `spikes/tauri-spike`
* **Implementation:**
  - Rust command: `probe_environment(dir_path: String) -> Result<ProbeResult, String>` returning structured `LocalItemInfo` records.
  - TypeScript interface: `ProbeResult` matching the Rust payload.
  - Frontend: `invoke<ProbeResult>("probe_environment", { dirPath: path })`.
* **Results:**
  - `cargo test` in `src-tauri` passed with 100% success (`test tests::test_probe_environment ... ok`).
  - `npm run build` (`tsc && vite build`) passed with zero type errors in 194ms.
  - Native Linux binary compiled and linked with GTK 3 and WebKit2GTK-4.1.

### Prototype 3 & 4: Strict SSH Host-Key Verification & SFTP Browsing
* **Target:** `ubuntu@192.168.122.10:22` (OpenSSH on Ubuntu VM)
* **Implementation:** `spikes/protocol-spike/src/main.rs`
* **Results:**
  - **Host-Key Check:** Intercepted `ssh-ed25519` key in `Handler::check_server_key` and extracted exact fingerprint `SHA256:Vv0VgQVgEC7Xn8zymXiYc6H72l+DCAEiaf5PURe/dMg`.
  - **Auth:** Authenticated via `~/.ssh/id_rsa` using `rsa-sha2-512` (avoiding deprecated SHA-1 signatures).
  - **SFTP Browsing:** Initialized `SftpSession`, listed `/home/ubuntu`, and retrieved exact file sizes and octal Unix permissions (`700`, `644`, `600`).

### Prototype 5: Executed SFTP Process Channel (Sudo SFTP Baseline)
* **Target:** `ubuntu@192.168.122.10`
* **Implementation:** Opened session channel (`session.channel_open_session().await?`) and executed `/usr/lib/openssh/sftp-server` directly on the channel.
* **Results:**
  - Wrapped `channel.into_stream()` into `SftpSession::new()`.
  - Succeeded in listing directory entries over the executed binary channel without invoking the standard subsystem.
  - When testing `sudo -n /usr/lib/openssh/sftp-server` against an unconfigured sudo account, the channel stream immediately cleanly closed (`sender dropped` / EOF), avoiding hung sessions and generating a clean diagnostic error.

### Prototype 6: Linux Secret Service & Keyring
* **Implementation:** `spikes/protocol-spike/src/bin/keyring_test.rs`
* **Results:**
  - Created entry in `linuxscp_test_service`.
  - Stored test secret in GNOME Keyring over DBus.
  - Retrieved stored secret and validated length (22 bytes).
  - Cleaned up and deleted test credential. Zero secrets written to SQLite or filesystem.

### Prototype 7: XDG Desktop Application Discovery & Safe Launch
* **Implementation:** `spikes/protocol-spike/src/bin/xdg_test.rs`
* **Results:**
  - Discovered 94 installed applications across `/usr/share/applications` and `~/.local/share/applications`.
  - Extracted binary executable and arguments (`%F`, `%U`, etc.), filtering out `NoDisplay=true`.
  - Verified safe process construction using `shlex` and `std::process::Command` without shell string interpolation (`sh -c`). Verified that arguments with spaces are preserved verbatim.

---

## 3. Unsupported or Uncertain Capabilities

1. **`internal-sftp` Elevation:**
   - Servers using `Subsystem sftp internal-sftp` cannot be elevated via `sudo -n /usr/lib/openssh/sftp-server` because no external executable exists in that mode.
   - *Status:* Supported as documented limitation with clear detection and error reporting.
2. **Interactive Sudo SFTP:**
   - Interactive password prompting for `sudo` cannot be supported on the SFTP binary channel because any non-SFTP output corrupts the binary handshake.
   - *Status:* Strictly non-interactive `sudo -n` will be enforced per specification.

---

## 4. Risks and Fallback Options

| Risk | Impact | Fallback Option |
|---|---|---|
| **Secret Service Daemon Absent / Headless** | `keyring` operations fail on minimal window managers or containers without DBus Secret Service. | Surface clear modal informing the user that credentials cannot be saved; prompt for password on each session instead of falling back to insecure plaintext storage. |
| **Custom SFTP Server Paths** | Distros use varying paths (`/usr/lib/openssh/sftp-server`, `/usr/libexec/openssh/sftp-server`, `/usr/lib/ssh/sftp-server`). | Connection profile configuration allows specifying a custom binary path per server. |
| **RSA Key Deprecation** | Modern OpenSSH rejects legacy `ssh-rsa` SHA-1. | Enforce `rsa-sha2-256` or `rsa-sha2-512` in key authentication by default. |

---

## 5. Recommended Architecture Decision

1. **Adopt Tauri 2 + React + TypeScript + Rust (`russh` + `russh-sftp`)** as the core architecture.
2. **Proceed to Milestone 1:** Scaffold the clean application codebase and implement the local filesystem Commander-style dual pane.
