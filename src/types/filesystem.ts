export type FileType =
  | { type: "Regular" }
  | { type: "Directory" }
  | { type: "Symlink"; target?: string }
  | { type: "Special" };

export interface UnixPermissions {
  raw_mode: number;
  user_read: boolean;
  user_write: boolean;
  user_execute: boolean;
  group_read: boolean;
  group_write: boolean;
  group_execute: boolean;
  other_read: boolean;
  other_write: boolean;
  other_execute: boolean;
  set_uid: boolean;
  set_gid: boolean;
  sticky: boolean;
}

export interface FileOwnership {
  uid?: number;
  gid?: number;
  user_name?: string;
  group_name?: string;
}

export interface FileMetadata {
  size?: number;
  modified_at_secs?: number;
  accessed_at_secs?: number;
  permissions?: UnixPermissions;
  ownership?: FileOwnership;
}

export interface FileEntry {
  name: string;
  path: string;
  file_type: FileType;
  metadata: FileMetadata;
  is_hidden: boolean;
}
