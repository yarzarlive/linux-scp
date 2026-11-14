import { invoke } from "@tauri-apps/api/core";
import { ConnectionProfile, ConnectionProfileSummary } from "../types/connection";
import {
  FileAppAssociation,
  LocalFileLaunchResult,
  LocalFileOpenOutcome,
  OpenWithContext,
  RemoteEditSession,
  RemoteEditUploadMode,
  RemoteEditUploadOutcome,
  RemoteFileLaunchResult,
  RemoteFileOpenOutcome,
} from "../types/desktop";
import { FileOwnership, LocalDirectoryListing, RemoteDirectoryListing } from "../types/filesystem";
import { AppError, PingResponse, SystemInfo } from "../types/ipc";
import { ConnectOutcome, KnownHostEntry, SessionStateSnapshot } from "../types/session";
import {
  TerminalEnvironmentInfo,
  TerminalLaunchResult,
  TerminalPreference,
} from "../types/terminal";
import { ConflictDecision, ConflictPolicy, TransferJob } from "../types/transfer";

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
   * Retrieves saved theme preference ("dark" or "light").
   */
  static async getThemePreference(): Promise<"dark" | "light"> {
    return this.invokeCommand<"dark" | "light">("get_theme_preference");
  }

  /**
   * Persists saved theme preference ("dark" or "light").
   */
  static async setThemePreference(theme: "dark" | "light"): Promise<void> {
    return this.invokeCommand<void>("set_theme_preference", { theme });
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
   * Retrieves saved directory paths (bookmarks) for a scope/pane (e.g. "local" or "remote:<siteId>").
   */
  static async getSavedPaths(paneType: string): Promise<string[]> {
    return this.invokeCommand<string[]>("get_saved_paths", { paneType });
  }

  /**
   * Persists saved directory paths (bookmarks) for a scope/pane (e.g. "local" or "remote:<siteId>").
   */
  static async saveSavedPaths(paneType: string, paths: string[]): Promise<void> {
    return this.invokeCommand<void>("save_saved_paths", { paneType, paths });
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
   * Exports all connection profiles as formatted JSON.
   */
  static async exportConnectionProfiles(
    destinationPath?: string
  ): Promise<{ file_path?: string; json_content: string; count: number }> {
    return this.invokeCommand<{ file_path?: string; json_content: string; count: number }>(
      "export_connection_profiles",
      { destinationPath: destinationPath || null }
    );
  }

  /**
   * Imports connection profiles from JSON string via 'merge' or 'replace' mode.
   */
  static async importConnectionProfiles(
    jsonContent: string,
    mode: "merge" | "replace"
  ): Promise<{ imported_count: number; total_count: number; mode: string }> {
    return this.invokeCommand<{ imported_count: number; total_count: number; mode: string }>(
      "import_connection_profiles",
      { jsonContent, mode }
    );
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

  /**
   * Lists a remote directory over the active SFTP session.
   */
  static async listRemoteDirectory(
    path?: string,
    showHidden?: boolean
  ): Promise<RemoteDirectoryListing> {
    return this.invokeCommand<RemoteDirectoryListing>("list_remote_directory", {
      path,
      showHidden,
    });
  }

  /**
   * Checks whether a remote path (following symlinks) points to a directory.
   */
  static async checkRemotePathIsDir(path: string): Promise<boolean> {
    return this.invokeCommand<boolean>("check_remote_path_is_dir", {
      path,
    });
  }

  /**
   * Creates a new directory on the active remote SFTP server.
   */
  static async createRemoteDirectory(parentPath: string, dirName: string): Promise<string> {
    return this.invokeCommand<string>("create_remote_directory", {
      parentPath,
      dirName,
    });
  }

  /**
   * Creates a new empty file on the active remote SFTP server.
   */
  static async createRemoteFile(parentPath: string, fileName: string): Promise<string> {
    return this.invokeCommand<string>("create_remote_file", {
      parentPath,
      fileName,
    });
  }

  /**
   * Renames a remote file or directory.
   */
  static async renameRemoteEntry(sourcePath: string, newName: string): Promise<string> {
    return this.invokeCommand<string>("rename_remote_entry", {
      sourcePath,
      newName,
    });
  }

  /**
   * Deletes a remote file or symlink.
   */
  static async deleteRemoteFile(path: string): Promise<void> {
    return this.invokeCommand<void>("delete_remote_file", {
      path,
    });
  }

  /**
   * Deletes a remote directory (non-recursive unless explicitly confirmed).
   */
  static async deleteRemoteDirectory(path: string, recursive: boolean): Promise<void> {
    return this.invokeCommand<void>("delete_remote_directory", {
      path,
      recursive,
    });
  }

  /**
   * Sets Unix permissions (octal mode, optionally recursive) on remote paths.
   */
  static async setRemotePermissions(
    paths: string[],
    octalMode: string,
    recursive: boolean
  ): Promise<void> {
    return this.invokeCommand<void>("set_remote_permissions", {
      paths,
      octalMode,
      recursive,
    });
  }

  /**
   * Sets numeric UID/GID (or resolves names via optional `getent`) on a remote path.
   */
  static async setRemoteOwnership(
    path: string,
    uid?: number,
    gid?: number,
    userName?: string,
    groupName?: string
  ): Promise<void> {
    return this.invokeCommand<void>("set_remote_ownership", {
      path,
      uid,
      gid,
      userName,
      groupName,
    });
  }

  /**
   * Optionally resolves numeric UID/GID to names via remote `getent`.
   */
  static async resolveRemoteIdentity(uid?: number, gid?: number): Promise<FileOwnership> {
    return this.invokeCommand<FileOwnership>("resolve_remote_identity", {
      uid,
      gid,
    });
  }

  /**
   * Enqueues one or more local files or directories for SFTP upload.
   */
  static async enqueueUploads(
    localPaths: string[],
    remoteDir: string,
    conflictPolicy: ConflictPolicy = "ask"
  ): Promise<TransferJob[]> {
    return this.invokeCommand<TransferJob[]>("enqueue_uploads", {
      localPaths,
      remoteDir,
      conflictPolicy,
    });
  }

  /**
   * Enqueues one or more remote files or directories for SFTP download.
   */
  static async enqueueDownloads(
    remotePaths: string[],
    localDir: string,
    conflictPolicy: ConflictPolicy = "ask"
  ): Promise<TransferJob[]> {
    return this.invokeCommand<TransferJob[]>("enqueue_downloads", {
      remotePaths,
      localDir,
      conflictPolicy,
    });
  }

  /**
   * Lists all transfer jobs in the backend queue.
   */
  static async listTransfers(): Promise<TransferJob[]> {
    return this.invokeCommand<TransferJob[]>("list_transfers");
  }

  /**
   * Cancels a queued or active transfer job.
   */
  static async cancelTransfer(jobId: string): Promise<void> {
    return this.invokeCommand<void>("cancel_transfer", { jobId });
  }

  /**
   * Retries a failed or cancelled transfer job.
   */
  static async retryTransfer(jobId: string): Promise<TransferJob> {
    return this.invokeCommand<TransferJob>("retry_transfer", { jobId });
  }

  /**
   * Delivers an explicit user conflict resolution decision to a waiting transfer.
   */
  static async resolveTransferConflict(
    conflictId: string,
    decision: ConflictDecision,
    applyToAll: boolean
  ): Promise<void> {
    return this.invokeCommand<void>("resolve_transfer_conflict", {
      conflictId,
      decision,
      applyToAll,
    });
  }

  /**
   * Clears completed and cancelled jobs from the queue.
   */
  static async clearFinishedTransfers(): Promise<TransferJob[]> {
    return this.invokeCommand<TransferJob[]>("clear_finished_transfers");
  }

  /**
   * Inspects installed terminal emulators, OpenSSH binary availability, and saved user terminal preferences.
   */
  static async getTerminalEnvironment(): Promise<TerminalEnvironmentInfo> {
    return this.invokeCommand<TerminalEnvironmentInfo>("get_terminal_environment");
  }

  /**
   * Saves the user's preferred external terminal emulator configuration.
   */
  static async saveTerminalPreference(preference: TerminalPreference): Promise<TerminalPreference> {
    return this.invokeCommand<TerminalPreference>("save_terminal_preference", {
      preference,
    });
  }

  /**
   * Launches an external interactive SSH terminal session for the selected connection profile,
   * optionally opening directly in the specified remote directory.
   */
  static async launchSshTerminal(
    profileId: string,
    remoteDirectory?: string
  ): Promise<TerminalLaunchResult> {
    return this.invokeCommand<TerminalLaunchResult>("launch_ssh_terminal", {
      profileId,
      remoteDirectory,
    });
  }

  /**
   * Discovers installed Linux XDG desktop applications and resolves remembered associations for a file.
   */
  static async discoverDesktopApplications(filePath?: string): Promise<OpenWithContext> {
    return this.invokeCommand<OpenWithContext>("discover_desktop_applications", {
      filePath,
    });
  }

  /**
   * Opens a local file with its remembered default desktop application, or signals that the picker is needed.
   */
  static async openLocalFileDefault(filePath: string): Promise<LocalFileOpenOutcome> {
    return this.invokeCommand<LocalFileOpenOutcome>("open_local_file_default", {
      filePath,
    });
  }

  /**
   * Opens a local file with the chosen desktop application and optionally remembers the default association.
   */
  static async openLocalFileWithApp(
    filePath: string,
    desktopAppId: string,
    rememberDefault: boolean
  ): Promise<LocalFileLaunchResult> {
    return this.invokeCommand<LocalFileLaunchResult>("open_local_file_with_app", {
      filePath,
      desktopAppId,
      rememberDefault,
    });
  }

  /**
   * Lists all saved LinuxSCP file extension to desktop application associations.
   */
  static async listFileAppAssociations(): Promise<FileAppAssociation[]> {
    return this.invokeCommand<FileAppAssociation[]>("list_file_app_associations");
  }

  /**
   * Removes a remembered default application association for a file extension.
   */
  static async deleteFileAppAssociation(extension: string): Promise<void> {
    return this.invokeCommand<void>("delete_file_app_association", {
      extension,
    });
  }

  /**
   * Opens a remote file with its remembered default desktop application via a managed
   * temporary working copy, or signals that the application picker is needed.
   */
  static async openRemoteFileDefault(remotePath: string): Promise<RemoteFileOpenOutcome> {
    return this.invokeCommand<RemoteFileOpenOutcome>("open_remote_file_default", {
      remotePath,
    });
  }

  /**
   * Downloads a remote file to a managed temporary working copy and opens it with the
   * chosen desktop application, optionally remembering the default association.
   */
  static async openRemoteFileWithApp(
    remotePath: string,
    desktopAppId: string,
    rememberDefault: boolean
  ): Promise<RemoteFileLaunchResult> {
    return this.invokeCommand<RemoteFileLaunchResult>("open_remote_file_with_app", {
      remotePath,
      desktopAppId,
      rememberDefault,
    });
  }

  /**
   * Lists all tracked remote editing sessions after inspecting their local working copies.
   */
  static async listRemoteEditSessions(): Promise<RemoteEditSession[]> {
    return this.invokeCommand<RemoteEditSession[]>("list_remote_edit_sessions");
  }

  /**
   * Scans all tracked remote editing working copies immediately (e.g. on window focus)
   * and returns the updated session list.
   */
  static async scanRemoteEditChanges(): Promise<RemoteEditSession[]> {
    return this.invokeCommand<RemoteEditSession[]>("scan_remote_edit_changes");
  }

  /**
   * Uploads a modified local working copy back to the remote server after checking for remote conflicts.
   */
  static async uploadRemoteEditSession(
    sessionId: string,
    overwriteRemoteChanges = false
  ): Promise<RemoteEditUploadOutcome> {
    return this.invokeCommand<RemoteEditUploadOutcome>("upload_remote_edit_session", {
      sessionId,
      overwriteRemoteChanges,
    });
  }

  /**
   * Dismisses the upload prompt for the current local modification while keeping the local working copy.
   */
  static async keepLocalRemoteEditChanges(sessionId: string): Promise<RemoteEditSession> {
    return this.invokeCommand<RemoteEditSession>("keep_local_remote_edit_changes", {
      sessionId,
    });
  }

  /**
   * Saves a copy of the local temporary working copy into `destinationDir` under a non-conflicting name.
   */
  static async saveRemoteEditLocalCopy(sessionId: string, destinationDir: string): Promise<string> {
    return this.invokeCommand<string>("save_remote_edit_local_copy", {
      sessionId,
      destinationDir,
    });
  }

  /**
   * Discards a tracked remote editing session and cleans up its temporary working copy directory.
   */
  static async discardRemoteEditSession(
    sessionId: string,
    discardLocalChanges = false
  ): Promise<void> {
    return this.invokeCommand<void>("discard_remote_edit_session", {
      sessionId,
      discardLocalChanges,
    });
  }

  /**
   * Replaces the local temporary working copy of a remote editing session with the latest
   * version from the remote server and optionally relaunches its associated desktop app.
   */
  static async reloadRemoteEditFromRemote(
    sessionId: string,
    relaunchApp = true
  ): Promise<RemoteEditSession> {
    return this.invokeCommand<RemoteEditSession>("reload_remote_edit_from_remote", {
      sessionId,
      relaunchApp,
    });
  }

  /**
   * Gets the configured upload-back mode for remote editing sessions ("auto" or "prompt").
   */
  static async getRemoteEditUploadMode(): Promise<RemoteEditUploadMode> {
    return this.invokeCommand<RemoteEditUploadMode>("get_remote_edit_upload_mode");
  }

  /**
   * Sets and persists the upload-back mode for remote editing sessions ("auto" or "prompt").
   */
  static async setRemoteEditUploadMode(mode: RemoteEditUploadMode): Promise<RemoteEditUploadMode> {
    return this.invokeCommand<RemoteEditUploadMode>("set_remote_edit_upload_mode", {
      mode,
    });
  }
}
