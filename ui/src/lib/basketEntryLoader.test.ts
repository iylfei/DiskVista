import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import { createBasketEntryLoader } from "./basketEntryLoader";
import type { FileRecord, HistoryItem } from "./types";

vi.mock("./api", () => ({ api: vi.fn() }));
const call = vi.mocked(api);

function file(id = 1): FileRecord {
  return {
    id,
    path: `D:\\App\\Cache\\${id}`,
    logicalBytes: 100,
    allocatedBytes: 100,
    isDir: true,
    complete: true,
    hasBlockedChildren: false,
    assessment: { risk: "review" },
  } as FileRecord;
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

function open() {
  const onAdd = vi.fn();
  const onPending = vi.fn();
  const onError = vi.fn();
  const loader = createBasketEntryLoader({ onAdd, onPending, onError });
  return { loader, onAdd, onPending, onError };
}

beforeEach(() => call.mockReset());

describe("basket entry loading", () => {
  it("reads the full target without previewing or executing cleanup", async () => {
    const target = file();
    call.mockResolvedValueOnce(target);
    const run = open();
    await run.loader.add("scan", target.id);
    expect(call).toHaveBeenCalledExactlyOnceWith("entry_detail", {
      scanId: "scan",
      entryId: 1,
    });
    expect(run.onAdd).toHaveBeenCalledExactlyOnceWith(target);
    expect(run.onPending.mock.calls.map(([ids]) => [...ids])).toEqual([
      [1],
      [],
    ]);
    expect(run.onError).not.toHaveBeenCalled();
  });

  it("keeps independent requests while ignoring duplicate clicks", async () => {
    const first = deferred<FileRecord>();
    const second = deferred<FileRecord>();
    call.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
    const run = open();
    const one = run.loader.add("scan", 1);
    const two = run.loader.add("scan", 2);
    await run.loader.add("scan", 1);
    expect(call).toHaveBeenCalledTimes(2);
    second.resolve(file(2));
    await two;
    expect(run.onPending).toHaveBeenLastCalledWith(new Set([1]));
    first.resolve(file(1));
    await one;
    expect(run.onAdd.mock.calls.map(([target]) => target.id)).toEqual([2, 1]);
    expect(run.onPending).toHaveBeenLastCalledWith(new Set());
  });

  it.each([false, true])(
    "ignores old responses after a scan change without clearing new pending state (error = %s)",
    async (error) => {
      const old = deferred<FileRecord>();
      const current = deferred<FileRecord>();
      call
        .mockReturnValueOnce(old.promise)
        .mockReturnValueOnce(current.promise);
      const run = open();
      const one = run.loader.add("old-scan", 1);
      run.loader.reset();
      const two = run.loader.add("new-scan", 1);
      const notifications = run.onPending.mock.calls.length;
      if (error) old.reject(new Error("旧扫描的读取失败"));
      else old.resolve(file());
      await one;
      expect(run.onAdd).not.toHaveBeenCalled();
      expect(run.onError).not.toHaveBeenCalled();
      expect(run.onPending).toHaveBeenCalledTimes(notifications);
      current.resolve({ ...file(), path: "E:\\New\\1" });
      await two;
      expect(run.onAdd).toHaveBeenCalledTimes(1);
      expect(run.onAdd.mock.calls[0][0].path).toBe("E:\\New\\1");
      expect(run.onPending).toHaveBeenLastCalledWith(new Set());
    },
  );

  it("ignores a request after the basket is cleared", async () => {
    const pending = deferred<FileRecord>();
    call.mockReturnValueOnce(pending.promise);
    const run = open();
    const added = run.loader.add("scan", 1);
    run.loader.reset();
    pending.resolve(file());
    await added;
    expect(run.onAdd).not.toHaveBeenCalled();
    expect(run.onPending).toHaveBeenLastCalledWith(new Set());
  });

  it("does not re-add a recycled directory descendant from a late detail response", async () => {
    const pending = deferred<FileRecord>();
    call.mockReturnValueOnce(pending.promise);
    const run = open();
    const added = run.loader.add("scan", 1);
    run.loader.discardRecycled([
      { status: "recycled", path: "d:/APP/cache/", snapshot: { isDir: true } },
    ] as HistoryItem[]);
    pending.resolve(file());
    await added;
    expect(run.onAdd).not.toHaveBeenCalled();
    expect(run.onError).not.toHaveBeenCalled();
    expect(run.onPending).toHaveBeenLastCalledWith(new Set());
  });

  it("only discards requests covered by a successful recycle result", async () => {
    const one = deferred<FileRecord>();
    const two = deferred<FileRecord>();
    const three = deferred<FileRecord>();
    call
      .mockReturnValueOnce(one.promise)
      .mockReturnValueOnce(two.promise)
      .mockReturnValueOnce(three.promise);
    const run = open();
    const requests = [1, 2, 3].map((id) => run.loader.add("scan", id));
    run.loader.discardRecycled([
      { status: "recycled", path: file(1).path, snapshot: { isDir: false } },
      { status: "failed", path: file(2).path },
      { status: "skipped", path: file(3).path },
    ] as HistoryItem[]);
    one.resolve(file(1));
    two.resolve(file(2));
    three.resolve(file(3));
    await Promise.all(requests);
    expect(run.onAdd.mock.calls.map(([target]) => target.id)).toEqual([2, 3]);
  });

  it.each([
    ["mismatched ID", { ...file(), id: 2 }],
    ["protected target", { ...file(), assessment: { risk: "protected" } }],
    ["incomplete target", { ...file(), complete: false }],
    ["blocked descendants", { ...file(), hasBlockedChildren: true }],
  ])("rejects %s and permits a fresh retry", async (_label, target) => {
    call.mockResolvedValueOnce(target).mockResolvedValueOnce(file());
    const run = open();
    await run.loader.add("scan", 1);
    expect(run.onAdd).not.toHaveBeenCalled();
    expect(run.onError).toHaveBeenCalledTimes(1);
    expect(run.onPending).toHaveBeenLastCalledWith(new Set());
    await run.loader.add("scan", 1);
    expect(call).toHaveBeenCalledTimes(2);
    expect(run.onAdd).toHaveBeenCalledExactlyOnceWith(file());
  });
});
