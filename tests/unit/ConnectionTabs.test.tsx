import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { RemotePane } from "../../src/components/FilePanel/RemotePane";
import { RemoteTabState, createDefaultTabState } from "../../src/types/tabs";
import { FileEntry } from "../../src/types/filesystem";

vi.mock("../../src/services/ipc", () => ({
  IpcClient: {
    listRemoteDirectory: vi.fn(),
    checkRemotePathIsDir: vi.fn(),
    createRemoteDirectory: vi.fn(),
    createRemoteFile: vi.fn(),
    renameRemoteEntry: vi.fn(),
    deleteRemoteFile: vi.fn(),
    deleteRemoteDirectory: vi.fn(),
    setActiveSession: vi.fn().mockResolvedValue(undefined),
    disconnectSftpSession: vi.fn().mockResolvedValue(undefined),
    enqueueDownloads: vi.fn().mockResolvedValue(["job-1"]),
    enqueueUploads: vi.fn().mockResolvedValue(["job-2"]),
    resolveRemoteIdentity: vi.fn().mockResolvedValue({ user_name: "root", group_name: "root" }),
    setRemotePermissions: vi.fn().mockResolvedValue(undefined),
    setRemoteOwnership: vi.fn().mockResolvedValue(undefined),
  },
}));

const mockEntry1: FileEntry = {
  name: "index.html",
  path: "/var/www/index.html",
  file_type: { type: "Regular" },
  is_hidden: false,
  metadata: {
    size: 1024,
    modified_at_secs: 1772366400,
    permissions: {
      raw_mode: 0o644,
      user_read: true,
      user_write: true,
      user_execute: false,
      group_read: true,
      group_write: false,
      group_execute: false,
      other_read: true,
      other_write: false,
      other_execute: false,
      set_uid: false,
      set_gid: false,
      sticky: false,
    },
    ownership: { uid: 1000, user_name: "www-data", gid: 1000, group_name: "www-data" },
  },
};

const mockEntry2: FileEntry = {
  name: "nginx.conf",
  path: "/etc/nginx/nginx.conf",
  file_type: { type: "Regular" },
  is_hidden: false,
  metadata: {
    size: 2048,
    modified_at_secs: 1772366400,
    permissions: {
      raw_mode: 0o644,
      user_read: true,
      user_write: true,
      user_execute: false,
      group_read: true,
      group_write: false,
      group_execute: false,
      other_read: true,
      other_write: false,
      other_execute: false,
      set_uid: false,
      set_gid: false,
      sticky: false,
    },
    ownership: { uid: 0, user_name: "root", gid: 0, group_name: "root" },
  },
};

function createConnectedTab(id: string, title: string, sessionId: string, currentPath: string, entries: FileEntry[]): RemoteTabState {
  const tab = createDefaultTabState(id, title);
  tab.currentPath = currentPath;
  tab.parentPath = "/";
  tab.homePath = "/home/ubuntu";
  tab.entries = entries;
  tab.sessionState = {
    status: { status: "Connected" },
    session: {
      session_id: sessionId,
      protocol: "SFTP",
      username: "ubuntu",
      host: "192.168.1.100",
      port: 22,
      status: { status: "Connected" },
      elevation: { type: "None" },
      current_remote_path: currentPath,
      home_remote_path: "/home/ubuntu",
      capabilities: {
        protocol: "SFTP",
        can_list_directory: true,
        can_stat_metadata: true,
        can_upload: true,
        can_download: true,
        can_create_file: true,
        can_create_directory: true,
        can_rename: true,
        can_delete_file: true,
        can_delete_directory: true,
        can_set_permissions: true,
        can_set_owner_group: true,
        can_elevate_sudo: true,
        can_preserve_timestamps: true,
        can_resume_transfer: true,
        can_symlink: true,
      },
      last_active_ms: Date.now(),
    },
    trusted_host_key: null,
  };
  return tab;
}

describe("Connection Tabs Integration", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("maintains independent paths, entries, and selection states across tabs when switching", async () => {
    const tab1 = createConnectedTab("tab-1", "Web Server", "sess-web", "/var/www", [mockEntry1]);
    tab1.selectedPaths = new Set(["/var/www/index.html"]);
    tab1.focusedPath = "/var/www/index.html";

    const tab2 = createConnectedTab("tab-2", "Config Server", "sess-cfg", "/etc/nginx", [mockEntry2]);
    tab2.selectedPaths = new Set(["/etc/nginx/nginx.conf"]);
    tab2.focusedPath = "/etc/nginx/nginx.conf";

    const onSelectTab = vi.fn();
    const onUpdateTab = vi.fn();

    const { rerender } = render(
      <RemotePane
        tabs={[tab1, tab2]}
        activeTabId="tab-1"
        onSelectTab={onSelectTab}
        onUpdateTab={onUpdateTab}
      />
    );

    // Tab 1 is active: renders index.html from /var/www
    expect(screen.getByText("index.html")).toBeInTheDocument();
    expect(screen.queryByText("nginx.conf")).not.toBeInTheDocument();
    expect(screen.getByDisplayValue("/var/www")).toBeInTheDocument();

    // Click on Tab 2
    fireEvent.click(screen.getByText("Config Server"));
    expect(onSelectTab).toHaveBeenCalledWith("tab-2");

    // Re-render as if state switched activeTabId to "tab-2"
    rerender(
      <RemotePane
        tabs={[tab1, tab2]}
        activeTabId="tab-2"
        onSelectTab={onSelectTab}
        onUpdateTab={onUpdateTab}
      />
    );

    // Tab 2 is active: renders nginx.conf from /etc/nginx
    expect(screen.getByText("nginx.conf")).toBeInTheDocument();
    expect(screen.queryByText("index.html")).not.toBeInTheDocument();
    expect(screen.getByDisplayValue("/etc/nginx")).toBeInTheDocument();

    // Tab 1 state was untouched and retains its own independent path & selection
    expect(tab1.currentPath).toBe("/var/www");
    expect(tab1.selectedPaths.has("/var/www/index.html")).toBe(true);
  });

  it("isolates disconnect so disconnecting tab 1 does not affect tab 2", async () => {
    const tab1 = createConnectedTab("tab-1", "Tab 1", "sess-1", "/var/www", [mockEntry1]);
    const tab2 = createConnectedTab("tab-2", "Tab 2", "sess-2", "/etc/nginx", [mockEntry2]);

    // Now disconnect Tab 1 while Tab 2 remains connected
    const disconnectedTab1: RemoteTabState = {
      ...tab1,
      sessionState: {
        status: { status: "Disconnected" },
        session: null,
        trusted_host_key: null,
      },
      entries: [],
      selectedPaths: new Set(),
    };

    const { rerender } = render(
      <RemotePane
        tabs={[disconnectedTab1, tab2]}
        activeTabId="tab-1"
      />
    );

    // Tab 1 shows disconnected status
    expect(screen.getByText("No Remote Connection Established")).toBeInTheDocument();
    expect(screen.queryByText("index.html")).not.toBeInTheDocument();

    // Switch to Tab 2
    rerender(
      <RemotePane
        tabs={[disconnectedTab1, tab2]}
        activeTabId="tab-2"
      />
    );

    // Tab 2 is still fully connected and displays its entries
    expect(screen.getByText("nginx.conf")).toBeInTheDocument();
    expect(screen.getByDisplayValue("/etc/nginx")).toBeInTheDocument();
  });

  it("passes each tab's specific sessionId when initiating downloads or uploads", async () => {
    const tab1 = createConnectedTab("tab-1", "Tab 1", "sess-1", "/var/www", [mockEntry1]);
    tab1.selectedPaths = new Set(["/var/www/index.html"]);

    const tab2 = createConnectedTab("tab-2", "Tab 2", "sess-2", "/etc/nginx", [mockEntry2]);
    tab2.selectedPaths = new Set(["/etc/nginx/nginx.conf"]);

    const onDownload = vi.fn();
    const onDropUpload = vi.fn();

    const { rerender } = render(
      <RemotePane
        tabs={[tab1, tab2]}
        activeTabId="tab-1"
        onDownload={onDownload}
        onDropUpload={onDropUpload}
      />
    );

    // Download from Tab 1
    const downloadBtn = screen.getByTitle("Download Selected to Local Pane");
    fireEvent.click(downloadBtn);
    expect(onDownload).toHaveBeenCalledWith(
      ["/var/www/index.html"],
      undefined,
      "sess-1"
    );

    // Switch to Tab 2 and download
    rerender(
      <RemotePane
        tabs={[tab1, tab2]}
        activeTabId="tab-2"
        onDownload={onDownload}
        onDropUpload={onDropUpload}
      />
    );

    const downloadBtn2 = screen.getByTitle("Download Selected to Local Pane");
    fireEvent.click(downloadBtn2);
    expect(onDownload).toHaveBeenCalledWith(
      ["/etc/nginx/nginx.conf"],
      undefined,
      "sess-2"
    );
  });

  it("handles adding new tabs and renders all tabs in the tab strip", () => {
    const tab1 = createConnectedTab("tab-1", "Production Web", "sess-1", "/var/www", [mockEntry1]);
    const tab2 = createConnectedTab("tab-2", "Staging DB", "sess-2", "/var/lib/mysql", []);
    const tab3 = createDefaultTabState("tab-3", "Remote 3");

    const onSelectTab = vi.fn();
    const onCloseTab = vi.fn();
    const onAddTab = vi.fn();

    render(
      <RemotePane
        tabs={[tab1, tab2, tab3]}
        activeTabId="tab-2"
        onSelectTab={onSelectTab}
        onCloseTab={onCloseTab}
        onAddTab={onAddTab}
      />
    );

    const renderedTabs = screen.getAllByRole("tab");
    expect(renderedTabs).toHaveLength(3);
    expect(screen.getByText("Production Web")).toBeInTheDocument();
    expect(screen.getByText("Staging DB")).toBeInTheDocument();
    expect(screen.getByText("Remote 3")).toBeInTheDocument();

    // Tab 2 is active
    expect(renderedTabs[1]).toHaveAttribute("aria-selected", "true");
    expect(renderedTabs[0]).toHaveAttribute("aria-selected", "false");

    // Close button appears for tabs when > 1 tab exists
    const closeBtn = screen.getByLabelText("Close tab Production Web");
    fireEvent.click(closeBtn);
    expect(onCloseTab).toHaveBeenCalledWith("tab-1");
  });
});
