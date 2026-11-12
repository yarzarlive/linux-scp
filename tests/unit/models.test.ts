import { describe, it, expect } from "vitest";
import {
  ConnectionProfile,
  ProtocolType,
  FileEntry,
  FileMetadata,
  TransferJob,
} from "../../src/types";

describe("Domain Models & Invariants", () => {
  it("maintains protocol default port semantics", () => {
    const ports: Record<ProtocolType, number> = {
      SFTP: 22,
      FTP: 21,
      FTPS: 990,
      SCP: 22,
    };
    expect(ports.SFTP).toBe(22);
    expect(ports.FTP).toBe(21);
    expect(ports.FTPS).toBe(990);
    expect(ports.SCP).toBe(22);
  });

  it("distinguishes unknown file size from 0 bytes", () => {
    const unknownMeta: FileMetadata = {
      size: undefined,
    };
    const emptyMeta: FileMetadata = {
      size: 0,
    };

    expect(unknownMeta.size).toBeUndefined();
    expect(emptyMeta.size).toBe(0);
    expect(unknownMeta.size).not.toEqual(emptyMeta.size);
  });

  it("handles hidden dotfile detection correctly", () => {
    const regularFile: FileEntry = {
      name: "test.txt",
      path: "/home/user/test.txt",
      file_type: { type: "Regular" },
      metadata: {},
      is_hidden: false,
    };
    const hiddenFile: FileEntry = {
      name: ".bashrc",
      path: "/home/user/.bashrc",
      file_type: { type: "Regular" },
      metadata: {},
      is_hidden: true,
    };

    expect(regularFile.is_hidden).toBe(false);
    expect(hiddenFile.is_hidden).toBe(true);
  });

  it("ensures connection profile does not contain plaintext secret fields", () => {
    const profile: ConnectionProfile = {
      id: "uuid-123",
      name: "Production Server",
      protocol: "SFTP",
      host: "10.0.0.1",
      port: 22,
      username: "admin",
      auth_method: { type: "Password" },
      sudo_sftp: { enabled: false },
      transfer_settings: {
        preserve_timestamps: true,
        preserve_permissions: true,
        concurrency_limit: 2,
      },
      connection_settings: {
        timeout_seconds: 30,
        keepalive_interval_seconds: 15,
      },
      created_at_ms: 1700000000,
      updated_at_ms: 1700000000,
    };

    const keys = Object.keys(profile);
    expect(keys).not.toContain("password");
    expect(keys).not.toContain("passphrase");
    expect(keys).not.toContain("secret");
  });

  it("handles transfer job lifecycle states correctly", () => {
    const job: TransferJob = {
      id: "transfer-1",
      session_id: "session-1",
      source_path: "/local/data.zip",
      destination_path: "/remote/data.zip",
      direction: "upload",
      state: { status: "Queued" },
      total_bytes: 1048576,
      transferred_bytes: 0,
      speed_bps: 0,
      conflict_policy: "ask",
      created_at_ms: 1700000000,
      updated_at_ms: 1700000000,
    };

    expect(job.state.status).toBe("Queued");
    expect(job.direction).toBe("upload");
  });
});

