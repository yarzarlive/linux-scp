/**
 * Core IPC Data Transfer Objects matching Rust backend models.
 */

export interface SystemInfo {
  app_name: string;
  app_version: string;
  os_name: string;
  os_family: string;
  arch: string;
}

export interface PingResponse {
  status: string;
  timestamp_ms: number;
}

export type AppErrorKind =
  | "Validation"
  | "Filesystem"
  | "Protocol"
  | "Authentication"
  | "Security"
  | "Database"
  | "Internal";

export interface AppError {
  kind: AppErrorKind;
  message: string;
}
