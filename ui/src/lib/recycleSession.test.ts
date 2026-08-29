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

beforeEach(() => call.mockReset());

describe("one-click recycle session", () => {
  it("checks and then immediately executes with risk acknowledgement", async () => {
    const history = [{ status: "recycled" }] as HistoryItem[];
    call.mockResolvedValueOnce(preview()).mockResolvedValueOnce(history);

    const run = open();

    await vi.waitFor(() => expect(run.latest().phase).toBe("complete"));
    expect(call.mock.calls).toEqual([
      ["preview_cleanup", { scanId: "scan-one", entryIds: [7] }],
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
    });

    pending.resolve(preview());
    await duplicate;
    await vi.waitFor(() => expect(run.latest().phase).toBe("complete"));
    expect(call).toHaveBeenCalledTimes(2);
  });

  it("can close during checking and ignores the late result", async () => {
    const pending = deferred<CleanupPreview>();
    call.mockReturnValueOnce(pending.promise);
    const run = open();

    expect(run.session.close()).toBe(true);
    pending.resolve(preview());
    await Promise.resolve();
    await Promise.resolve();

    expect(call).toHaveBeenCalledTimes(1);
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
    expect(run.session.close()).toBe(false);

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
