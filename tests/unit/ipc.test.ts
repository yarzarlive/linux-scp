import { describe, it, expect, vi, beforeEach } from "vitest";
import { IpcClient } from "../../src/services/ipc";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("IpcClient", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("fetches system information successfully", async () => {
    const mockInfo = {
      app_name: "linuxscp",
      app_version: "0.1.0",
      os_name: "linux",
      os_family: "unix",
      arch: "x86_64",
    };

    vi.mocked(invoke).mockResolvedValueOnce(mockInfo);

    const result = await IpcClient.getSystemInfo();
    expect(result).toEqual(mockInfo);
    expect(invoke).toHaveBeenCalledWith("get_system_info", undefined);
  });

  it("sends ping to backend with typed parameters", async () => {
    const mockResponse = {
      status: "ACK: ping-test",
      timestamp_ms: 1700000000,
    };

    vi.mocked(invoke).mockResolvedValueOnce(mockResponse);

    const result = await IpcClient.pingBackend("ping-test");
    expect(result).toEqual(mockResponse);
    expect(invoke).toHaveBeenCalledWith("ping_backend", { payload: "ping-test" });
  });

  it("translates structured backend errors cleanly", async () => {
    const backendError = {
      kind: "Validation",
      message: "Payload cannot be empty",
    };

    vi.mocked(invoke).mockRejectedValueOnce(backendError);

    await expect(IpcClient.pingBackend("")).rejects.toEqual(backendError);
  });

  it("invokes remote Open With and remote edit lifecycle commands with typed arguments", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      outcome: "NeedsAppPicker",
      extension_key: ".conf",
      extension_display: ".conf",
    });
    await IpcClient.openRemoteFileDefault("/etc/nginx/nginx.conf");
    expect(invoke).toHaveBeenCalledWith("open_remote_file_default", {
      remotePath: "/etc/nginx/nginx.conf",
    });

    vi.mocked(invoke).mockResolvedValueOnce([]);
    await IpcClient.listRemoteEditSessions();
    expect(invoke).toHaveBeenCalledWith("list_remote_edit_sessions", undefined);

    vi.mocked(invoke).mockResolvedValueOnce({ outcome: "Unchanged", session: {} });
    await IpcClient.uploadRemoteEditSession("sess-1", true);
    expect(invoke).toHaveBeenCalledWith("upload_remote_edit_session", {
      sessionId: "sess-1",
      overwriteRemoteChanges: true,
    });

    vi.mocked(invoke).mockResolvedValueOnce(undefined);
    await IpcClient.discardRemoteEditSession("sess-1", false);
    expect(invoke).toHaveBeenCalledWith("discard_remote_edit_session", {
      sessionId: "sess-1",
      discardLocalChanges: false,
    });

    vi.mocked(invoke).mockResolvedValueOnce({});
    await IpcClient.reloadRemoteEditFromRemote("sess-1", true);
    expect(invoke).toHaveBeenCalledWith("reload_remote_edit_from_remote", {
      sessionId: "sess-1",
      relaunchApp: true,
    });

    vi.mocked(invoke).mockResolvedValueOnce("auto");
    const mode = await IpcClient.getRemoteEditUploadMode();
    expect(mode).toBe("auto");
    expect(invoke).toHaveBeenCalledWith("get_remote_edit_upload_mode", undefined);

    vi.mocked(invoke).mockResolvedValueOnce("prompt");
    const updatedMode = await IpcClient.setRemoteEditUploadMode("prompt");
    expect(updatedMode).toBe("prompt");
    expect(invoke).toHaveBeenCalledWith("set_remote_edit_upload_mode", {
      mode: "prompt",
    });
  });
});
