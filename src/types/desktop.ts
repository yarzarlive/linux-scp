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

