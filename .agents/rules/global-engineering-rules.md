---
trigger: always_on
---

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