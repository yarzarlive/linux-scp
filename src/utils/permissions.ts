import { UnixPermissions } from "../types/filesystem";

export interface PermissionBits {
  userRead: boolean;
  userWrite: boolean;
  userExecute: boolean;
  groupRead: boolean;
  groupWrite: boolean;
  groupExecute: boolean;
  otherRead: boolean;
  otherWrite: boolean;
  otherExecute: boolean;
  setUid: boolean;
  setGid: boolean;
  sticky: boolean;
}

export function modeToBits(mode: number): PermissionBits {
  return {
    userRead: (mode & 0o400) !== 0,
    userWrite: (mode & 0o200) !== 0,
    userExecute: (mode & 0o100) !== 0,
    groupRead: (mode & 0o040) !== 0,
    groupWrite: (mode & 0o020) !== 0,
    groupExecute: (mode & 0o010) !== 0,
    otherRead: (mode & 0o004) !== 0,
    otherWrite: (mode & 0o002) !== 0,
    otherExecute: (mode & 0o001) !== 0,
    setUid: (mode & 0o4000) !== 0,
    setGid: (mode & 0o2000) !== 0,
    sticky: (mode & 0o1000) !== 0,
  };
}

export function unixPermissionsToBits(perms?: UnixPermissions): PermissionBits {
  if (!perms) {
    return modeToBits(0o644);
  }
  return {
    userRead: perms.user_read,
    userWrite: perms.user_write,
    userExecute: perms.user_execute,
    groupRead: perms.group_read,
    groupWrite: perms.group_write,
    groupExecute: perms.group_execute,
    otherRead: perms.other_read,
    otherWrite: perms.other_write,
    otherExecute: perms.other_execute,
    setUid: perms.set_uid,
    setGid: perms.set_gid,
    sticky: perms.sticky,
  };
}

export function bitsToMode(bits: PermissionBits): number {
  let mode = 0;
  if (bits.setUid) mode |= 0o4000;
  if (bits.setGid) mode |= 0o2000;
  if (bits.sticky) mode |= 0o1000;

  if (bits.userRead) mode |= 0o400;
  if (bits.userWrite) mode |= 0o200;
  if (bits.userExecute) mode |= 0o100;

  if (bits.groupRead) mode |= 0o040;
  if (bits.groupWrite) mode |= 0o020;
  if (bits.groupExecute) mode |= 0o010;

  if (bits.otherRead) mode |= 0o004;
  if (bits.otherWrite) mode |= 0o002;
  if (bits.otherExecute) mode |= 0o001;
  return mode;
}

export function bitsToOctalString(bits: PermissionBits): string {
  return bitsToMode(bits).toString(8).padStart(4, "0");
}

export function bitsToSymbolicString(bits: PermissionBits): string {
  const uExec = bits.setUid ? (bits.userExecute ? "s" : "S") : bits.userExecute ? "x" : "-";
  const gExec = bits.setGid ? (bits.groupExecute ? "s" : "S") : bits.groupExecute ? "x" : "-";
  const oExec = bits.sticky ? (bits.otherExecute ? "t" : "T") : bits.otherExecute ? "x" : "-";

  return [
    bits.userRead ? "r" : "-",
    bits.userWrite ? "w" : "-",
    uExec,
    bits.groupRead ? "r" : "-",
    bits.groupWrite ? "w" : "-",
    gExec,
    bits.otherRead ? "r" : "-",
    bits.otherWrite ? "w" : "-",
    oExec,
  ].join("");
}

export function parseOctalInput(
  raw: string
): { valid: true; mode: number; bits: PermissionBits } | { valid: false; error: string } {
  const trimmed = raw.trim();
  if (!/^[0-7]{3,4}$/.test(trimmed)) {
    return {
      valid: false,
      error: "Enter 3 or 4 octal digits (0-7), e.g. 0755 or 4755.",
    };
  }
  const mode = parseInt(trimmed, 8);
  if (Number.isNaN(mode) || mode < 0 || mode > 0o7777) {
    return {
      valid: false,
      error: "Octal mode must be between 0000 and 7777.",
    };
  }
  return {
    valid: true,
    mode,
    bits: modeToBits(mode),
  };
}
