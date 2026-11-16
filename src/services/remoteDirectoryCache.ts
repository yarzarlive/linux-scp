import { RemoteDirectoryListing } from "../types/filesystem";

/**
 * Normalizes posix directory path for cache keys (strips trailing slashes except root).
 */
function normalizePathKey(path: string): string {
  const trimmed = path.trim();
  if (trimmed === "" || trimmed === "/") return "/";
  const normalized = trimmed.replace(/\/+$/, "");
  return normalized === "" ? "/" : normalized;
}

/**
 * Client-side directory listing cache partitioned by server IP/host.
 * - Caches directory listings of every opened directory for fast speed user experience
 * - Unique directory listing for each server IP (e.g. 30.20.10.1 and 100.90.80.10 have isolated caches)
 * - Clears all directory listings on login for that server IP
 * - Refreshes directory listing cache of a directory when user manually triggers refresh or mutations
 */
export class RemoteDirectoryCacheManager {
  private cache = new Map<string, Map<string, RemoteDirectoryListing>>();

  private makeKey(path: string, showHidden: boolean): string {
    return `${normalizePathKey(path)}::${showHidden ? "hidden" : "normal"}`;
  }

  /**
   * Returns cached directory listing for a given server IP, path, and hidden flag.
   */
  get(
    serverIp: string | undefined,
    path: string | undefined,
    showHidden: boolean
  ): RemoteDirectoryListing | undefined {
    if (!serverIp || !path) return undefined;
    const serverMap = this.cache.get(serverIp);
    return serverMap?.get(this.makeKey(path, showHidden));
  }

  /**
   * Caches a directory listing under the given server IP.
   */
  set(
    serverIp: string | undefined,
    path: string | undefined,
    showHidden: boolean,
    listing: RemoteDirectoryListing
  ): void {
    if (!serverIp || !path) return;
    let serverMap = this.cache.get(serverIp);
    if (!serverMap) {
      serverMap = new Map();
      this.cache.set(serverIp, serverMap);
    }
    serverMap.set(this.makeKey(path, showHidden), listing);
    if (listing.current_path && listing.current_path !== path) {
      serverMap.set(this.makeKey(listing.current_path, showHidden), listing);
    }
  }

  /**
   * Invalidates a specific directory path on a server IP (or all paths on that server if path is omitted).
   */
  invalidate(serverIp: string | undefined, path?: string): void {
    if (!serverIp) return;
    if (!path) {
      this.clearServer(serverIp);
      return;
    }
    const serverMap = this.cache.get(serverIp);
    if (serverMap) {
      serverMap.delete(this.makeKey(path, true));
      serverMap.delete(this.makeKey(path, false));
    }
  }

  /**
   * Clears all cached directory listings for a server IP on login.
   */
  clearServer(serverIp: string | undefined): void {
    if (serverIp) {
      this.cache.delete(serverIp);
    }
  }

  /**
   * Clears entire cache across all servers.
   */
  clearAll(): void {
    this.cache.clear();
  }

  /**
   * Returns count of cached directories for a server IP.
   */
  serverCacheCount(serverIp: string): number {
    return this.cache.get(serverIp)?.size ?? 0;
  }
}

export const remoteDirectoryCache = new RemoteDirectoryCacheManager();

