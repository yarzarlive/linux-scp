import { describe, it, expect, beforeEach } from "vitest";
import { RemoteDirectoryCacheManager } from "../../src/services/remoteDirectoryCache";
import { RemoteDirectoryListing } from "../../src/types/filesystem";

function makeMockListing(path: string, names: string[]): RemoteDirectoryListing {
  return {
    current_path: path,
    parent_path: path === "/" ? null : "/",
    home_path: "/home/ubuntu",
    entries: names.map((name) => ({
      name,
      path: path === "/" ? `/${name}` : `${path}/${name}`,
      file_type: { type: "Regular" },
      is_hidden: name.startsWith("."),
      metadata: {
        size: 1024,
        modified_at_secs: 1700000000,
      },
    })),
  };
}

describe("RemoteDirectoryCacheManager", () => {
  let cache: RemoteDirectoryCacheManager;

  beforeEach(() => {
    cache = new RemoteDirectoryCacheManager();
  });

  it("caches opened directories and retrieves them accurately", () => {
    const listing = makeMockListing("/var/log", ["syslog", "auth.log"]);
    cache.set("30.20.10.1", "/var/log", false, listing);

    const retrieved = cache.get("30.20.10.1", "/var/log", false);
    expect(retrieved).toBeDefined();
    expect(retrieved?.entries.length).toBe(2);
    expect(retrieved?.entries[0].name).toBe("syslog");
  });

  it("maintains unique, isolated directory listing caches for each server IP", () => {
    const server1 = "30.20.10.1";
    const server2 = "100.90.80.10";

    const listing1 = makeMockListing("/var/log", ["server1-syslog.log"]);
    const listing2 = makeMockListing("/var/log", ["server2-nginx.log"]);

    cache.set(server1, "/var/log", false, listing1);
    cache.set(server2, "/var/log", false, listing2);

    expect(cache.serverCacheCount(server1)).toBe(1);
    expect(cache.serverCacheCount(server2)).toBe(1);

    const res1 = cache.get(server1, "/var/log", false);
    const res2 = cache.get(server2, "/var/log", false);

    expect(res1?.entries[0].name).toBe("server1-syslog.log");
    expect(res2?.entries[0].name).toBe("server2-nginx.log");
  });

  it("clears all directory listings on server login without affecting other servers", () => {
    const server1 = "30.20.10.1";
    const server2 = "100.90.80.10";

    cache.set(server1, "/var/log", false, makeMockListing("/var/log", ["syslog"]));
    cache.set(server1, "/etc", false, makeMockListing("/etc", ["hosts"]));
    cache.set(server2, "/var/log", false, makeMockListing("/var/log", ["nginx.log"]));

    expect(cache.serverCacheCount(server1)).toBe(2);
    expect(cache.serverCacheCount(server2)).toBe(1);

    // On login to server1, clear its directory listings
    cache.clearServer(server1);

    expect(cache.serverCacheCount(server1)).toBe(0);
    expect(cache.get(server1, "/var/log", false)).toBeUndefined();
    expect(cache.get(server1, "/etc", false)).toBeUndefined();

    // server2 cache remains intact
    expect(cache.serverCacheCount(server2)).toBe(1);
    expect(cache.get(server2, "/var/log", false)).toBeDefined();
  });

  it("invalidates and refreshes directory listing cache on manual refresh", () => {
    const server = "30.20.10.1";
    const oldListing = makeMockListing("/var/log", ["syslog"]);
    cache.set(server, "/var/log", false, oldListing);
    expect(cache.get(server, "/var/log", false)?.entries.length).toBe(1);

    // Manual refresh invalidates the directory
    cache.invalidate(server, "/var/log");
    expect(cache.get(server, "/var/log", false)).toBeUndefined();

    // New live listing is cached
    const freshListing = makeMockListing("/var/log", ["syslog", "auth.log", "kern.log"]);
    cache.set(server, "/var/log", false, freshListing);
    expect(cache.get(server, "/var/log", false)?.entries.length).toBe(3);
  });

  it("normalizes trailing slashes so paths match regardless of trailing slash", () => {
    const server = "30.20.10.1";
    const listing = makeMockListing("/var/log", ["syslog"]);
    cache.set(server, "/var/log/", false, listing);

    expect(cache.get(server, "/var/log", false)).toBeDefined();
    expect(cache.get(server, "/var/log/", false)).toBeDefined();
  });
});

