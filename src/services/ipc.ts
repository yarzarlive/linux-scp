import { invoke } from "@tauri-apps/api/core";
import { ConnectionProfile, ConnectionProfileSummary } from "../types/connection";
import { LocalDirectoryListing } from "../types/filesystem";
import { AppError, PingResponse, SystemInfo } from "../types/ipc";
import { ConnectOutcome, KnownHostEntry, SessionStateSnapshot } from "../types/session";

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

  /**
   * Lists all saved connection profiles with secret presence indicators.
   */
  static async listConnectionProfiles(): Promise<ConnectionProfileSummary[]> {
    return this.invokeCommand<ConnectionProfileSummary[]>("list_connection_profiles");
  }

  /**
   * Retrieves a full connection profile configuration by id.
   */
  static async getConnectionProfile(id: string): Promise<ConnectionProfile> {
    return this.invokeCommand<ConnectionProfile>("get_connection_profile", { id });
  }

  /**
   * Saves or updates a connection profile and optionally stores its secret in Keyring.
   */
  static async saveConnectionProfile(
    profile: ConnectionProfile,
    secret?: string
  ): Promise<ConnectionProfile> {
    return this.invokeCommand<ConnectionProfile>("save_connection_profile", {
      profile,
      secret,
    });
  }

  /**
   * Duplicates an existing connection profile under a new name.
   */
  static async duplicateConnectionProfile(id: string, newName: string): Promise<ConnectionProfile> {
    return this.invokeCommand<ConnectionProfile>("duplicate_connection_profile", {
      id,
      newName,
    });
  }

  /**
   * Deletes a connection profile and its associated Secret Service credential.
   */
  static async deleteConnectionProfile(id: string): Promise<void> {
    return this.invokeCommand<void>("delete_connection_profile", { id });
  }

  /**
   * Probes whether Linux Secret Service / Keyring storage is responsive.
   */
  static async checkKeyringStatus(): Promise<boolean> {
    return this.invokeCommand<boolean>("check_keyring_status");
  }

  /**
   * Connects to an SFTP server using a saved profile ID.
   */
  static async connectSftpSession(profileId: string, secret?: string): Promise<ConnectOutcome> {
    return this.invokeCommand<ConnectOutcome>("connect_sftp_session", {
      profileId,
      secret,
    });
  }

  /**
   * Disconnects the active SFTP session or cancels an in-progress connection.
   */
  static async disconnectSftpSession(): Promise<SessionStateSnapshot> {
    return this.invokeCommand<SessionStateSnapshot>("disconnect_sftp_session");
  }

  /**
   * Reconnects to the most recently used connection profile.
   */
  static async reconnectSftpSession(secret?: string): Promise<ConnectOutcome> {
    return this.invokeCommand<ConnectOutcome>("reconnect_sftp_session", {
      secret,
    });
  }

  /**
   * Retrieves the current session lifecycle state snapshot.
   */
  static async getSessionState(): Promise<SessionStateSnapshot> {
    return this.invokeCommand<SessionStateSnapshot>("get_session_state");
  }

  /**
   * Explicitly trusts and stores an unknown SSH host key after user approval.
   */
  static async trustHostKey(
    host: string,
    port: number,
    algorithm: string,
    fingerprintSha256: string,
    publicKeyOpenssh: string
  ): Promise<KnownHostEntry> {
    return this.invokeCommand<KnownHostEntry>("trust_host_key", {
      host,
      port,
      algorithm,
      fingerprintSha256,
      publicKeyOpenssh,
    });
  }

  /**
   * Lists all trusted SSH host keys in the persistent known_hosts store.
   */
  static async listKnownHosts(): Promise<KnownHostEntry[]> {
    return this.invokeCommand<KnownHostEntry[]>("list_known_hosts");
  }
}
