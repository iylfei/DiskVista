import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import {
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
    requiresExtraConfirmation: true,
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
  const onDone = vi.fn();
  const onBusyChange = vi.fn();
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

beforeEach(() => {
  call.mockReset();
});

describe("one-click recycle session", () => {
  it("checks and then immediately executes with risk acknowledgement", async () => {
    const history = [{ status: "recycled" }] as HistoryItem[];
    call.mockResolvedValueOnce(preview()).mockResolvedValueOnce(history);

    const run = open();

    await vi.waitFor(() => expect(run.latest().phase).toBe("complete"));
    expect(call.mock.calls).toEqual([
      [
        "preview_cleanup",
        { scanId: "scan-one", entryIds: [7], requestId: expect.any(String) },
      ],
      ["execute_cleanup", { previewId: "preview-one", acknowledgeRisk: true }],
    ]);
    expect(run.onBusyChange.mock.calls).toEqual([[true], [false]]);
    expect(run.onDone).toHaveBeenCalledExactlyOnceWith(history);

    await run.session.run();
    expect(call).toHaveBeenCalledTimes(2);
  });

  it("executes allowed targets from a mixed, deduplicated preview", async () => {
    const files = [target, { id: 8, isDir: false }, { id: 9, isDir: false }];
    const mixed = preview({
      id: "mixed-preview",
      items: [
        preview().items[0],
        {
          ...preview().items[0],
          entryId: 9,
          path: "D:\\Fixture\\blocked",
          allowed: false,
          reason: "文件已变化",
        },
      ],
    });
    const history = [
      { status: "recycled", path: mixed.items[0].path },
      { status: "skipped", path: mixed.items[1].path },
    ] as HistoryItem[];
    call.mockResolvedValueOnce(mixed).mockResolvedValueOnce(history);

    const run = open(files);

    await vi.waitFor(() => expect(run.onDone).toHaveBeenCalledOnce());
    expect(call).toHaveBeenNthCalledWith(1, "preview_cleanup", {
      scanId: "scan-one",
      entryIds: [7, 8, 9],
      requestId: expect.any(String),
    });
    expect(call).toHaveBeenNthCalledWith(2, "execute_cleanup", {
      previewId: "mixed-preview",
      acknowledgeRisk: true,
    });
    expect(run.onDone).toHaveBeenCalledWith(history);
  });

  it("does not execute when every target is blocked and includes refusal details", async () => {
    call.mockResolvedValueOnce(
      preview({
        pendingBytes: 0,
        items: [
          {
            ...preview().items[0],
            allowed: false,
            reason: "受保护目录",
          },
        ],
      }),
    );

    const run = open();

    await vi.waitFor(() => expect(run.latest().phase).toBe("error"));
    expect(run.latest().error).toContain("没有项目通过安全检查");
    expect(run.latest().error).toContain("D:\\Fixture\\target：受保护目录");
    expect(call).toHaveBeenCalledExactlyOnceWith("preview_cleanup", {
      scanId: "scan-one",
      entryIds: [7],
      requestId: expect.any(String),
    });
    expect(run.onBusyChange).not.toHaveBeenCalled();
    expect(run.onDone).not.toHaveBeenCalled();
  });

  it.each([
    "scan",
    "entry",
    "empty",
    "duplicate",
    "missing-id",
    "missing-items",
    "missing-preview",
  ])("rejects an inconsistent check result: %s", async (mismatch) => {
    const wrong = preview();
    if (mismatch === "scan") wrong.scanId = "other-scan";
    else if (mismatch === "entry") wrong.items[0].entryId = 99;
    else if (mismatch === "empty") wrong.items = [];
    else if (mismatch === "duplicate") wrong.items.push({ ...wrong.items[0] });
    else if (mismatch === "missing-id") wrong.id = "";
    else if (mismatch === "missing-items")
      wrong.items = undefined as unknown as CleanupPreview["items"];
    call.mockResolvedValueOnce(mismatch === "missing-preview" ? null : wrong);

    const run = open([target, { id: 8, isDir: false }]);

    await vi.waitFor(() => expect(run.latest().phase).toBe("error"));
    expect(run.latest().error).toContain("当前清单不一致");
    expect(call).toHaveBeenCalledTimes(1);
    expect(run.onBusyChange).not.toHaveBeenCalled();
  });

  it.each(["empty", "over-limit", "duplicate"])(
    "rejects an invalid target list before checking: %s",
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

      await vi.waitFor(() => expect(run.latest().phase).toBe("error"));
      expect(run.latest().error).toContain("1–500");
      expect(call).not.toHaveBeenCalled();
    },
  );

  it("ignores duplicate runs while a check is pending", async () => {
    const pending = deferred<CleanupPreview>();
    call.mockReturnValueOnce(pending.promise).mockResolvedValueOnce([]);
    const run = open();

    const duplicate = run.session.run();
    await run.session.run();
    expect(call).toHaveBeenCalledExactlyOnceWith("preview_cleanup", {
      scanId: "scan-one",
      entryIds: [7],
      requestId: expect.any(String),
    });

    pending.resolve(preview());
    await duplicate;
    await vi.waitFor(() => expect(run.latest().phase).toBe("complete"));
    expect(call).toHaveBeenCalledTimes(2);
  });

  it("can close during checking and ignores the late result", async () => {
    const pending = deferred<CleanupPreview>();
    call.mockReturnValueOnce(pending.promise).mockResolvedValueOnce(undefined);
    const run = open();

    const closed = run.session.close();
    expect(run.latest().cancelRequested).toBe(true);
    pending.resolve(preview());
    expect(await closed).toBe(true);
    await Promise.resolve();
    await Promise.resolve();

    expect(call).toHaveBeenCalledTimes(2);
    expect(call).toHaveBeenLastCalledWith("cancel_cleanup_check", {
      requestId: call.mock.calls[0][1]?.requestId,
    });
    expect(run.onBusyChange).not.toHaveBeenCalled();
    expect(run.onDone).not.toHaveBeenCalled();
  });

  it("performs a fresh check before retrying a failed execution", async () => {
    call
      .mockResolvedValueOnce(preview({ id: "stale" }))
      .mockRejectedValueOnce(new Error("文件已变化"));
    const run = open();
    await vi.waitFor(() => expect(run.latest().phase).toBe("error"));
    expect(run.latest().error).toBe("文件已变化");

    const history = [{ status: "recycled" }] as HistoryItem[];
    call
      .mockResolvedValueOnce(preview({ id: "fresh" }))
      .mockResolvedValueOnce(history);
    await run.session.run();

    expect(call.mock.calls.map(([command]) => command)).toEqual([
      "preview_cleanup",
      "execute_cleanup",
      "preview_cleanup",
      "execute_cleanup",
    ]);
    expect(call).toHaveBeenLastCalledWith("execute_cleanup", {
      previewId: "fresh",
      acknowledgeRisk: true,
    });
    expect(run.onDone).toHaveBeenCalledExactlyOnceWith(history);
    expect(run.onBusyChange.mock.calls).toEqual([
      [true],
      [false],
      [true],
      [false],
    ]);
  });

  it("cancels remaining work once and waits for the execution result", async () => {
    const execution = deferred<HistoryItem[]>();
    const cancellation = deferred<unknown>();
    call
      .mockResolvedValueOnce(preview())
      .mockReturnValueOnce(execution.promise)
      .mockReturnValueOnce(cancellation.promise);
    const run = open();
    await vi.waitFor(() => expect(call).toHaveBeenCalledTimes(2));

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
    expect(await run.session.close()).toBe(false);

    cancellation.resolve(undefined);
    await cancel;
    const history = [{ status: "skipped" }] as HistoryItem[];
    execution.resolve(history);
    await vi.waitFor(() => expect(run.onDone).toHaveBeenCalledOnce());
    expect(run.onDone).toHaveBeenCalledWith(history);
    expect(run.onBusyChange.mock.calls).toEqual([[true], [false]]);
  });

  it("allows cancellation to be retried after a cancellation error", async () => {
    const execution = deferred<HistoryItem[]>();
    call
      .mockResolvedValueOnce(preview())
      .mockReturnValueOnce(execution.promise)
      .mockRejectedValueOnce(new Error("取消暂不可用"))
      .mockResolvedValueOnce(undefined);
    const run = open();
    await vi.waitFor(() => expect(call).toHaveBeenCalledTimes(2));

    await run.session.cancelRemaining();
    expect(run.latest()).toMatchObject({
      phase: "executing",
      cancelRequested: false,
      cancelError: "取消暂不可用",
    });
    await run.session.cancelRemaining();
    expect(run.latest()).toMatchObject({
      phase: "executing",
      cancelRequested: true,
      cancelError: "",
    });

    execution.resolve([]);
    await vi.waitFor(() => expect(run.onDone).toHaveBeenCalledOnce());
  });

  it("still reports completion and clears busy state after disposal", async () => {
    const execution = deferred<HistoryItem[]>();
    call
      .mockResolvedValueOnce(preview())
      .mockReturnValueOnce(execution.promise);
    const run = open();
    await vi.waitFor(() => expect(run.onBusyChange).toHaveBeenCalledWith(true));
    const stateCount = run.states.length;

    run.session.dispose();
    const history = [{ status: "recycled" }] as HistoryItem[];
    execution.resolve(history);

    await vi.waitFor(() => expect(run.onDone).toHaveBeenCalledOnce());
    expect(run.states).toHaveLength(stateCount);
    expect(run.onDone).toHaveBeenCalledWith(history);
    expect(run.onBusyChange.mock.calls).toEqual([[true], [false]]);
  });
});

describe("cancellable check progress", () => {
  it("reports progress and rejects late progress after closing starts", async () => {
    vi.useFakeTimers();
    try {
      const pending = deferred<CleanupPreview>();
      const late = deferred<unknown>();
      let reads = 0;
      call.mockImplementation((command) => {
        if (command === "preview_cleanup") return pending.promise;
        if (command === "cleanup_check_progress") {
          reads += 1;
          return reads === 1
            ? Promise.resolve({
                stage: "filesystem",
                targetsDone: 0,
                targetsTotal: 1,
                currentPath: "D:\\Fixture",
                checkedEntries: 64,
              })
            : late.promise;
        }
        if (command === "cancel_cleanup_check")
          return Promise.resolve(undefined);
        throw Error(`Unexpected ${command}`);
      });
      const run = open();
      await vi.advanceTimersByTimeAsync(200);
      expect(run.latest().checkProgress?.checkedEntries).toBe(64);
      await vi.advanceTimersByTimeAsync(200);
      const closed = run.session.close();
      late.resolve({
        stage: "complete",
        targetsDone: 1,
        targetsTotal: 1,
        checkedEntries: 999,
      });
      await vi.advanceTimersByTimeAsync(200);
      expect(run.latest().checkProgress?.checkedEntries).toBe(64);
      pending.reject(new Error("cancelled"));
      expect(await closed).toBe(true);
      await vi.advanceTimersByTimeAsync(1000);
      expect(reads).toBe(2);
      expect(call.mock.calls.some(([name]) => name === "execute_cleanup")).toBe(
        false,
      );
    } finally {
      vi.useRealTimers();
    }
  });

  it("keeps the dialog after a failed cancellation and waits for backend exit on retry", async () => {
    const pending = deferred<CleanupPreview>();
    let attempts = 0;
    call.mockImplementation((command) => {
      if (command === "preview_cleanup") return pending.promise;
      if (command === "cancel_cleanup_check")
        return ++attempts === 1
          ? Promise.reject(new Error("IPC unavailable"))
          : Promise.resolve(undefined);
      throw Error(`Unexpected ${command}`);
    });
    const run = open();
    expect(await run.session.close()).toBe(false);
    expect(run.latest().cancelError).toBe("IPC unavailable");
    let returned = false;
    const closed = run.session.close().then((value) => {
      returned = true;
      return value;
    });
    await Promise.resolve();
    await Promise.resolve();
    expect(returned).toBe(false);
    pending.reject(new Error("cancelled"));
    expect(await closed).toBe(true);
    expect(attempts).toBe(2);
    expect(run.onDone).not.toHaveBeenCalled();
  });

  it("unmount cancellation only targets the abandoned request", async () => {
    const pending = deferred<CleanupPreview>();
    call.mockReturnValueOnce(pending.promise).mockResolvedValue(undefined);
    const run = open();
    const requestId = call.mock.calls[0][1]?.requestId;
    run.session.dispose();
    expect(call).toHaveBeenLastCalledWith("cancel_cleanup_check", {
      requestId,
    });
    pending.resolve(preview());
    await Promise.resolve();
    expect(call.mock.calls.some(([name]) => name === "execute_cleanup")).toBe(
      false,
    );
  });
});
