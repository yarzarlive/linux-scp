---
name: react-typescript-development
description: >-
  Use this skill when developing, refactoring, or testing the React and TypeScript frontend,
  Commander dual-pane file manager layout, drag-and-drop interactions, UI state management,
  and Tauri IPC integration for LinuxSCP.
---

# React & TypeScript Frontend Development for LinuxSCP

This skill guides the implementation of the LinuxSCP frontend: a WinSCP-inspired Commander-style dual-pane file manager built with React, TypeScript, and Tailwind CSS.

---

## 1. UI Architecture & Layout

The user interface follows the classic Commander dual-pane paradigm:

```text
+-----------------------------------------------------------------------+
| LinuxSCP Menu Bar (File, Commands, Options, Help)                    |
+-----------------------------------------------------------------------+
| Connection Toolbar: [New] [Open] [Disconnect]                         |
| File Operations: [Back] [Forward] [Parent] [Root] [Home] [Refresh]   |
+-----------------------------------+-----------------------------------+
| LOCAL PANE                        | REMOTE PANE                       |
| Path: [/home/user/projects      ] | Path: [/var/www/html            ] |
| --------------------------------- | --------------------------------- |
| Name     Size   Changed   Rights  | Name     Size   Changed   Rights  |
| ..                                | ..                                |
| src/     —      10:24     rwxr-xr | public/  —      09:15     rwxr-xr |
| app.ts   14 KB  10:22     rw-r--r | index.js 22 KB  09:12     rw-r--r |
+-----------------------------------+-----------------------------------+
| Transfer Queue Panel (Collapsible/Resizable)                         |
| website.zip -> /var/www/html   [========---] 72%  4.2 MB/s  Active    |
+-----------------------------------------------------------------------+
| Status Bar: Connected: server (SFTP) | Mode: ELEVATED (ROOT) | Items: 4|
+-----------------------------------------------------------------------+
```

### Component Structure
```text
src/
├── app/                        # Application root and shell layout
├── components/
│   ├── FilePanel/              # Pane container, header, address bar, file table
│   │   ├── AddressBar.tsx      # Editable path bar with Enter-to-navigate
│   │   ├── FileTable.tsx       # Virtualized/sortable file list
│   │   ├── FileRow.tsx         # File entry row with icons and context menu
│   │   └── ColumnHeader.tsx    # Sortable & resizable column headers
│   ├── NavigationToolbar/      # Back, Forward, Up, Root, Home, Refresh
│   ├── ConnectionDialog/       # Profile configuration modal
│   ├── ConnectionManager/      # Saved connections list, connect/disconnect
│   ├── ContextMenu/            # Right-click context actions
│   ├── PropertiesDialog/       # Permissions (chmod octal/matrix) and ownership
│   ├── OpenWithDialog/         # Installed app picker with default association toggle
│   ├── TransferQueue/          # Queue list, progress bars, pause/cancel controls
│   └── ElevatedBanner/         # Prominent warning badge for elevated SFTP sessions
├── hooks/                      # Custom React hooks (usePaneNavigation, useIpcEvent)
├── stores/                     # State stores (pane state, transfer store, settings)
├── services/                   # Typed Tauri IPC wrappers (invoke & listen)
└── types/                      # Domain interfaces matching Rust DTOs
```

---

## 2. Pane State & Table Interaction

### State Management
- Each pane (Local and Remote) maintains independent state:
  - Current working directory path
  - History stack: `history: string[]` and `historyIndex: number`
  - Selection: `Set<string>` (paths or filenames)
  - Sorting: `{ column: 'name' | 'size' | 'mtime' | 'rights' | 'owner', direction: 'asc' | 'desc' }`
  - Dotfiles visibility toggle (`showHidden: boolean`)
- Ensure navigation history maintains proper back/forward boundaries.

### Selection & Keyboard Controls
- **Single click:** Select single item.
- **Ctrl + click:** Toggle item in multi-selection.
- **Shift + click:** Range selection from last focused item.
- **Double click / Enter:** If directory, navigate into it; if file, trigger default Open action.
- **Backspace / Alt + Up:** Navigate to parent directory.
- **Delete / F8:** Confirm and delete selected items.
- **F5:** Refresh current pane.

### Column Formatting Rules
- Display default columns in order: **Name**, **Size**, **Changed**, **Rights**, **Owner**.
- Format file sizes with clear units (`B`, `KB`, `MB`, `GB`). Directories must display `—` unless explicitly calculated.
- Format permissions as Unix symbolic strings (`-rw-r--r--` or `drwxr-xr-x`).
- Truncate long filenames visually with CSS ellipsis without mutating the raw path.

---

## 3. Internal Drag-and-Drop Rules

### Scope Boundary
- **Internal Only:** Drag-and-drop is supported **only** between LinuxSCP's own local and remote panes.
- **Explicit Non-Goal:** Dragging files from external desktop applications or dropping onto the Linux desktop is explicitly out of scope.
- **Operations:**
  - Local pane -> Remote pane = Request Upload
  - Remote pane -> Local pane = Request Download

### Implementation Pattern
- Use standard HTML5 drag-and-drop or pointer events to track internal drag state.
- Drag data transfers only structured metadata (source paths, source pane ID):
  ```typescript
  interface DragPayload {
    sourcePane: 'local' | 'remote';
    sourcePaths: string[];
  }
  ```
- On drop, determine target directory (the pane's current directory or the hovered folder row).
- Prompt confirmation if configured, then call the Tauri backend transfer command.
- **Never attempt to stream or read file bytes in React.**

---

## 4. Elevated Mode (Root/Sudo) Visual Safety

When a remote connection is operating in elevated SFTP mode:
1. Display a prominent badge (`ELEVATED` or `ROOT`) in high-contrast styling (e.g., amber or red) in the status bar and remote pane header.
2. Display the effective identity (e.g., `root (via sudo)`) alongside the login user.
3. For destructive operations (delete, recursive permissions) in elevated mode, use distinct confirmation dialogs emphasizing the elevated risk.

---

## 5. Typed Tauri IPC Service Layer

All backend interaction must pass through typed service functions:

```typescript
// services/filesystem.ts
import { invoke } from '@tauri-apps/api/core';
import type { FileEntry, DirectoryListing } from '../types';

export async function listRemoteDirectory(sessionId: string, path: string): Promise<DirectoryListing> {
  return await invoke<DirectoryListing>('list_remote_directory', { sessionId, path });
}
```

Listen to backend events with cleanup on unmount:
```typescript
useEffect(() => {
  const unlisten = listen<TransferProgress>('transfer://progress', (event) => {
    updateTransferProgress(event.payload);
  });
  return () => {
    unlisten.then((fn) => fn());
  };
}, []);
```

---

## 6. Common Pitfalls to Avoid

- **No Mock / Fake Progress:** Do not use `setInterval` or fake timers to simulate file transfers. Hook into real backend events.
- **Scroll Position Flashes:** Maintain stable scroll and selection during background refreshes.
- **Untyped IPC Calls:** Never call `invoke('some_command')` without defining exact request arguments and response interfaces.
- **UI Freezes:** Never perform heavy data manipulation or recursive filtering synchronously in render loops.

---

## 7. Testing & Quality Standards

- Verify component rendering and interaction logic with Vitest and React Testing Library:
  ```bash
  npm test
  ```
- Run TypeScript type checking:
  ```bash
  npx tsc --noEmit
  ```
- Run linting:
  ```bash
  npm run lint
  ```
- Reference documentation: `docs/LinuxSCP_Product_and_Technical_Specification.md` Sections 4, 5, 10, and 11.

