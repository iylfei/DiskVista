import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import { createHistoryPager, type HistoryPagingState } from "./historyPaging";
import type { HistoryPage } from "./types";

vi.mock("./api", () => ({ api: vi.fn() }));
const call = vi.mocked(api);

function page(offset = 0, total = 47): HistoryPage {
  return {
    total,
    items: Array.from(
      { length: Math.max(0, Math.min(20, total - offset)) },
      (_, index) => ({
        id: `history-${offset + index}`,
        batchId: "batch",
        path: `D:\\历史\\文件-${offset + index}.bin`,
        bytes: 100,
        time: 1,
        status: "recycled",
        message: "已回收",
        freeSpaceDelta: 0,
      }),
    ),
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

function open() {
  const states: HistoryPagingState[] = [];
  const onError = vi.fn();
  const pager = createHistoryPager({
    onState: (state) => states.push(state),
    onError,
  });
  return { pager, states, onError, latest: () => states[states.length - 1] };
}

beforeEach(() => call.mockReset());

describe("history database pagination", () => {
  it("requests only 20 records at a time and can reach beyond the old 500-record limit", async () => {
    call.mockResolvedValueOnce(page(0, 607));
    const run = open();
    expect(run.latest().status).toBe("loading");
    expect(run.latest().total).toBeNull();
    await Promise.resolve();
    expect(call).toHaveBeenCalledExactlyOnceWith("history_page", {
      offset: 0,
      limit: 20,
    });
    expect(run.latest()).toMatchObject({
      page: 0,
      total: 607,
      status: "ready",
    });
    expect(run.latest().items).toHaveLength(20);
    call.mockResolvedValueOnce(page(600, 607));
    await run.pager.load(30);
    expect(call).toHaveBeenLastCalledWith("history_page", {
      offset: 600,
      limit: 20,
    });
    expect(run.latest()).toMatchObject({
      page: 30,
      total: 607,
      status: "ready",
    });
    expect(run.latest().items).toEqual(page(600, 607).items);
    expect(
      call.mock.calls.every(([command]) => command === "history_page"),
    ).toBe(true);
  });

  it.each([false, true])(
    "ignores an older page response after fast navigation (error = %s)",
    async (error) => {
      const older = deferred<HistoryPage>();
      call.mockReturnValueOnce(older.promise).mockResolvedValueOnce(page(20));
      const run = open();
      await run.pager.load(1);
      const count = run.states.length;
      if (error) older.reject(new Error("迟到的错误"));
      else older.resolve(page());
      await Promise.resolve();
      expect(run.states).toHaveLength(count);
      expect(run.latest()).toMatchObject({ page: 1, items: page(20).items });
      expect(run.onError).not.toHaveBeenCalled();
    },
  );

  it.each([false, true])(
    "ignores a response after unmounting (error = %s)",
    async (error) => {
      const pending = deferred<HistoryPage>();
      call.mockReturnValueOnce(pending.promise);
      const run = open();
      run.pager.dispose();
      if (error) pending.reject("视图已关闭");
      else pending.resolve(page());
      await Promise.resolve();
      await run.pager.load(1);
      expect(run.states).toHaveLength(1);
      expect(call).toHaveBeenCalledTimes(1);
      expect(run.onError).not.toHaveBeenCalled();
    },
  );

  it("clears previous rows while loading or failing, then retries the same page", async () => {
    call
      .mockResolvedValueOnce(page())
      .mockRejectedValueOnce(new Error("数据库暂不可用"));
    const run = open();
    await Promise.resolve();
    const loading = run.pager.load(1);
    expect(run.latest()).toMatchObject({
      status: "loading",
      page: 1,
      items: [],
    });
    await loading;
    expect(run.latest()).toMatchObject({
      status: "error",
      page: 1,
      total: 47,
      items: [],
      error: "数据库暂不可用",
    });
    expect(run.onError).toHaveBeenCalledTimes(1);
    call.mockResolvedValueOnce(page(20));
    await run.pager.retry();
    expect(call).toHaveBeenLastCalledWith("history_page", {
      offset: 20,
      limit: 20,
    });
    expect(run.latest()).toMatchObject({
      status: "ready",
      page: 1,
      error: "",
      items: page(20).items,
    });
  });

  it("returns to the last available page when the history count shrinks", async () => {
    call.mockResolvedValueOnce(page(0, 61));
    const run = open();
    await Promise.resolve();
    call
      .mockResolvedValueOnce(page(60, 21))
      .mockResolvedValueOnce(page(20, 21));
    await run.pager.load(3);
    expect(call.mock.calls.slice(1)).toEqual([
      ["history_page", { offset: 60, limit: 20 }],
      ["history_page", { offset: 20, limit: 20 }],
    ]);
    expect(run.latest()).toMatchObject({ page: 1, total: 21, status: "ready" });
    expect(run.latest().items).toHaveLength(1);
    expect(
      run.states.some((state) => state.status === "ready" && state.page === 3),
    ).toBe(false);
  });

  it("shows an empty successful result without additional requests", async () => {
    call.mockResolvedValueOnce(page(0, 0));
    const run = open();
    await Promise.resolve();
    expect(run.latest()).toMatchObject({
      page: 0,
      total: 0,
      items: [],
      status: "ready",
      error: "",
    });
    expect(call).toHaveBeenCalledTimes(1);
  });
});
