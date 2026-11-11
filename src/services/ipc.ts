import { invoke } from "@tauri-apps/api/core";
import { AppError, PingResponse, SystemInfo } from "../types/ipc";

/**
 * Type-safe IPC client bridging the React frontend to Tauri Rust commands.
 */
export class IpcClient {
  /**
   * Invokes a Tauri command with explicit parameter and return typing,
   * parsing typed AppError failures safely.
   */
  static async invokeCommand<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
    try {
      return await invoke<T>(cmd, args);
    } catch (error) {
      if (typeof error === "object" && error !== null && "kind" in error && "message" in error) {
        throw error as AppError;
      }
      throw {
        kind: "Internal",
        message: typeof error === "string" ? error : JSON.stringify(error),
      } as AppError;
    }
  }

  /**
   * Fetches authentic host and application system information.
   */
  static async getSystemInfo(): Promise<SystemInfo> {
    return this.invokeCommand<SystemInfo>("get_system_info");
  }

  /**
   * Sends a healthcheck ping to verify backend IPC responsiveness.
   */
  static async pingBackend(payload: string): Promise<PingResponse> {
    return this.invokeCommand<PingResponse>("ping_backend", { payload });
  }
}
