import { useState } from "react";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { LocalPane } from "../../src/components/FilePanel/LocalPane";
import { RemotePane } from "../../src/components/FilePanel/RemotePane";
import { IpcClient } from "../../src/services/ipc";
import { SessionStateSnapshot } from "../../src/types/session";
import { INTERNAL_DRAG_MIME, InternalDragPayload } from "../../src/utils/dragDrop";

vi.mock("../../src/services/ipc", () => ({
  IpcClient: {
    getHomeDirectory: vi.fn(),
    listLocalDirectory: vi.fn(),
    listRemoteDirectory: vi.fn(),
    checkRemotePathIsDir: vi.fn(),
    enqueueUploads: vi.fn(),
    enqueueDownloads: vi.fn(),
  },
}));

const CONNECTED_SESSION: SessionStateSnapshot = {
  status: { status: "Connected" },
  session: {
    session_id: "sess-1",
    profile_id: "prof-1",
    protocol: "SFTP",
    host: "192.168.122.10",
    port: 22,
    username: "ubuntu",
    status: { status: "Connected" },
    elevation: { type: "None" },
    current_remote_path: "/remote/dest",
    home_remote_path: "/remote/dest",
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
    host: "192.168.122.10",
    port: 22,
    algorithm: "ssh-ed25519",
    fingerprint_sha256: "SHA256:test",
    public_key_openssh: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI...",
    trusted_at_ms: 1700000000000,
  },
};

function DualPaneHarness({
  onUpload,
  onDownload,
}: {
  onUpload: (paths: string[], targetDir?: string) => void;
  onDownload: (paths: string[], targetDir?: string) => void;
}) {
  const [activeDrag, setActiveDrag] = useState<InternalDragPayload | null>(null);
  return (
    <div>
      <LocalPane
        initialPath="/local/src"
        canUpload={true}
        onUpload={onUpload}
        onDropDownload={onDownload}
        activeDrag={activeDrag}
        onInternalDragStart={setActiveDrag}
        onInternalDragEnd={() => setActiveDrag(null)}
      />
      <RemotePane
        sessionState={CONNECTED_SESSION}
        onDownload={onDownload}
        onDropUpload={onUpload}
        activeDrag={activeDrag}
        onInternalDragStart={setActiveDrag}
        onInternalDragEnd={() => setActiveDrag(null)}
      />
    </div>
  );
}

function createMockDataTransfer() {
  const store: Record<string, string> = {};
  return {
    effectAllowed: "none",
    dropEffect: "none",
    files: [] as File[],
    get types() {
      return Object.keys(store);
    },
    setData: vi.fn((type: string, val: string) => {
      store[type] = val;
    }),
    getData: vi.fn((type: string) => store[type] ?? ""),
  };
}

describe("Internal two-pane drag-and-drop", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(IpcClient.getHomeDirectory).mockResolvedValue("/local/src");
    vi.mocked(IpcClient.listLocalDirectory).mockResolvedValue({
      current_path: "/local/src",
      parent_path: "/local",
      entries: [
        {
          name: "alpha.txt",
          path: "/local/src/alpha.txt",
          file_type: { type: "Regular" },
          metadata: { size: 100, modified_at_secs: 1700000000 },
          is_hidden: false,
        },
        {
          name: "beta.txt",
          path: "/local/src/beta.txt",
          file_type: { type: "Regular" },
          metadata: { size: 200, modified_at_secs: 1700000000 },
          is_hidden: false,
        },
        {
          name: "local-sub",
          path: "/local/src/local-sub",
          file_type: { type: "Directory" },
          metadata: { size: undefined, modified_at_secs: 1700000000 },
          is_hidden: false,
        },
      ],
    });
    vi.mocked(IpcClient.listRemoteDirectory).mockResolvedValue({
      current_path: "/remote/dest",
      parent_path: "/remote",
      entries: [
        {
          name: "gamma.log",
          path: "/remote/dest/gamma.log",
          file_type: { type: "Regular" },
          metadata: { size: 300, modified_at_secs: 1700000000 },
          is_hidden: false,
        },
        {
          name: "remote-sub",
          path: "/remote/dest/remote-sub",
          file_type: { type: "Directory" },
          metadata: { size: undefined, modified_at_secs: 1700000000 },
          is_hidden: false,
        },
      ],
    });
  });

  it("uploads multi-selected local files when dropped onto the remote pane and does not expose external drag MIME types", async () => {
    const onUpload = vi.fn();
    const onDownload = vi.fn();
    render(<DualPaneHarness onUpload={onUpload} onDownload={onDownload} />);

    await waitFor(() => {
      expect(screen.getByText("alpha.txt")).toBeInTheDocument();
      expect(screen.getByText("gamma.log")).toBeInTheDocument();
    });

    // Multi-select alpha.txt and beta.txt
    fireEvent.click(screen.getByText("alpha.txt"));
    fireEvent.click(screen.getByText("beta.txt"), { ctrlKey: true });

    const dt = createMockDataTransfer();
    const alphaRow = screen.getByText("alpha.txt").closest('[role="row"]')!;
    fireEvent.dragStart(alphaRow, { dataTransfer: dt });

    // Verify no external desktop MIME types (text/plain, text/uri-list) were set
    expect(dt.setData).toHaveBeenCalledWith(INTERNAL_DRAG_MIME, expect.any(String));
    expect(dt.setData).not.toHaveBeenCalledWith("text/plain", expect.anything());
    expect(dt.setData).not.toHaveBeenCalledWith("text/uri-list", expect.anything());

    const remotePane = screen.getByRole("region", { name: "Remote Filesystem" });
    fireEvent.dragOver(remotePane, { dataTransfer: dt });

    expect(screen.getByTestId("pane-drop-indicator")).toHaveTextContent(
      /Upload 2 items\s*→\s*\/remote\/dest/
    );

    fireEvent.drop(remotePane, { dataTransfer: dt });
    expect(onUpload).toHaveBeenCalledTimes(1);
    expect(onUpload).toHaveBeenCalledWith(
      ["/local/src/alpha.txt", "/local/src/beta.txt"],
      "/remote/dest"
    );
    expect(onDownload).not.toHaveBeenCalled();
  });

  it("downloads remote file into a hovered local subfolder while preserving pane directory", async () => {
    const onUpload = vi.fn();
    const onDownload = vi.fn();
    render(<DualPaneHarness onUpload={onUpload} onDownload={onDownload} />);

    await waitFor(() => {
      expect(screen.getByText("gamma.log")).toBeInTheDocument();
      expect(screen.getByText("local-sub")).toBeInTheDocument();
    });

    const dt = createMockDataTransfer();
    const gammaRow = screen.getByText("gamma.log").closest('[role="row"]')!;
    fireEvent.dragStart(gammaRow, { dataTransfer: dt });

    const localSubRow = screen.getByText("local-sub").closest('[role="row"]')!;
    fireEvent.dragOver(localSubRow, { dataTransfer: dt });

    expect(screen.getByTestId("pane-drop-indicator")).toHaveTextContent(
      /Download 1 item\s*→\s*\/local\/src\/local-sub/
    );

    fireEvent.drop(localSubRow, { dataTransfer: dt });
    expect(onDownload).toHaveBeenCalledTimes(1);
    expect(onDownload).toHaveBeenCalledWith(["/remote/dest/gamma.log"], "/local/src/local-sub");
    expect(onUpload).not.toHaveBeenCalled();
  });

  it("treats dropping onto a non-directory file row as dropping into the currently opened pane directory, while rejecting same-pane and external drops", async () => {
    const onUpload = vi.fn();
    const onDownload = vi.fn();
    render(<DualPaneHarness onUpload={onUpload} onDownload={onDownload} />);

    await waitFor(() => {
      expect(screen.getByText("alpha.txt")).toBeInTheDocument();
      expect(screen.getByText("gamma.log")).toBeInTheDocument();
    });

    // 1. Same-pane drop (local -> local)
    const dt1 = createMockDataTransfer();
    const alphaRow = screen.getByText("alpha.txt").closest('[role="row"]')!;
    fireEvent.dragStart(alphaRow, { dataTransfer: dt1 });

    const localSubRow = screen.getByText("local-sub").closest('[role="row"]')!;
    fireEvent.dragOver(localSubRow, { dataTransfer: dt1 });
    expect(screen.getByTestId("pane-drop-rejected")).toHaveTextContent(
      "Cannot drop items into the same pane."
    );
    fireEvent.drop(localSubRow, { dataTransfer: dt1 });
    expect(onUpload).not.toHaveBeenCalled();
    expect(onDownload).not.toHaveBeenCalled();

    // 2. Drop onto non-directory file row (gamma.log in remote pane) transfers into currently opened remote directory (/remote/dest)
    const dt2 = createMockDataTransfer();
    fireEvent.dragStart(alphaRow, { dataTransfer: dt2 });
    const gammaRow = screen.getByText("gamma.log").closest('[role="row"]')!;
    fireEvent.dragOver(gammaRow, { dataTransfer: dt2 });
    expect(screen.getByTestId("pane-drop-indicator")).toHaveTextContent(
      /Upload 1 item\s*→\s*\/remote\/dest/
    );
    fireEvent.drop(gammaRow, { dataTransfer: dt2 });
    expect(onUpload).toHaveBeenCalledTimes(1);
    expect(onUpload).toHaveBeenCalledWith(["/local/src/alpha.txt"], "/remote/dest");
    expect(onDownload).not.toHaveBeenCalled();

    // 3. External desktop file drop (no internal drag active, Files present)
    const externalDt = {
      effectAllowed: "all",
      dropEffect: "none",
      files: [new File(["hello"], "outside.txt", { type: "text/plain" })],
      types: ["Files"],
      setData: vi.fn(),
      getData: vi.fn(() => ""),
    };
    const remotePane = screen.getByRole("region", { name: "Remote Filesystem" });
    fireEvent.dragOver(remotePane, { dataTransfer: externalDt });
    expect(screen.queryByTestId("pane-drop-indicator")).not.toBeInTheDocument();
    fireEvent.drop(remotePane, { dataTransfer: externalDt });
    expect(onUpload).toHaveBeenCalledTimes(1);
    expect(onDownload).not.toHaveBeenCalled();
  });

  it("does not trigger transfers on ordinary selection clicks", async () => {
    const onUpload = vi.fn();
    const onDownload = vi.fn();
    render(<DualPaneHarness onUpload={onUpload} onDownload={onDownload} />);

    await waitFor(() => {
      expect(screen.getByText("alpha.txt")).toBeInTheDocument();
      expect(screen.getByText("gamma.log")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("alpha.txt"));
    fireEvent.click(screen.getByText("gamma.log"));
    expect(onUpload).not.toHaveBeenCalled();
    expect(onDownload).not.toHaveBeenCalled();
  });
});
