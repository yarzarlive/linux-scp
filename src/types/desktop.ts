export const NO_EXTENSION_KEY = "__no_extension__";

export interface DesktopAppInfo {
  id: string;
  name: string;
  comment?: string | null;
  icon?: string | null;
  exec_raw: string;
  executable: string;
  args_template: string[];
  mime_types: string[];
  categories: string[];
  terminal: boolean;
  desktop_file_path: string;
}

export interface FileAppAssociation {
  extension: string;
  desktop_app_id: string;
  created_at: string;
  updated_at_ms: number;
}

export interface OpenWithContext {
  extension_key: string;
  extension_display: string;
  remembered_app_id?: string | null;
  remembered_app_installed: boolean;
  recommended_app_ids: string[];
  applications: DesktopAppInfo[];
}

export type LocalFileOpenOutcome =
  | {
      outcome: "Launched";
      app: DesktopAppInfo;
      extension_key: string;
      pid?: number | null;
    }
  | {
      outcome: "NeedsAppPicker";
      extension_key: string;
      extension_display: string;
      missing_remembered_app_id?: string | null;
    };

export interface LocalFileLaunchResult {
  app: DesktopAppInfo;
  extension_key: string;
  saved_as_default: boolean;
  pid?: number | null;
}

export interface RemoteEndpointIdentity {
  host: string;
  port: number;
  username: string;
  elevated: boolean;
}

export interface RemoteFileFingerprint {
  size?: number | null;
  modified_at_secs?: number | null;
  permissions_mode?: number | null;
  uid?: number | null;
  gid?: number | null;
}

export type UploadFailureKind = "Disconnected" | "WrongServer" | "RemoteError" | "LocalError";

export type RemoteEditState =
  | { state: "Clean" }
  | { state: "Modified" }
  | { state: "Uploading" }
  | {
      state: "Conflict";
      details: {
        reason: string;
        remote_current?: RemoteFileFingerprint | null;
      };
    }
  | {
      state: "UploadFailed";
      details: {
        message: string;
        kind: UploadFailureKind;
      };
    }
  | { state: "LocalMissing" };

export interface RemoteEditSession {
  id: string;
  endpoint: RemoteEndpointIdentity;
  profile_id?: string | null;
  remote_path: string;
  file_name: string;
  local_path: string;
  app_id?: string | null;
  app_name?: string | null;
  baseline: RemoteFileFingerprint;
  baseline_sha256: string;
  local_sha256?: string | null;
  state: RemoteEditState;
  prompt_dismissed: boolean;
  has_unsynced_changes: boolean;
  created_at_ms: number;
  last_synced_at_ms: number;
  upload_count: number;
  recovered?: boolean;
}

export type RemoteFileOpenOutcome =
  | {
      outcome: "Launched";
      app: DesktopAppInfo;
      extension_key: string;
      pid?: number | null;
      session: RemoteEditSession;
    }
  | {
      outcome: "NeedsAppPicker";
      extension_key: string;
      extension_display: string;
      missing_remembered_app_id?: string | null;
    };

export interface RemoteFileLaunchResult {
  app: DesktopAppInfo;
  extension_key: string;
  saved_as_default: boolean;
  pid?: number | null;
  session: RemoteEditSession;
}

export type RemoteEditUploadOutcome =
  | { outcome: "Uploaded"; session: RemoteEditSession }
  | { outcome: "Unchanged"; session: RemoteEditSession }
  | { outcome: "Conflict"; session: RemoteEditSession }
  | { outcome: "Failed"; session: RemoteEditSession };

export type RemoteEditUploadMode = "auto" | "prompt";
