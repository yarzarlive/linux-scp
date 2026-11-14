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

  it("invokes launch_ssh_terminal with profileId and optional remoteDirectory", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      profile_id: "prof-1",
      profile_name: "Web Server",
      host: "10.0.0.1",
      port: 22,
      username: "ubuntu",
      terminal_binary: "gnome-terminal",
      pid: 1234,
      status: "ProcessSpawned",
      message: "Launched terminal in /var/www",
    });

    const res = await IpcClient.launchSshTerminal("prof-1", "/var/www");
    expect(res.status).toBe("ProcessSpawned");
    expect(invoke).toHaveBeenCalledWith("launch_ssh_terminal", {
      profileId: "prof-1",
      remoteDirectory: "/var/www",
    });

    vi.mocked(invoke).mockResolvedValueOnce({
      profile_id: "prof-2",
      profile_name: "DB Server",
      host: "10.0.0.2",
      port: 22,
      username: "ubuntu",
      terminal_binary: "gnome-terminal",
      pid: 1235,
      status: "ProcessSpawned",
      message: "Launched terminal",
    });

    await IpcClient.launchSshTerminal("prof-2");
    expect(invoke).toHaveBeenCalledWith("launch_ssh_terminal", {
      profileId: "prof-2",
      remoteDirectory: undefined,
    });
  });

  it("retrieves and persists saved directory paths across panes", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(["/home/user", "/var/log"]);
    const localPaths = await IpcClient.getSavedPaths("local");
    expect(localPaths).toEqual(["/home/user", "/var/log"]);
    expect(invoke).toHaveBeenCalledWith("get_saved_paths", { paneType: "local" });

    vi.mocked(invoke).mockResolvedValueOnce(undefined);
    await IpcClient.saveSavedPaths("remote", ["/etc/nginx", "/var/www"]);
    expect(invoke).toHaveBeenCalledWith("save_saved_paths", {
      paneType: "remote",
      paths: ["/etc/nginx", "/var/www"],
    });
  });

  it("retrieves and persists theme preference via IPC", async () => {
    vi.mocked(invoke).mockResolvedValueOnce("light");
    const theme = await IpcClient.getThemePreference();
    expect(theme).toBe("light");
    expect(invoke).toHaveBeenCalledWith("get_theme_preference", undefined);

    vi.mocked(invoke).mockResolvedValueOnce(undefined);
    await IpcClient.setThemePreference("dark");
    expect(invoke).toHaveBeenCalledWith("set_theme_preference", { theme: "dark" });
  });
});
