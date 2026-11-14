export type ProtocolType = "SFTP" | "FTP" | "FTPS" | "SCP";

export type AuthMethod =
  | { type: "Password" }
  | { type: "PublicKey"; details: { private_key_path: string; requires_passphrase: boolean } }
  | { type: "Agent" }
  | { type: "Interactive" };

export interface SudoSftpConfig {
  enabled: boolean;
  sftp_server_path?: string;
  custom_command?: string;
}

export interface ProfileConnectionSettings {
  timeout_seconds: number;
  keepalive_interval_seconds?: number;
}

export interface ProfileTransferSettings {
  preserve_timestamps: boolean;
  preserve_permissions: boolean;
  concurrency_limit: number;
}

export interface ConnectionProfile {
  id: string;
  name: string;
  protocol: ProtocolType;
  host: string;
  port: number;
  username: string;
  auth_method: AuthMethod;
  initial_local_directory?: string;
  initial_remote_directory?: string;
  sudo_sftp: SudoSftpConfig;
  transfer_settings: ProfileTransferSettings;
  connection_settings: ProfileConnectionSettings;
  created_at_ms: number;
  updated_at_ms: number;
}

export interface ConnectionProfileSummary {
  id: string;
  name: string;
  protocol: ProtocolType;
  host: string;
  port: number;
  username: string;
  has_saved_secret: boolean;
  updated_at_ms: number;
}
