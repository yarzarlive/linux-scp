import { invoke } from "@tauri-apps/api/core";
import { LocalDirectoryListing } from "../types/filesystem";
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

  /**
   * Lists authentic contents of a local directory via Rust POSIX backend.
   */
  static async listLocalDirectory(
    path?: string,
    showHidden?: boolean
  ): Promise<LocalDirectoryListing> {
    return this.invokeCommand<LocalDirectoryListing>("list_local_directory", {
      path,
      showHidden,
    });
  }

  /**
   * Retrieves the authentic current user home directory path.
   */
  static async getHomeDirectory(): Promise<string> {
    return this.invokeCommand<string>("get_home_directory");
  }

  /**
   * Creates a new local directory.
   */
  static async createLocalDirectory(parentPath: string, dirName: string): Promise<string> {
    return this.invokeCommand<string>("create_local_directory", {
      parentPath,
      dirName,
    });
  }

  /**
   * Creates a new local empty file.
   */
  static async createLocalFile(parentPath: string, fileName: string): Promise<string> {
    return this.invokeCommand<string>("create_local_file", {
      parentPath,
      fileName,
    });
  }

  /**
   * Renames a local file or directory.
   */
  static async renameLocalEntry(sourcePath: string, newName: string): Promise<string> {
    return this.invokeCommand<string>("rename_local_entry", {
      sourcePath,
      newName,
    });
  }

  /**
   * Deletes one or more local files or directories.
   */
  static async deleteLocalEntries(paths: string[]): Promise<string[]> {
    return this.invokeCommand<string[]>("delete_local_entries", {
      paths,
    });
  }
}
