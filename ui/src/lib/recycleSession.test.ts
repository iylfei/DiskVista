import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import {
  canExecuteRecycle,
  createRecycleSession,
  type RecycleDialogState,
} from "./recycleSession";
import type { CleanupPreview, HistoryItem } from "./types";

vi.mock("./api", () => ({ api: vi.fn() }));
const call = vi.mocked(api);
const target = { id: 7, isDir: true };
function preview(overrides: Partial<CleanupPreview> = {}): CleanupPreview {
  return {
    id: "preview-one",
    scanId: "scan-one",
    created: 1,
    policyFingerprint: "policy",
    pendingBytes: 128,
    requiresExtraConfirmation: false,
    items: [
      {
        entryId: 7,
        path: "D:\\Fixture\\target",
        bytes: 128,
        fingerprint: "file",
        risk: "low",
        allowed: true,
        reason: "检查通过",
      },
    ],
    ...overrides,
  };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
function open(files = [target]) {
  const states: RecycleDialogState[] = [];
  const onDone = vi.fn(),
    onBusyChange = vi.fn();
  const session = createRecycleSession({
    scanId: "scan-one",
    files,
    onState: (state) => states.push(state),
    onDone,
    onBusyChange,
  });
  return {
    session,
    states,
    latest: () => states[states.length - 1],
    onDone,
    onBusyChange,
  };
}

beforeEach(() => call.mockReset());

describe("confirmed batch recycle session", () => {
  it("opens only a preview and requires explicit directory acknowledgement before executing", async () => {
    call.mockResolvedValueOnce(preview());
    const run = open();
    await Promise.resolve();
    expect(call).toHaveBeenCalledExactlyOnceWith("preview_cleanup", {
      scanId: "scan-one",
      entryIds: [7],
    });
    expect(run.latest().phase).toBe("ready");
    expect(canExecuteRecycle([target], run.latest())).toBe(false);
    await run.session.execute();
    expect(call).toHaveBeenCalledTimes(1);
    run.session.acknowledge(true);
    expect(call).toHaveBeenCalledTimes(1);
    expect(canExecuteRecycle([target], run.latest())).toBe(true);

    const result = [
      { status: "recycled" },
      { status: "skipped" },
      { status: "failed" },
    ] as HistoryItem[];
    call.mockResolvedValueOnce(result);
    await run.session.execute();
    expect(call).toHaveBeenLastCalledWith("execute_cleanup", {
      previewId: "preview-one",
      acknowledgeRisk: true,
    });
    expect(run.onDone).toHaveBeenCalledExactlyOnceWith(result);
    expect(run.onBusyChange.mock.calls).toEqual([[true], [false]]);
    expect(run.latest().phase).toBe("complete");
    await run.session.execute();
    expect(call).toHaveBeenCalledTimes(2);
  });

  it("previews a mixed batch before any execution and passes every outcome through unchanged", async () => {
    const files = [target, { id: 8, isDir: false }, { id: 9, isDir: false }];
    const mixed = preview({
      pendingBytes: 256,
      items: files.map((file, index) => ({
        ...preview().items[0],
        entryId: file.id,
        path: `D:\\Fixture\\${file.id}`,
        allowed: index !== 1,
        reason: index === 1 ? "文件已变化" : "检查通过",
      })),
    });
    call.mockResolvedValueOnce(mixed);
    const run = open(files);
    await Promise.resolve();
    expect(call).toHaveBeenCalledExactlyOnceWith("preview_cleanup", {
      scanId: "scan-one",
      entryIds: [7, 8, 9],
    });
    expect(run.latest().preview).toBe(mixed);
    expect(run.onBusyChange).not.toHaveBeenCalled();
    await run.session.execute();
    expect(call).toHaveBeenCalledTimes(1);
    run.session.acknowledge(true);
    const history = [
      { status: "recycled", path: mixed.items[0].path },
      { status: "skipped", path: mixed.items[1].path },
      { status: "failed", path: mixed.items[2].path },
    ] as HistoryItem[];
    call.mockResolvedValueOnce(history);
    await run.session.execute();
    expect(call).toHaveBeenLastCalledWith("execute_cleanup", {
      previewId: mixed.id,
      acknowledgeRisk: true,
    });
    expect(run.onDone).toHaveBeenCalledExactlyOnceWith(history);
    expect(run.onDone.mock.calls[0][0]).toBe(history);
  });

  it("accepts a server preview that folds selected descendants into their parent", async () => {
    const files = [target, { id: 8, isDir: false }];
    call.mockResolvedValueOnce(preview({ id: "parent-only" }));
    const run = open(files);
    await Promise.resolve();
    expect(run.latest().phase).toBe("ready");
    expect(run.latest().preview?.items.map((item) => item.entryId)).toEqual([
      7,
    ]);
    expect(call).toHaveBeenCalledExactlyOnceWith("preview_cleanup", {
      scanId: "scan-one",
      entryIds: [7, 8],
    });
    run.session.acknowledge(true);
    call.mockResolvedValueOnce([]);
    await run.session.execute();
    expect(call).toHaveBeenLastCalledWith("execute_cleanup", {
      previewId: "parent-only",
      acknowledgeRisk: true,
    });
  });

  it.each(["empty", "over-limit", "duplicate"])(
    "rejects an invalid input batch before calling the backend: %s",
    async (invalid) => {
      const files =
        invalid === "empty"
          ? []
          : invalid === "duplicate"
            ? [target, { ...target }]
            : Array.from({ length: 501 }, (_, index) => ({
                id: index + 1,
                isDir: false,
              }));
      const run = open(files);
      await Promise.resolve();
      expect(run.latest().phase).toBe("error");
      expect(run.latest().error).toContain("1–500");
      await run.session.execute();
      expect(call).not.toHaveBeenCalled();
    },
  );

  it("allows all 500 selected targets without starting execution", async () => {
    const files = Array.from({ length: 500 }, (_, index) => ({
      id: index + 1,
      isDir: false,
    }));
    call.mockResolvedValueOnce(
      preview({
        items: files.map((file) => ({
          ...preview().items[0],
          entryId: file.id,
          path: `D:\\Fixture\\${file.id}`,
        })),
      }),
    );
    const run = open(files);
    await Promise.resolve();
    expect(run.latest().phase).toBe("ready");
    expect(run.latest().preview?.items).toHaveLength(500);
    expect(call).toHaveBeenCalledExactlyOnceWith("preview_cleanup", {
      scanId: "scan-one",
      entryIds: files.map((file) => file.id),
    });
  });

  it("freezes target IDs and directory confirmation at the start of a session", async () => {
    const files = [{ ...target }];
    const pending = deferred<CleanupPreview>();
    call.mockReturnValueOnce(pending.promise);
    const run = open(files);
    files[0].id = 99;
    files[0].isDir = false;
    pending.resolve(preview());
    await Promise.resolve();
    expect(run.latest().phase).toBe("ready");
    await run.session.execute();
    expect(call).toHaveBeenCalledExactlyOnceWith("preview_cleanup", {
      scanId: "scan-one",
      entryIds: [7],
    });
  });

  it.each([true, false])(
    "respects file extra confirmation = %s",
    async (requiresExtraConfirmation) => {
      const file = { id: 7, isDir: false };
      call.mockResolvedValueOnce(preview({ requiresExtraConfirmation }));
      const run = open([file]);
      await Promise.resolve();
      expect(canExecuteRecycle([file], run.latest())).toBe(
        !requiresExtraConfirmation,
      );
      if (requiresExtraConfirmation) {
        await run.session.execute();
        expect(call).toHaveBeenCalledTimes(1);
        run.session.acknowledge(true);
      }
      call.mockResolvedValueOnce([]);
      await run.session.execute();
      expect(call).toHaveBeenLastCalledWith("execute_cleanup", {
        previewId: "preview-one",
        acknowledgeRisk: requiresExtraConfirmation,
      });
      expect(run.onDone).toHaveBeenCalledExactlyOnceWith([]);
    },
  );

  it("blocks duplicate submission, closing and preview refresh while execution is in flight", async () => {
    call.mockResolvedValueOnce(preview());
    const run = open();
    await Promise.resolve();
    run.session.acknowledge(true);
    const pending = deferred<HistoryItem[]>();
    call.mockReturnValueOnce(pending.promise);
    const first = run.session.execute();
    await run.session.execute();
    await run.session.preview();
    expect(run.session.close()).toBe(false);
    expect(run.latest().phase).toBe("executing");
    expect(call).toHaveBeenCalledTimes(2);
    expect(run.onBusyChange.mock.calls).toEqual([[true]]);
    pending.resolve([]);
    await first;
    expect(run.onDone).toHaveBeenCalledTimes(1);
    expect(run.onBusyChange.mock.calls).toEqual([[true], [false]]);
  });

  it("ignores a late preview after closing without ever executing it", async () => {
    const pending = deferred<CleanupPreview>();
    call.mockReturnValueOnce(pending.promise);
    const run = open();
    expect(run.session.close()).toBe(true);
    pending.resolve(preview());
    await Promise.resolve();
    run.session.acknowledge(true);
    await run.session.execute();
    expect(call).toHaveBeenCalledTimes(1);
    expect(run.states).toHaveLength(1);
    expect(run.onDone).not.toHaveBeenCalled();
    expect(run.onBusyChange).not.toHaveBeenCalled();
  });

  it("never replaces a newer preview with an older response", async () => {
    const older = deferred<CleanupPreview>(),
      newer = deferred<CleanupPreview>();
    call.mockReturnValueOnce(older.promise).mockReturnValueOnce(newer.promise);
    const run = open();
    const retry = run.session.preview();
    newer.resolve(preview({ id: "newer" }));
    await retry;
    older.resolve(preview({ id: "older" }));
    await Promise.resolve();
    expect(run.latest().preview?.id).toBe("newer");
    expect(run.latest().acknowledged).toBe(false);
    expect(
      call.mock.calls.every(([command]) => command === "preview_cleanup"),
    ).toBe(true);
  });

  it("shows preview errors locally and retries only the preview", async () => {
    call.mockRejectedValueOnce(new Error("预览检查失败"));
    const run = open();
    await Promise.resolve();
    expect(run.latest()).toMatchObject({
      phase: "error",
      error: "预览检查失败",
      preview: null,
    });
    call.mockResolvedValueOnce(preview());
    await run.session.preview();
    expect(run.latest()).toMatchObject({
      phase: "ready",
      error: "",
      acknowledged: false,
    });
    expect(call.mock.calls.map(([command]) => command)).toEqual([
      "preview_cleanup",
      "preview_cleanup",
    ]);
  });

  it("requires a fresh preview and acknowledgement after execution rejects", async () => {
    call.mockResolvedValueOnce(preview()).mockRejectedValueOnce("文件已变化");
    const run = open();
    await Promise.resolve();
    run.session.acknowledge(true);
    await run.session.execute();
    expect(run.latest()).toMatchObject({
      phase: "error",
      preview: null,
      error: "文件已变化",
      acknowledged: false,
    });
    await run.session.execute();
    expect(call).toHaveBeenCalledTimes(2);
    expect(run.onDone).not.toHaveBeenCalled();
    expect(run.onBusyChange.mock.calls).toEqual([[true], [false]]);
    call.mockResolvedValueOnce(preview({ id: "fresh" }));
    await run.session.preview();
    expect(canExecuteRecycle([target], run.latest())).toBe(false);
    run.session.acknowledge(true);
    call.mockResolvedValueOnce([]);
    await run.session.execute();
    expect(call).toHaveBeenLastCalledWith("execute_cleanup", {
      previewId: "fresh",
      acknowledgeRisk: true,
    });
  });

  it("does not execute a preview without allowed targets", async () => {
    const denied = preview();
    denied.items = denied.items.map((item) => ({
      ...item,
      allowed: false,
      reason: "受保护目录",
    }));
    denied.pendingBytes = 0;
    call.mockResolvedValueOnce(denied);
    const run = open();
    await Promise.resolve();
    run.session.acknowledge(true);
    expect(canExecuteRecycle([target], run.latest())).toBe(false);
    await run.session.execute();
    expect(call).toHaveBeenCalledTimes(1);
  });

  it.each([
    "scan",
    "entry",
    "empty",
    "duplicate",
    "missing-id",
    "missing-items",
    "missing-preview",
  ])("rejects a preview with an invalid target: %s", async (mismatch) => {
    const wrong = preview();
    if (mismatch === "scan") wrong.scanId = "other-scan";
    else if (mismatch === "entry") wrong.items[0].entryId = 99;
    else if (mismatch === "empty") wrong.items = [];
    else if (mismatch === "duplicate") wrong.items.push({ ...wrong.items[0] });
    else if (mismatch === "missing-id") wrong.id = "";
    else if (mismatch === "missing-items")
      wrong.items = undefined as unknown as CleanupPreview["items"];
    call.mockResolvedValueOnce(mismatch === "missing-preview" ? null : wrong);
    const run = open([
      target,
      { id: 8, isDir: false },
      { id: 9, isDir: false },
    ]);
    await Promise.resolve();
    expect(run.latest().phase).toBe("error");
    expect(run.latest().error).toContain("当前清单不一致");
    await run.session.execute();
    expect(call).toHaveBeenCalledTimes(1);
  });

  it("only cancels during execution, sends one request and waits for the actual results", async () => {
    call.mockResolvedValueOnce(preview());
    const run = open();
    await run.session.cancelRemaining();
    expect(call).toHaveBeenCalledTimes(1);
    await Promise.resolve();
    await run.session.cancelRemaining();
    expect(call).toHaveBeenCalledTimes(1);
    run.session.acknowledge(true);
    const pending = deferred<HistoryItem[]>();
    const canceled = deferred<unknown>();
    call
      .mockReturnValueOnce(pending.promise)
      .mockReturnValueOnce(canceled.promise);
    const execution = run.session.execute();
    const cancel = run.session.cancelRemaining();
    await run.session.cancelRemaining();
    expect(call.mock.calls.map(([command]) => command)).toEqual([
      "preview_cleanup",
      "execute_cleanup",
      "cancel_cleanup",
    ]);
    expect(run.latest()).toMatchObject({
      phase: "executing",
      cancelRequested: true,
    });
    expect(run.session.close()).toBe(false);
    canceled.resolve(undefined);
    await cancel;
    expect(run.onBusyChange.mock.calls).toEqual([[true]]);
    expect(run.onDone).not.toHaveBeenCalled();
    const history = [{ status: "skipped" }] as HistoryItem[];
    pending.resolve(history);
    await execution;
    expect(run.onDone).toHaveBeenCalledExactlyOnceWith(history);
    expect(run.onBusyChange.mock.calls).toEqual([[true], [false]]);
    await run.session.cancelRemaining();
    expect(call).toHaveBeenCalledTimes(3);
  });

  it("keeps execution locked when cancellation fails and only retries cancellation", async () => {
    call.mockResolvedValueOnce(preview());
    const run = open();
    await Promise.resolve();
    run.session.acknowledge(true);
    const pending = deferred<HistoryItem[]>();
    call
      .mockReturnValueOnce(pending.promise)
      .mockRejectedValueOnce(new Error("取消暂不可用"));
    const execution = run.session.execute();
    await run.session.cancelRemaining();
    expect(run.latest()).toMatchObject({
      phase: "executing",
      cancelRequested: false,
      cancelError: "取消暂不可用",
      error: "",
    });
    expect(run.session.close()).toBe(false);
    expect(run.onBusyChange.mock.calls).toEqual([[true]]);
    call.mockResolvedValueOnce(undefined);
    await run.session.cancelRemaining();
    expect(run.latest()).toMatchObject({
      phase: "executing",
      cancelRequested: true,
      cancelError: "",
    });
    expect(call.mock.calls.map(([command]) => command)).toEqual([
      "preview_cleanup",
      "execute_cleanup",
      "cancel_cleanup",
      "cancel_cleanup",
    ]);
    pending.resolve([]);
    await execution;
  });

  it("ignores a late cancellation error after execution has already finished", async () => {
    call.mockResolvedValueOnce(preview());
    const run = open();
    await Promise.resolve();
    run.session.acknowledge(true);
    const pending = deferred<HistoryItem[]>();
    const cancellation = deferred<unknown>();
    call
      .mockReturnValueOnce(pending.promise)
      .mockReturnValueOnce(cancellation.promise);
    const execution = run.session.execute();
    const cancel = run.session.cancelRemaining();
    pending.resolve([]);
    await execution;
    const count = run.states.length;
    cancellation.reject(new Error("迟到的取消失败"));
    await cancel;
    expect(run.states).toHaveLength(count);
    expect(run.latest().phase).toBe("complete");
    expect(run.onDone).toHaveBeenCalledExactlyOnceWith([]);
    expect(run.onBusyChange.mock.calls).toEqual([[true], [false]]);
  });

  it("does not drop completed history or unlock early if its view unmounts during execution", async () => {
    call.mockResolvedValueOnce(preview());
    const run = open();
    await Promise.resolve();
    run.session.acknowledge(true);
    const pending = deferred<HistoryItem[]>();
    call.mockReturnValueOnce(pending.promise);
    const execution = run.session.execute();
    const stateCount = run.states.length;
    run.session.dispose();
    expect(run.onBusyChange.mock.calls).toEqual([[true]]);
    const history = [{ status: "skipped" }] as HistoryItem[];
    pending.resolve(history);
    await execution;
    expect(run.states).toHaveLength(stateCount);
    expect(run.onDone).toHaveBeenCalledExactlyOnceWith(history);
    expect(run.onBusyChange.mock.calls).toEqual([[true], [false]]);
  });
});
