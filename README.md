# LinuxSCP

> A modern, WinSCP-inspired dual-pane file transfer client for the Linux desktop.

Built with **Tauri 2**, **Rust**, **Tokio**, **React 18**, **TypeScript**, and **Tailwind CSS**.

---

## 1. Prerequisites

### Linux System Dependencies (Ubuntu / Debian / Zorin OS)
```bash
sudo apt update && sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libdbus-1-dev \
  build-essential \
  curl \
  wget \
  file \
  pkg-config
```

### Language Toolchains
- **Rust:** `1.80+` (managed via `rustup`)
- **Node.js:** `v20+` or `v22+` (with `npm`)

---

## 2. Setup & Installation

Clone the repository and install frontend dependencies:

```bash
git clone https://github.com/your-org/LinuxSCP.git
cd LinuxSCP
npm install
```

---

## 3. Development Commands

### Run Desktop App in Development Mode (Live Reload)
```bash
npm run tauri dev
```

### Run Frontend in Vite Browser Mode
```bash
npm run dev
```

---

## 4. Testing & Quality Verification

LinuxSCP enforces strict verification gates before every milestone completion:

### Frontend Checks & Tests
```bash
# 1. Typecheck TypeScript contracts
npm run typecheck

# 2. Check code formatting
npm run format:check

# 3. Lint source files
npm run lint

# 4. Run Vitest unit tests
npm test

# 5. Build frontend distribution bundle
npm run build
```

### Rust Backend Checks & Tests
```bash
cd src-tauri

# 1. Check Rust formatting
cargo fmt --check

# 2. Run Clippy with zero warnings allowed
cargo clippy --all-targets -- -D warnings

# 3. Run Rust unit tests
cargo test

# 4. Compile desktop debug binary
cargo build
```

---

## 5. Security & Privacy Architectural Principles

1. **Zero Plaintext Secrets:** Passwords, private-key passphrases, and tokens are stored exclusively in the Linux Secret Service (via DBus) and never written to SQLite, plaintext files, or application logs.
2. **Log Redaction:** Structured logging via `tracing` actively filters and redacts any occurrence of credentials, keys, and PEM blocks.
3. **Strict Host-Key Verification:** Host keys are verified explicitly; changed or untrusted keys are never accepted automatically.
4. **Internal Drag-and-Drop Boundary:** Drag-and-drop is restricted strictly between LinuxSCP's local and remote panes; desktop file dropping is unsupported.
5. **No Shell Interpolation:** External commands and desktop entries are launched with explicit argument vectors using `std::process::Command`, preventing command injection vulnerabilities.

---

## 6. Architecture & Directory Overview

```text
LinuxSCP/
├── src/                          # React + TypeScript Frontend
│   ├── app/                      # Main Commander application window layout
│   ├── components/               # UI components (Header, StatusBar, Panes)
│   ├── services/                 # Type-safe IPC client wrappers (invoke & listen)
│   └── types/                    # Domain models and IPC DTO contracts
│
├── src-tauri/                    # Tauri 2 & Rust Backend
│   ├── src/
│   │   ├── main.rs               # Binary entry point
│   │   ├── lib.rs                # Tauri runtime builder & handler registration
│   │   ├── commands/             # Thin IPC command controllers
│   │   ├── errors.rs             # Typed AppError enumeration
│   │   └── logging.rs            # Security-filtered structured logger
│   └── tauri.conf.json           # Tauri 2 window & bundle capabilities
│
├── tests/                        # Frontend & IPC Unit Tests
├── docs/                         # Specification & architecture blueprints
└── spikes/                       # Milestone 0 technical feasibility prototypes
```

