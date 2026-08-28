import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import {
  createSpaceMapCache,
  spaceMapRequestKey,
  type SpaceMapRequest,
  type SpaceMapSnapshot,
} from "./spaceMapData";
import type { FileRecord } from "./types";

vi.mock("./api", () => ({ api: vi.fn() }));
const call = vi.mocked(api);
const query: SpaceMapRequest = {
  scanId: "scan",
  parent: "D:\\Map",
  revision: "complete:0",
  cacheable: true,
};

function snapshot(parent = query.parent, bytes = 100): SpaceMapSnapshot {
  return {
    parent: {
      id: 1,
      path: parent,
      isDir: true,
      logicalBytes: bytes,
    } as FileRecord,
    items: [],
    total: 0,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

beforeEach(() => call.mockReset());

describe("space map directory cache", () => {
  it("shares an in-flight request and reuses a completed directory without another RPC", async () => {
    const pending = deferred<SpaceMapSnapshot>();
    call.mockReturnValueOnce(pending.promise);
    const cache = createSpaceMapCache();
    const one = cache.load(query);
    const two = cache.load(query);
    expect(one).toBe(two);
    expect(cache.peek(query)).toBeNull();
    await Promise.resolve();
    expect(call).toHaveBeenCalledExactlyOnceWith("space_map", {
      scanId: "scan",
      parent: "D:\\Map",
    });
    const result = snapshot();
    pending.resolve(result);
    expect(await one).toBe(result);
    expect(await two).toBe(result);
    expect(await cache.load(query)).toBe(result);
    expect(cache.peek(query)).toBe(result);
    expect(call).toHaveBeenCalledTimes(1);
  });

  it("keeps separate directories correct when they finish in reverse order", async () => {
    const a = deferred<SpaceMapSnapshot>();
    const b = deferred<SpaceMapSnapshot>();
    call.mockReturnValueOnce(a.promise).mockReturnValueOnce(b.promise);
    const cache = createSpaceMapCache();
    const next = { ...query, parent: "D:\\Map\\Nested" };
    const first = cache.load(query);
    const second = cache.load(next);
    b.resolve(snapshot(next.parent));
    await second;
    a.resolve(snapshot());
    await first;
    expect(cache.peek(query)?.parent.path).toBe(query.parent);
    expect(cache.peek(next)?.parent.path).toBe(next.parent);
  });

  it("keys paths like the Windows index and never mixes scans or revisions", async () => {
    const alias = { ...query, parent: "\\\\?\\d:/MAP/" };
    expect(spaceMapRequestKey(alias)).toBe(spaceMapRequestKey(query));
    call.mockResolvedValueOnce(snapshot());
    const cache = createSpaceMapCache();
    await cache.load(query);
    expect(await cache.load(alias)).toEqual(snapshot());
    expect(call).toHaveBeenCalledTimes(1);
    expect(cache.peek({ ...query, scanId: "other" })).toBeNull();
    expect(cache.peek({ ...query, revision: "complete:1" })).toBeNull();
    expect(cache.peek({ ...query, parent: "D:\\Map-copy" })).toBeNull();
  });

  it("invalidates old scope responses even after returning to the same scope", async () => {
    const old = deferred<SpaceMapSnapshot>();
    const fresh = deferred<SpaceMapSnapshot>();
    call
      .mockReturnValueOnce(old.promise)
      .mockResolvedValueOnce(snapshot(query.parent, 200))
      .mockReturnValueOnce(fresh.promise);
    const cache = createSpaceMapCache();
    const first = cache.load(query);
    await Promise.resolve();
    const changed = { ...query, revision: "complete:1" };
    await cache.load(changed);
    expect(cache.peek(query)).toBeNull();
    const latest = cache.load(query);
    old.resolve(snapshot(query.parent, 50));
    await first;
    expect(cache.peek(query)).toBeNull();
    fresh.resolve(snapshot(query.parent, 300));
    await latest;
    expect(cache.peek(query)?.parent.logicalBytes).toBe(300);
  });

  it("evicts the least recently visited directory at the cache limit", async () => {
    call.mockImplementation(async (_command, args) =>
      snapshot(args?.parent as string),
    );
    const cache = createSpaceMapCache({ limit: 2 });
    const b = { ...query, parent: "D:\\B" };
    const c = { ...query, parent: "D:\\C" };
    await cache.load(query);
    await cache.load(b);
    await cache.load(query);
    await cache.load(c);
    expect(cache.peek(query)).not.toBeNull();
    expect(cache.peek(b)).toBeNull();
    expect(cache.peek(c)).not.toBeNull();
    expect(call).toHaveBeenCalledTimes(3);
  });

  it("expires cached metadata after 30 seconds or a clock rollback", async () => {
    let time = 100_000;
    call.mockResolvedValue(snapshot());
    const cache = createSpaceMapCache({ now: () => time });
    await cache.load(query);
    time += 29_999;
    await cache.load(query);
    expect(call).toHaveBeenCalledTimes(1);
    time += 1;
    expect(cache.peek(query)).toBeNull();
    await cache.load(query);
    expect(call).toHaveBeenCalledTimes(2);
    time -= 1;
    expect(cache.peek(query)).toBeNull();
    await cache.load(query);
    expect(call).toHaveBeenCalledTimes(3);
  });

  it("does not retain unfinished scan results but still merges concurrent reads", async () => {
    call.mockResolvedValue(snapshot());
    const cache = createSpaceMapCache();
    const active = { ...query, revision: "scanning:0", cacheable: false };
    const first = cache.load(active);
    expect(cache.load(active)).toBe(first);
    await first;
    expect(cache.peek(active)).toBeNull();
    await cache.load(active);
    expect(call).toHaveBeenCalledTimes(2);
  });

  it("does not cache failures and allows an explicit retry", async () => {
    call
      .mockRejectedValueOnce(new Error("读取失败"))
      .mockResolvedValueOnce(snapshot());
    const cache = createSpaceMapCache();
    await expect(cache.load(query)).rejects.toThrow("读取失败");
    expect(cache.peek(query)).toBeNull();
    await expect(cache.load(query)).resolves.toEqual(snapshot());
    expect(call).toHaveBeenCalledTimes(2);
  });

  it.each([false, true])(
    "keeps a replacement request after invalidating an old request (error = %s)",
    async (error) => {
      const old = deferred<SpaceMapSnapshot>();
      const fresh = deferred<SpaceMapSnapshot>();
      call.mockReturnValueOnce(old.promise).mockReturnValueOnce(fresh.promise);
      const cache = createSpaceMapCache();
      const first = cache.load(query);
      const settled = first.catch(() => undefined);
      await Promise.resolve();
      cache.invalidate(query);
      const latest = cache.load(query);
      if (error) old.reject(new Error("旧请求错误"));
      else old.resolve(snapshot(query.parent, 10));
      await settled;
      expect(cache.load(query)).toBe(latest);
      fresh.resolve(snapshot(query.parent, 20));
      await latest;
      expect(cache.peek(query)?.parent.logicalBytes).toBe(20);
    },
  );

  it.each([
    { ...snapshot(), parent: { path: "D:\\Wrong" } },
    { ...snapshot(), items: null },
    { ...snapshot(), total: -1 },
  ])(
    "rejects an invalid or mismatched directory response",
    async (response) => {
      call.mockResolvedValueOnce(response).mockResolvedValueOnce(snapshot());
      const cache = createSpaceMapCache();
      await expect(cache.load(query)).rejects.toThrow("当前目录不一致");
      expect(cache.peek(query)).toBeNull();
      await expect(cache.load(query)).resolves.toEqual(snapshot());
    },
  );
});
