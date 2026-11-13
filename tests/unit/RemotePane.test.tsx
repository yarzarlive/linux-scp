import { render, screen, fireEvent, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { RemotePane } from "../../src/components/FilePanel/RemotePane";
import { SessionStateSnapshot } from "../../src/types/session";
import { IpcClient } from "../../src/services/ipc";
import { RemoteDirectoryListing } from "../../src/types/filesystem";

vi.mock("../../src/services/ipc", () => ({
  IpcClient: {
    listRemoteDirectory: vi.fn(),
    checkRemotePathIsDir: vi.fn(),
    createRemoteDirectory: vi.fn(),
    createRemoteFile: vi.fn(),
    renameRemoteEntry: vi.fn(),
    deleteRemoteFile: vi.fn(),
    deleteRemoteDirectory: vi.fn(),
    openRemoteFileDefault: vi.fn(),
    openRemoteFileWithApp: vi.fn(),
    discoverDesktopApplications: vi.fn(),
  },
}));

function makeConnectedState(overrides?: {
  session_id?: string;
  profile_id?: string;
  host?: string;
  current_remote_path?: string;
  home_remote_path?: string;
}): SessionStateSnapshot {
  return {
    status: { status: "Connected" },
    session: {
      session_id: overrides?.session_id ?? "sess-123",
      profile_id: overrides?.profile_id ?? "prof-1",
      protocol: "SFTP",
      host: overrides?.host ?? "192.168.122.10",
      port: 22,
      username: "ubuntu",
      status: { status: "Connected" },
      elevation: { type: "None" },
      current_remote_path: overrides?.current_remote_path ?? "/home/ubuntu",
      home_remote_path: overrides?.home_remote_path ?? "/home/ubuntu",
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
      last_active_ms: 1700000000000,
    },
    trusted_host_key: {
      host: overrides?.host ?? "192.168.122.10",
      port: 22,
      algorithm: "ssh-ed25519",
      fingerprint_sha256: "SHA256:VerifiedKey12345",
      public_key_openssh: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI...",
      trusted_at_ms: 1700000000000,
    },
  };
}

function makeListing(current_path: string): RemoteDirectoryListing {
  return {
    current_path,
    parent_path: current_path === "/" ? null : "/home",
    home_path: "/home/ubuntu",
    entries: [
      {
        name: "docs",
        path: `${current_path === "/" ? "" : current_path}/docs`,
        file_type: { type: "Directory" },
        metadata: { size: undefined, modified_at_secs: 1700000000 },
        is_hidden: false,
      },
    ],
  };
}

describe("RemotePane Component", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders explicit honest disconnected state with no fabricated files", () => {
    render(<RemotePane />);

    expect(screen.getAllByText("Disconnected").length).toBeGreaterThan(0);
    expect(screen.getByText("No Remote Connection Established")).toBeInTheDocument();

    const addressInput = screen.getByLabelText("Path location") as HTMLInputElement;
    expect(addressInput.value).toBe("sftp://[not connected]/");
    expect(addressInput).toBeDisabled();

    expect(screen.getByTitle("Sort by Name")).toBeDisabled();
    expect(screen.getByTitle("Sort by Size")).toBeDisabled();
    expect(screen.getByTitle("Sort by Changed")).toBeDisabled();
    expect(screen.getByTitle("Sort by Rights")).toBeDisabled();
    expect(screen.getByTitle("Sort by Owner")).toBeDisabled();
  });

  it("calls onConnect when New Connection button is clicked", () => {
    const handleConnect = vi.fn();
    render(<RemotePane onConnect={handleConnect} />);

    fireEvent.click(screen.getByRole("button", { name: /New Connection.../i }));
    expect(handleConnect).toHaveBeenCalledTimes(1);
  });

  it("renders connecting state with cancel action", () => {
    const handleDisconnect = vi.fn();
    const connectingState: SessionStateSnapshot = {
      status: { status: "Connecting" },
      session: null,
      trusted_host_key: null,
    };

    render(<RemotePane sessionState={connectingState} onDisconnect={handleDisconnect} />);

    expect(screen.getByText("Connecting")).toBeInTheDocument();
    expect(screen.getByText("Establishing SFTP Session...")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Cancel Connection/i }));
    expect(handleDisconnect).toHaveBeenCalledTimes(1);
  });

  it("renders failed state with actionable error message and retry button", () => {
    const handleReconnect = vi.fn();
    const failedState: SessionStateSnapshot = {
      status: {
        status: "Failed",
        details: { error_message: "Authentication failed for user ubuntu" },
      },
      session: null,
      trusted_host_key: null,
    };

    render(<RemotePane sessionState={failedState} onReconnect={handleReconnect} />);

    expect(screen.getByText("Failed")).toBeInTheDocument();
    expect(screen.getByText("Remote SFTP Connection Failed")).toBeInTheDocument();
    expect(screen.getByText("Authentication failed for user ubuntu")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Retry Connection/i }));
    expect(handleReconnect).toHaveBeenCalledTimes(1);
  });

  it("loads remote directory listing on connect and renders authentic metadata without inventing missing fields", async () => {
    const listing: RemoteDirectoryListing = {
      current_path: "/home/ubuntu",
      parent_path: "/home",
      home_path: "/home/ubuntu",
      entries: [
        {
          name: "My Docs 日本語",
          path: "/home/ubuntu/My Docs 日本語",
          file_type: { type: "Directory" },
          metadata: {
            size: undefined,
            modified_at_secs: 1700000100,
            permissions: {
              raw_mode: 0o040755,
              user_read: true,
              user_write: true,
              user_execute: true,
              group_read: true,
              group_write: false,
              group_execute: true,
              other_read: true,
              other_write: false,
              other_execute: true,
              set_uid: false,
              set_gid: false,
              sticky: false,
            },
            ownership: { uid: 1000, gid: 1000 },
          },
          is_hidden: false,
        },
        {
          name: "sparse.dat",
          path: "/home/ubuntu/sparse.dat",
          file_type: { type: "Regular" },
          metadata: {
            size: 2048,
            modified_at_secs: undefined,
            permissions: undefined,
            ownership: undefined,
          },
          is_hidden: false,
        },
      ],
    };

    vi.mocked(IpcClient.listRemoteDirectory).mockResolvedValueOnce(listing);

    const handleDisconnect = vi.fn();
    const handleReconnect = vi.fn();

    render(
      <RemotePane
        sessionState={makeConnectedState()}
        onDisconnect={handleDisconnect}
        onReconnect={handleReconnect}
      />
    );

    await waitFor(() => {
      expect(screen.getByText("My Docs 日本語")).toBeInTheDocument();
      expect(screen.getByText("sparse.dat")).toBeInTheDocument();
    });

    // Numeric UID/GID shown when names are unavailable
    expect(screen.getByText("1000:1000")).toBeInTheDocument();
    // Size formatted
    expect(screen.getByText("2.0 KB")).toBeInTheDocument();
    // Missing metadata renders as dash "—" rather than invented values
    expect(screen.getAllByText("—").length).toBeGreaterThanOrEqual(3);

    expect(screen.getByText("2 items")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Reconnect/i }));
    expect(handleReconnect).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: /Disconnect/i }));
    expect(handleDisconnect).toHaveBeenCalledTimes(1);
  });

  it("renders empty directory state", async () => {
    vi.mocked(IpcClient.listRemoteDirectory).mockResolvedValueOnce({
      current_path: "/home/ubuntu/empty",
      parent_path: "/home/ubuntu",
      home_path: "/home/ubuntu",
      entries: [],
    });

    render(
      <RemotePane
        sessionState={makeConnectedState({ current_remote_path: "/home/ubuntu/empty" })}
      />
    );

    await waitFor(() => {
      expect(screen.getByText("Directory is empty")).toBeInTheDocument();
    });
    expect(screen.getByText("0 items")).toBeInTheDocument();
  });

  it("renders permission-denied state and allows going back/up", async () => {
    vi.mocked(IpcClient.listRemoteDirectory)
      .mockResolvedValueOnce({
        current_path: "/home/ubuntu",
        parent_path: "/home",
        home_path: "/home/ubuntu",
        entries: [],
      })
      .mockRejectedValueOnce({
        kind: "PermissionDenied",
        message: "Permission denied accessing '/root': Permission denied",
      });

    render(<RemotePane sessionState={makeConnectedState()} />);

    await waitFor(() => {
      expect(IpcClient.listRemoteDirectory).toHaveBeenCalledTimes(1);
    });

    const addressInput = screen.getByLabelText("Path location") as HTMLInputElement;
    fireEvent.change(addressInput, { target: { value: "/root" } });
    fireEvent.submit(addressInput.closest("form")!);

    await waitFor(() => {
      expect(screen.getByText("Permission Denied")).toBeInTheDocument();
      expect(
        screen.getByText("Permission denied accessing '/root': Permission denied")
      ).toBeInTheDocument();
    });
  });

  it("renders network-error state with Retry and Reconnect buttons", async () => {
    vi.mocked(IpcClient.listRemoteDirectory).mockRejectedValueOnce({
      kind: "Network",
      message: "SFTP request timed out while accessing '/home/ubuntu'",
    });

    const handleReconnect = vi.fn();
    render(<RemotePane sessionState={makeConnectedState()} onReconnect={handleReconnect} />);

    await waitFor(() => {
      expect(screen.getByText("Remote Network Error")).toBeInTheDocument();
      expect(
        screen.getByText("SFTP request timed out while accessing '/home/ubuntu'")
      ).toBeInTheDocument();
    });

    const reconnectButtons = screen.getAllByRole("button", { name: /Reconnect/i });
    fireEvent.click(reconnectButtons[reconnectButtons.length - 1]);
    expect(handleReconnect).toHaveBeenCalledTimes(1);
  });

  it("supports Back, Forward, Parent, Root, Home, Refresh, Hidden toggle, and per-connection history", async () => {
    vi.mocked(IpcClient.listRemoteDirectory)
      .mockResolvedValueOnce({
        current_path: "/home/ubuntu",
        parent_path: "/home",
        home_path: "/home/ubuntu",
        entries: [],
      })
      .mockResolvedValueOnce({
        current_path: "/var/log",
        parent_path: "/var",
        home_path: "/home/ubuntu",
        entries: [],
      })
      .mockResolvedValueOnce({
        current_path: "/home/ubuntu",
        parent_path: "/home",
        home_path: "/home/ubuntu",
        entries: [],
      })
      .mockResolvedValueOnce({
        current_path: "/var/log",
        parent_path: "/var",
        home_path: "/home/ubuntu",
        entries: [],
      })
      .mockResolvedValueOnce({
        current_path: "/",
        parent_path: null,
        home_path: "/home/ubuntu",
        entries: [],
      });

    const { rerender } = render(<RemotePane sessionState={makeConnectedState()} />);

    await waitFor(() => {
      expect(IpcClient.listRemoteDirectory).toHaveBeenCalledWith("/home/ubuntu", false);
    });

    // Navigate via editable path bar to /var/log
    const addressInput = screen.getByLabelText("Path location") as HTMLInputElement;
    fireEvent.change(addressInput, { target: { value: "/var/log" } });
    fireEvent.submit(addressInput.closest("form")!);

    await waitFor(() => {
      expect(IpcClient.listRemoteDirectory).toHaveBeenCalledWith("/var/log", false);
    });

    // Click Back -> /home/ubuntu
    fireEvent.click(screen.getByTitle("Back (Alt+Left)"));
    await waitFor(() => {
      expect(IpcClient.listRemoteDirectory).toHaveBeenLastCalledWith("/home/ubuntu", false);
    });

    // Click Forward -> /var/log
    fireEvent.click(screen.getByTitle("Forward (Alt+Right)"));
    await waitFor(() => {
      expect(IpcClient.listRemoteDirectory).toHaveBeenLastCalledWith("/var/log", false);
    });

    // Click Root /
    fireEvent.click(screen.getByTitle("Root Directory (/)"));
    await waitFor(() => {
      expect(IpcClient.listRemoteDirectory).toHaveBeenLastCalledWith("/", false);
    });

    // Switch to a different connection profile -> its Back button must be disabled (isolated history)
    vi.mocked(IpcClient.listRemoteDirectory).mockResolvedValueOnce({
      current_path: "/opt/server2",
      parent_path: "/opt",
      home_path: "/home/deploy",
      entries: [],
    });

    rerender(
      <RemotePane
        sessionState={makeConnectedState({
          session_id: "sess-999",
          profile_id: "prof-2",
          host: "10.0.0.2",
          current_remote_path: "/opt/server2",
          home_remote_path: "/home/deploy",
        })}
      />
    );

    await waitFor(() => {
      expect(IpcClient.listRemoteDirectory).toHaveBeenLastCalledWith("/opt/server2", false);
    });
    expect(screen.getByTitle("Back (Alt+Left)")).toBeDisabled();
  });

  it("hides unsupported remote operations based on session capabilities", async () => {
    vi.mocked(IpcClient.listRemoteDirectory).mockResolvedValueOnce(makeListing("/home/ubuntu"));

    const restricted = makeConnectedState();
    restricted.session!.capabilities.can_create_file = false;
    restricted.session!.capabilities.can_rename = false;

    render(<RemotePane sessionState={restricted} />);

    await waitFor(() => {
      expect(screen.getByText("docs")).toBeInTheDocument();
    });

    expect(screen.getByTitle("New Remote Folder (F7)")).toBeInTheDocument();
    expect(screen.queryByTitle("New Remote File")).not.toBeInTheDocument();
    expect(screen.queryByTitle("Rename Selected Remote Item (F2)")).not.toBeInTheDocument();
  });

  it("creates remote directory, waits for confirmation, and refreshes listing", async () => {
    vi.mocked(IpcClient.listRemoteDirectory)
      .mockResolvedValueOnce(makeListing("/home/ubuntu"))
      .mockResolvedValueOnce(makeListing("/home/ubuntu"));
    vi.mocked(IpcClient.createRemoteDirectory).mockResolvedValueOnce("/home/ubuntu/new_dir");

    render(<RemotePane sessionState={makeConnectedState()} />);
    await waitFor(() => {
      expect(screen.getByText("docs")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTitle("New Remote Folder (F7)"));
    const input = screen.getByPlaceholderText("Folder name");
    fireEvent.change(input, { target: { value: "new_dir" } });
    fireEvent.click(screen.getByRole("button", { name: "Create Folder" }));

    await waitFor(() => {
      expect(IpcClient.createRemoteDirectory).toHaveBeenCalledWith("/home/ubuntu", "new_dir");
      expect(IpcClient.listRemoteDirectory).toHaveBeenCalledTimes(2);
    });
  });

  it("displays server error on failed create without claiming success or refreshing", async () => {
    vi.mocked(IpcClient.listRemoteDirectory).mockResolvedValueOnce(makeListing("/home/ubuntu"));
    vi.mocked(IpcClient.createRemoteDirectory).mockRejectedValueOnce({
      kind: "PermissionDenied",
      message: "Permission denied accessing '/home/ubuntu/blocked'",
    });

    render(<RemotePane sessionState={makeConnectedState()} />);
    await waitFor(() => {
      expect(screen.getByText("docs")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTitle("New Remote Folder (F7)"));
    fireEvent.change(screen.getByPlaceholderText("Folder name"), { target: { value: "blocked" } });
    fireEvent.click(screen.getByRole("button", { name: "Create Folder" }));

    await waitFor(() => {
      expect(
        screen.getByText("Permission denied accessing '/home/ubuntu/blocked'")
      ).toBeInTheDocument();
    });
    expect(IpcClient.listRemoteDirectory).toHaveBeenCalledTimes(1);
  });

  it("requires explicit recursive confirmation when deleting a non-empty remote directory", async () => {
    vi.mocked(IpcClient.listRemoteDirectory)
      .mockResolvedValueOnce(makeListing("/home/ubuntu"))
      .mockResolvedValueOnce(makeListing("/home/ubuntu"));
    vi.mocked(IpcClient.deleteRemoteDirectory)
      .mockRejectedValueOnce({
        kind: "Filesystem",
        message:
          "Remote directory '/home/ubuntu/docs' is not empty. Recursive deletion requires explicit confirmation.",
      })
      .mockResolvedValueOnce(undefined);

    render(<RemotePane sessionState={makeConnectedState()} />);
    await waitFor(() => {
      expect(screen.getByText("docs")).toBeInTheDocument();
    });

    // Select directory "docs" and click Delete
    fireEvent.click(screen.getByText("docs"));
    fireEvent.click(screen.getByTitle("Delete Selected Remote Items (Del)"));

    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText("Confirm Remote Deletion")).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: "Delete" }));

    // First attempt must be non-recursive (recursive=false) and NOT silently retry
    await waitFor(() => {
      expect(IpcClient.deleteRemoteDirectory).toHaveBeenCalledTimes(1);
      expect(IpcClient.deleteRemoteDirectory).toHaveBeenCalledWith("/home/ubuntu/docs", false);
    });

    // Explicit recursive confirmation dialog opens with target path displayed
    await waitFor(() => {
      expect(screen.getByText("Confirm Recursive Directory Deletion")).toBeInTheDocument();
      expect(screen.getByText("/home/ubuntu/docs")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "Delete Recursively" }));

    await waitFor(() => {
      expect(IpcClient.deleteRemoteDirectory).toHaveBeenCalledTimes(2);
      expect(IpcClient.deleteRemoteDirectory).toHaveBeenLastCalledWith("/home/ubuntu/docs", true);
      expect(IpcClient.listRemoteDirectory).toHaveBeenCalledTimes(2);
    });
  });

  it("opens remote file via remembered default or falls back to Open With picker", async () => {
    vi.mocked(IpcClient.listRemoteDirectory).mockResolvedValueOnce({
      current_path: "/home/ubuntu",
      parent_path: "/home",
      home_path: "/home/ubuntu",
      entries: [
        {
          name: "nginx.conf",
          path: "/home/ubuntu/nginx.conf",
          file_type: { type: "Regular" },
          metadata: { size: 512, modified_at_secs: 1700000100 },
          is_hidden: false,
        },
      ],
    });
    vi.mocked(IpcClient.openRemoteFileDefault).mockResolvedValueOnce({
      outcome: "NeedsAppPicker",
      extension_key: ".conf",
      extension_display: ".conf",
      missing_remembered_app_id: null,
    });
    vi.mocked(IpcClient.discoverDesktopApplications).mockResolvedValueOnce({
      extension_key: ".conf",
      extension_display: ".conf",
      remembered_app_id: null,
      remembered_app_installed: false,
      recommended_app_ids: ["org.gnome.TextEditor.desktop"],
      applications: [
        {
          id: "org.gnome.TextEditor.desktop",
          name: "Text Editor",
          comment: "Edit text files",
          icon: null,
          exec_raw: "gnome-text-editor %U",
          executable: "/usr/bin/gnome-text-editor",
          args_template: ["%U"],
          mime_types: ["text/plain"],
          categories: ["Utility"],
          terminal: false,
          desktop_file_path: "/usr/share/applications/org.gnome.TextEditor.desktop",
        },
      ],
    });

    render(<RemotePane sessionState={makeConnectedState()} />);
    await waitFor(() => {
      expect(screen.getByText("nginx.conf")).toBeInTheDocument();
    });

    fireEvent.doubleClick(screen.getByText("nginx.conf"));

    await waitFor(() => {
      expect(IpcClient.openRemoteFileDefault).toHaveBeenCalledWith("/home/ubuntu/nginx.conf");
      expect(screen.getByText("Open With — Choose Application")).toBeInTheDocument();
      expect(screen.getByText("Text Editor")).toBeInTheDocument();
    });
  });
});
