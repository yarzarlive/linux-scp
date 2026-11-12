import { FileOwnership, FileType, UnixPermissions } from "../types/filesystem";

/**
 * Formats a file size in bytes to human-readable format.
 * As per specification, directories must display as "—" (dash).
 */
export function formatFileSize(size: number | undefined, isDirectory: boolean): string {
  if (isDirectory) {
    return "—";
  }
  if (size === undefined || size === null) {
    return "—";
  }
  if (size === 0) {
    return "0 B";
  }

  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(size) / Math.log(1024));
  const clampedIndex = Math.min(i, units.length - 1);
  const value = size / Math.pow(1024, clampedIndex);

  if (clampedIndex === 0) {
    return `${size} B`;
  }
  return `${value.toFixed(value < 10 ? 1 : 0)} ${units[clampedIndex]}`;
}

/**
 * Formats an epoch timestamp in seconds to a standard YYYY-MM-DD HH:mm representation.
 */
export function formatTimestamp(timestampSecs?: number): string {
  if (!timestampSecs || timestampSecs <= 0) {
    return "—";
  }
  const date = new Date(timestampSecs * 1000);
  const pad = (n: number) => n.toString().padStart(2, "0");

  const year = date.getFullYear();
  const month = pad(date.getMonth() + 1);
  const day = pad(date.getDate());
  const hours = pad(date.getHours());
  const minutes = pad(date.getMinutes());

  return `${year}-${month}-${day} ${hours}:${minutes}`;
}

/**
 * Formats Unix permission bits and file type into standard 10-character POSIX string (e.g. `drwxr-xr-x`).
 */
export function formatPermissions(fileType: FileType, perms?: UnixPermissions): string {
  if (!perms) {
    return "—";
  }

  let typeChar = "-";
  if (fileType.type === "Directory") {
    typeChar = "d";
  } else if (fileType.type === "Symlink") {
    typeChar = "l";
  } else if (fileType.type === "Special") {
    typeChar = "c";
  }

  const uR = perms.user_read ? "r" : "-";
  const uW = perms.user_write ? "w" : "-";
  const uX = perms.set_uid ? (perms.user_execute ? "s" : "S") : perms.user_execute ? "x" : "-";

  const gR = perms.group_read ? "r" : "-";
  const gW = perms.group_write ? "w" : "-";
  const gX = perms.set_gid ? (perms.group_execute ? "s" : "S") : perms.group_execute ? "x" : "-";

  const oR = perms.other_read ? "r" : "-";
  const oW = perms.other_write ? "w" : "-";
  const oX = perms.sticky ? (perms.other_execute ? "t" : "T") : perms.other_execute ? "x" : "-";

  return `${typeChar}${uR}${uW}${uX}${gR}${gW}${gX}${oR}${oW}${oX}`;
}

/**
 * Formats ownership into `username:groupname` (or numeric IDs if unmapped).
 */
export function formatOwnership(ownership?: FileOwnership): string {
  if (!ownership) {
    return "—";
  }

  const user = ownership.user_name ?? (ownership.uid !== undefined ? String(ownership.uid) : "—");
  const group = ownership.group_name ?? (ownership.gid !== undefined ? String(ownership.gid) : "—");

  if (user === "—" && group === "—") {
    return "—";
  }
  return `${user}:${group}`;
}

/**
 * Helper to check if a FileType represents a directory.
 */
export function isDirectory(fileType: FileType): boolean {
  return fileType.type === "Directory";
}

/**
 * Helper to check if a FileType represents a symlink.
 */
export function isSymlink(fileType: FileType): boolean {
  return fileType.type === "Symlink";
}
