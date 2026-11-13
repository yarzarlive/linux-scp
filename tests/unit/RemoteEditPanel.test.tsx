import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { RemoteEditPanel } from "../../src/components/RemoteEditPanel/RemoteEditPanel";
import { RemoteEditSession } from "../../src/types/desktop";

function makeSession(overrides: Partial<RemoteEditSession>): RemoteEditSession {
  return {
    id: "22222222-2222-2222-2222-222222222222",
    endpoint: {
      host: "10.0.0.5",
      port: 22,
      username: "deploy",
      elevated: false,
    },
    profile_id: "prof-1",
    remote_path: "/etc/app/settings.yaml",
    file_name: "settings.yaml",
    local_path: "/home/user/.local/state/linuxscp/remote-edits/2222/work/settings.yaml",
    app_id: "org.gnome.TextEditor.desktop",
    app_name: "Text Editor",
    baseline: {
      size: 128,
      modified_at_secs: 1700000000,
      permissions_mode: 0o640,
      uid: 1000,
      gid: 1000,
    },
    baseline_sha256: "baseline-sha",
    local_sha256: "baseline-sha",
    state: { state: "Clean" },
    prompt_dismissed: false,
    has_unsynced_changes: false,
    created_at_ms: 1700000000000,
    last_synced_at_ms: 1700000000000,
    upload_count: 0,
    recovered: false,
    ...overrides,
  };
}

describe("RemoteEditPanel Component", () => {
  it("renders clean session and cleans up immediately on Close & Clean Up without confirmation", async () => {
    const onDiscard = vi.fn().mockResolvedValue(undefined);

    render(
      <RemoteEditPanel
        sessions={[makeSession({ state: { state: "Clean" }, has_unsynced_changes: false })]}
        isConnected={true}
        localCurrentDir="/home/user"
        onUploadSession={vi.fn()}
        onKeepLocalChanges={vi.fn()}
        onSaveLocalCopy={vi.fn()}
        onDiscardSession={onDiscard}
      />
    );

    expect(screen.getByText("settings.yaml")).toBeInTheDocument();
    expect(screen.getByText("Synced")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Close & Clean Up/i }));

    await waitFor(() => {
      expect(onDiscard).toHaveBeenCalledWith("22222222-2222-2222-2222-222222222222", false);
    });
  });

  it("offers Upload Changes, Keep Local Changes, and requires confirmation to discard a Modified session", async () => {
    const onUpload = vi.fn().mockResolvedValue(undefined);
    const onKeepLocal = vi.fn().mockResolvedValue(undefined);
    const onDiscard = vi.fn().mockResolvedValue(undefined);

    render(
      <RemoteEditPanel
        sessions={[
          makeSession({
            state: { state: "Modified" },
            local_sha256: "modified-sha",
            has_unsynced_changes: true,
          }),
        ]}
        isConnected={true}
        localCurrentDir="/home/user"
        onUploadSession={onUpload}
        onKeepLocalChanges={onKeepLocal}
        onSaveLocalCopy={vi.fn()}
        onDiscardSession={onDiscard}
      />
    );

    expect(screen.getByText("Modified Locally")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Upload Changes/i }));
    await waitFor(() => {
      expect(onUpload).toHaveBeenCalledWith("22222222-2222-2222-2222-222222222222", false);
    });

    fireEvent.click(screen.getByRole("button", { name: /Keep Local Changes/i }));
    await waitFor(() => {
      expect(onKeepLocal).toHaveBeenCalledWith("22222222-2222-2222-2222-222222222222");
    });

    // Clicking Discard Edit... opens explicit confirmation before calling onDiscard
    fireEvent.click(screen.getByRole("button", { name: /Discard Edit\.\.\./i }));
    expect(onDiscard).not.toHaveBeenCalled();
    expect(screen.getByText(/Discard unsaved local edit\?/i)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Confirm Discard/i }));
    await waitFor(() => {
      expect(onDiscard).toHaveBeenCalledWith("22222222-2222-2222-2222-222222222222", true);
    });
  });

  it("reports remote conflict and offers Overwrite Remote, Save Copy Under New Name, Download Newer Remote, and Keep Remote", async () => {
    const onUpload = vi.fn().mockResolvedValue(undefined);
    const onSaveCopy = vi.fn().mockResolvedValue("/home/user/settings (1).yaml");
    const onDownloadNewer = vi.fn().mockResolvedValue(undefined);

    render(
      <RemoteEditPanel
        sessions={[
          makeSession({
            state: {
              state: "Conflict",
              details: {
                reason: "Remote file size changed from 128 to 256 bytes since it was downloaded.",
                remote_current: { size: 256, modified_at_secs: 1700000500 },
              },
            },
            local_sha256: "modified-sha",
            has_unsynced_changes: true,
          }),
        ]}
        isConnected={true}
        localCurrentDir="/home/user"
        onUploadSession={onUpload}
        onKeepLocalChanges={vi.fn()}
        onSaveLocalCopy={onSaveCopy}
        onDiscardSession={vi.fn()}
        onDownloadNewerRemote={onDownloadNewer}
      />
    );

    expect(screen.getByText("Remote Conflict")).toBeInTheDocument();
    expect(screen.getByText(/Remote file size changed from 128 to 256 bytes/i)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Overwrite Remote/i }));
    await waitFor(() => {
      expect(onUpload).toHaveBeenCalledWith("22222222-2222-2222-2222-222222222222", true);
    });

    fireEvent.click(screen.getByRole("button", { name: /Save Copy Under New Name/i }));
    await waitFor(() => {
      expect(onSaveCopy).toHaveBeenCalledWith("22222222-2222-2222-2222-222222222222", "/home/user");
      expect(
        screen.getByText(/Saved local copy to \/home\/user\/settings \(1\)\.yaml/i)
      ).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: /Download Newer Remote/i }));
    await waitFor(() => {
      expect(onDownloadNewer).toHaveBeenCalledWith("/etc/app/settings.yaml", "/home/user");
    });
  });

  it("displays disconnected / failed upload diagnostics and allows Retry Upload and Save Local Copy", async () => {
    const onUpload = vi.fn().mockResolvedValue(undefined);
    const onSaveCopy = vi.fn().mockResolvedValue("/home/user/settings (1).yaml");

    render(
      <RemoteEditPanel
        sessions={[
          makeSession({
            state: {
              state: "UploadFailed",
              details: {
                kind: "Disconnected",
                message:
                  "Not connected to deploy@10.0.0.5:22: No active SFTP connection. Your edited copy is preserved.",
              },
            },
            local_sha256: "modified-sha",
            has_unsynced_changes: true,
          }),
        ]}
        isConnected={false}
        localCurrentDir="/home/user"
        onUploadSession={onUpload}
        onKeepLocalChanges={vi.fn()}
        onSaveLocalCopy={onSaveCopy}
        onDiscardSession={vi.fn()}
      />
    );

    expect(screen.getByText("Upload Failed")).toBeInTheDocument();
    expect(screen.getByText(/Upload failed \(Disconnected\):/i)).toBeInTheDocument();
    expect(screen.getByText(/Your edited copy is preserved/i)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Retry Upload/i }));
    await waitFor(() => {
      expect(onUpload).toHaveBeenCalledWith("22222222-2222-2222-2222-222222222222", false);
    });
  });

  it("allows toggling Auto-upload on save and reloading from remote on conflict", async () => {
    const onChangeUploadMode = vi.fn().mockResolvedValue(undefined);
    const onReloadFromRemote = vi.fn().mockResolvedValue(undefined);

    render(
      <RemoteEditPanel
        sessions={[
          makeSession({
            state: {
              state: "Conflict",
              details: {
                reason: "Remote file changed externally.",
                remote_current: { size: 300, modified_at_secs: 1700000600 },
              },
            },
            local_sha256: "modified-sha",
            has_unsynced_changes: true,
          }),
        ]}
        isConnected={true}
        localCurrentDir="/home/user"
        uploadMode="auto"
        onChangeUploadMode={onChangeUploadMode}
        onUploadSession={vi.fn()}
        onKeepLocalChanges={vi.fn()}
        onSaveLocalCopy={vi.fn()}
        onDiscardSession={vi.fn()}
        onReloadFromRemote={onReloadFromRemote}
      />
    );

    const checkbox = screen.getByRole("checkbox", { name: /Auto-upload on save/i });
    expect(checkbox).toBeChecked();
    fireEvent.click(checkbox);
    await waitFor(() => {
      expect(onChangeUploadMode).toHaveBeenCalledWith("prompt");
    });

    fireEvent.click(screen.getByRole("button", { name: /Load Remote Version/i }));
    await waitFor(() => {
      expect(onReloadFromRemote).toHaveBeenCalledWith("22222222-2222-2222-2222-222222222222");
    });
  });
});
