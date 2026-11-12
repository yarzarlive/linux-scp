import { describe, expect, it } from "vitest";
import {
  formatFileSize,
  formatOwnership,
  formatPermissions,
  formatTimestamp,
  isDirectory,
  isSymlink,
} from "../../src/utils/formatters";
import { FileOwnership, FileType, UnixPermissions } from "../../src/types/filesystem";

describe("formatters", () => {
  describe("formatFileSize", () => {
    it("returns dash for directories", () => {
      expect(formatFileSize(4096, true)).toBe("—");
      expect(formatFileSize(0, true)).toBe("—");
      expect(formatFileSize(undefined, true)).toBe("—");
    });

    it("returns dash for missing size on files", () => {
      expect(formatFileSize(undefined, false)).toBe("—");
    });

    it("formats bytes accurately", () => {
      expect(formatFileSize(0, false)).toBe("0 B");
      expect(formatFileSize(512, false)).toBe("512 B");
      expect(formatFileSize(1024, false)).toBe("1.0 KB");
      expect(formatFileSize(1536, false)).toBe("1.5 KB");
      expect(formatFileSize(1048576, false)).toBe("1.0 MB");
      expect(formatFileSize(1073741824, false)).toBe("1.0 GB");
    });
  });

  describe("formatTimestamp", () => {
    it("returns dash for undefined or 0", () => {
      expect(formatTimestamp(undefined)).toBe("—");
      expect(formatTimestamp(0)).toBe("—");
    });

    it("formats valid epoch seconds", () => {
      // 1700000000 = 2023-11-14T22:13:20Z (UTC)
      const formatted = formatTimestamp(1700000000);
      expect(formatted).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/);
    });
  });

  describe("formatPermissions", () => {
    const dirType: FileType = { type: "Directory" };
    const regType: FileType = { type: "Regular" };
    const symType: FileType = { type: "Symlink", target: "/opt" };

    const rwxr_xr_x: UnixPermissions = {
      raw_mode: 0o755,
      user_read: true,
      user_write: true,
      user_execute: true,
      group_read: true,
      group_write: false,
      group_execute: true,
      other_read: true,
      other_write: false,
      other_execute: true,
      set_uid: false,
      set_gid: false,
      sticky: false,
    };

    it("formats 755 directory", () => {
      expect(formatPermissions(dirType, rwxr_xr_x)).toBe("drwxr-xr-x");
    });

    it("formats 755 regular file", () => {
      expect(formatPermissions(regType, rwxr_xr_x)).toBe("-rwxr-xr-x");
    });

    it("formats symlink", () => {
      expect(formatPermissions(symType, rwxr_xr_x)).toBe("lrwxr-xr-x");
    });

    it("formats setuid, setgid, sticky flags", () => {
      const special: UnixPermissions = {
        ...rwxr_xr_x,
        set_uid: true,
        set_gid: true,
        sticky: true,
      };
      expect(formatPermissions(regType, special)).toBe("-rwsr-sr-t");
    });

    it("returns dash for undefined permissions", () => {
      expect(formatPermissions(regType, undefined)).toBe("—");
    });
  });

  describe("formatOwnership", () => {
    it("formats user:group with names", () => {
      const own: FileOwnership = {
        uid: 1000,
        gid: 1000,
        user_name: "alice",
        group_name: "developers",
      };
      expect(formatOwnership(own)).toBe("alice:developers");
    });

    it("falls back to numeric IDs when names absent", () => {
      const own: FileOwnership = { uid: 1001, gid: 1002 };
      expect(formatOwnership(own)).toBe("1001:1002");
    });

    it("returns dash for missing ownership", () => {
      expect(formatOwnership(undefined)).toBe("—");
    });
  });

  describe("type helpers", () => {
    it("detects directories and symlinks", () => {
      expect(isDirectory({ type: "Directory" })).toBe(true);
      expect(isDirectory({ type: "Regular" })).toBe(false);
      expect(isSymlink({ type: "Symlink" })).toBe(true);
      expect(isSymlink({ type: "Regular" })).toBe(false);
    });
  });
});
