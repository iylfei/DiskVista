import { api } from "./api";
import type { FileRecord } from "./types";

export interface SpaceMapSnapshot {
  parent: FileRecord;
  items: FileRecord[];
  total: number;
}

export interface SpaceMapRequest {
  scanId: string;
  parent: string;
  revision: string;
  cacheable: boolean;
}

function pathKey(path: string) {
  while (path.startsWith("\\\\?\\")) path = path.slice(4);
  return path.replaceAll("/", "\\").replace(/\\+$/, "").toLowerCase();
}

function scopeKey(request: SpaceMapRequest) {
  return JSON.stringify([request.scanId, request.revision, request.cacheable]);
}

export function spaceMapRequestKey(request: SpaceMapRequest) {
  return JSON.stringify([scopeKey(request), pathKey(request.parent)]);
}

interface CachedDirectory {
  savedAt: number;
  data: SpaceMapSnapshot | null;
  pending: Promise<SpaceMapSnapshot> | null;
}

export function createSpaceMapCache({
  limit = 32,
  maxAgeMs = 30_000,
  now = Date.now,
}: {
  limit?: number;
  maxAgeMs?: number;
  now?: () => number;
} = {}) {
  const directories = new Map<string, CachedDirectory>();
  let scope = "";

  function peek(request: SpaceMapRequest) {
    if (!request.cacheable || scope !== scopeKey(request)) return null;
    const entry = directories.get(pathKey(request.parent));
    if (!entry?.data) return null;
    const age = now() - entry.savedAt;
    return age >= 0 && age < maxAgeMs ? entry.data : null;
  }

  function touch(key: string, entry: CachedDirectory) {
    directories.delete(key);
    directories.set(key, entry);
    while (directories.size > limit) {
      const oldest = directories.keys().next().value;
      if (oldest === undefined) break;
      directories.delete(oldest);
    }
  }

  function load(request: SpaceMapRequest): Promise<SpaceMapSnapshot> {
    const requestedScope = scopeKey(request);
    if (scope !== requestedScope) {
      scope = requestedScope;
      directories.clear();
    }
    const key = pathKey(request.parent);
    const existing = directories.get(key);
    if (existing?.pending) {
      touch(key, existing);
      return existing.pending;
    }
    const cached = peek(request);
    if (cached && existing) {
      touch(key, existing);
      return Promise.resolve(cached);
    }
    const entry: CachedDirectory = { savedAt: 0, data: null, pending: null };
    touch(key, entry);
    const current = () =>
      scope === requestedScope && directories.get(key) === entry;
    entry.pending = Promise.resolve()
      .then(() =>
        api<SpaceMapSnapshot>("space_map", {
          scanId: request.scanId,
          parent: request.parent,
        }),
      )
      .then((data) => {
        if (
          typeof data?.parent?.path !== "string" ||
          pathKey(data.parent.path) !== key ||
          !Array.isArray(data.items) ||
          !Number.isSafeInteger(data.total) ||
          data.total < 0
        )
          throw new Error("空间地图与当前目录不一致，请重新读取");
        if (current()) {
          if (request.cacheable) {
            entry.data = data;
            entry.savedAt = now();
            entry.pending = null;
          } else directories.delete(key);
        }
        return data;
      })
      .catch((error: unknown) => {
        if (current()) directories.delete(key);
        throw error;
      });
    return entry.pending;
  }

  return {
    peek,
    load,
    invalidate(request: SpaceMapRequest) {
      if (scope === scopeKey(request))
        directories.delete(pathKey(request.parent));
    },
  };
}
