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
});

