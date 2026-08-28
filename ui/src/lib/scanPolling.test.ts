import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import { sameSnapshot, startScanPolling } from "./scanPolling";
import { startAnalysisPolling } from "./analysisPolling";
import { startAnalysisSummaryLoad } from "./analysisSummaries";
import { mergeFileDetail, startFileDetailLoad } from "./fileDetail";
import type { FileRecord, RuntimeStatus } from "./types";

vi.mock("./api", () => ({ api: vi.fn() }));
const status = {
  scan: { id: "s", status: "complete" },
  scans: [],
  analysisProgress: { active: false },
} as unknown as RuntimeStatus;
function fixture() {
  let overview = true;
  const callbacks = {
    onStatus: vi.fn(),
    onBootstrap: vi.fn(),
    onError: vi.fn(),
  };
  const stop = startScanPolling({
    scanId: "s",
    intervalMs: 4000,
    isOverview: () => overview,
    ...callbacks,
  });
  return {
    ...callbacks,
    stop,
    setOverview: (value: boolean) => {
      overview = value;
    },
  };
}
beforeEach(() => {
  vi.useFakeTimers();
  vi.mocked(api).mockReset().mockResolvedValue(status);
});
afterEach(() => {
  vi.clearAllTimers();
  vi.useRealTimers();
});

describe("file detail response merging", () => {
  const target = { scanId: "original-scan", entryId: 1 };
  const original = { id: 1, name: "original" } as FileRecord;
  const full = { ...original, name: "full detail" };

  it("updates the file that is still open in the requested scan", () => {
    expect(mergeFileDetail(original, target.scanId, target, full)).toBe(full);
  });

  it("does not reopen a closed detail panel after a delayed response", () => {
    expect(mergeFileDetail(null, target.scanId, target, full)).toBeNull();
  });

  it("does not replace a different file selected while loading", () => {
    const current = { ...original, id: 2 };
    expect(mergeFileDetail(current, target.scanId, target, full)).toBe(current);
  });

  it("ignores the previous scan even when its entry id is reused", () => {
    const current = { ...original, name: "another scan" };
    expect(mergeFileDetail(current, "new-scan", target, full)).toBe(current);
    expect(mergeFileDetail(null, null, target, full)).toBeNull();
  });

  it("does not accept a response for a different entry", () => {
    expect(
      mergeFileDetail(original, target.scanId, target, { ...full, id: 2 }),
    ).toBe(original);
  });
});

describe("application file detail loading", () => {
  it("only opens the latest requested file when responses arrive out of order", async () => {
    let resolveFirst!: (file: FileRecord) => void;
    const first = { id: 1 } as FileRecord;
    const latest = { id: 2 } as FileRecord;
    vi.mocked(api)
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveFirst = resolve;
        }),
      )
      .mockResolvedValueOnce(latest);
    const onResult = vi.fn();
    const onError = vi.fn();
    const cancel = startFileDetailLoad({
      scanId: "s",
      entryId: first.id,
      onResult,
      onError,
    });
    cancel();
    startFileDetailLoad({
      scanId: "s",
      entryId: latest.id,
      onResult,
      onError,
    });
    await vi.advanceTimersByTimeAsync(0);
    resolveFirst(first);
    await vi.advanceTimersByTimeAsync(0);
    expect(onResult).toHaveBeenCalledTimes(1);
    expect(onResult).toHaveBeenCalledWith(latest);
    expect(api).toHaveBeenCalledTimes(2);
    expect(api).toHaveBeenLastCalledWith("entry_detail", {
      scanId: "s",
      entryId: latest.id,
    });
  });

  it("ignores a response after leaving the scan or application page", async () => {
    let resolve!: (file: FileRecord) => void;
    vi.mocked(api).mockReturnValueOnce(
      new Promise((done) => {
        resolve = done;
      }),
    );
    const onResult = vi.fn();
    const cancel = startFileDetailLoad({
      scanId: "old-scan",
      entryId: 1,
      onResult,
      onError: vi.fn(),
    });
    cancel();
    resolve({ id: 1 } as FileRecord);
    await vi.advanceTimersByTimeAsync(0);
    expect(onResult).not.toHaveBeenCalled();
  });

  it("reports active failures but suppresses errors from cancelled inspections", async () => {
    const error = new Error("detail read failed");
    vi.mocked(api).mockRejectedValue(error);
    const onError = vi.fn();
    const options = {
      scanId: "s",
      entryId: 1,
      onResult: vi.fn(),
      onError,
    };
    const cancel = startFileDetailLoad(options);
    cancel();
    await vi.advanceTimersByTimeAsync(0);
    expect(onError).not.toHaveBeenCalled();
    startFileDetailLoad(options);
    await vi.advanceTimersByTimeAsync(0);
    expect(onError).toHaveBeenCalledOnce();
    expect(onError).toHaveBeenCalledWith(error);
  });
});

describe("file analysis summary loading", () => {
  it("loads a page in one batch and skips empty pages", async () => {
    vi.mocked(api).mockResolvedValue([]);
    const onResults = vi.fn();
    startAnalysisSummaryLoad({
      scanId: "s",
      entryIds: [1, 2, 2, 3],
      onResults,
      onError: vi.fn(),
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(api).toHaveBeenCalledTimes(1);
    expect(api).toHaveBeenCalledWith("analysis_summaries", {
      scanId: "s",
      entryIds: [1, 2, 3],
    });
    startAnalysisSummaryLoad({
      scanId: "s",
      entryIds: [],
      onResults,
      onError: vi.fn(),
    });
    expect(api).toHaveBeenCalledTimes(1);
    expect(onResults).toHaveBeenLastCalledWith([]);
  });

  it("ignores a response after switching the scan or page", async () => {
    let resolve!: (value: unknown[]) => void;
    vi.mocked(api).mockReturnValue(
      new Promise((done) => {
        resolve = done;
      }),
    );
    const onResults = vi.fn();
    const stop = startAnalysisSummaryLoad({
      scanId: "old",
      entryIds: [1],
      onResults,
      onError: vi.fn(),
    });
    stop();
    resolve([{ entryId: 1 }]);
    await vi.advanceTimersByTimeAsync(0);
    expect(onResults).not.toHaveBeenCalled();
  });
});

describe("analysis result polling", () => {
  it("checks idle results once and does not keep polling", async () => {
    vi.mocked(api).mockResolvedValue([]);
    const stop = startAnalysisPolling({
      scanId: "s",
      entryId: 1,
      active: false,
      onResults: vi.fn(),
      onError: vi.fn(),
    });
    await vi.advanceTimersByTimeAsync(30_000);
    expect(api).toHaveBeenCalledTimes(1);
    expect(api).toHaveBeenCalledWith("analysis_results", {
      scanId: "s",
      entryId: 1,
      revalidate: true,
    });
    stop();
  });
  it("uses lightweight reads during analysis, then revalidates once when it finishes", async () => {
    vi.mocked(api).mockResolvedValue([]);
    const options = {
      scanId: "s",
      entryId: 1,
      onResults: vi.fn(),
      onError: vi.fn(),
    };
    const stop = startAnalysisPolling({ ...options, active: true });
    await vi.advanceTimersByTimeAsync(9000);
    expect(api).toHaveBeenCalledTimes(4);
    expect(
      vi.mocked(api).mock.calls.every(([, args]) => args?.revalidate === false),
    ).toBe(true);
    stop();
    const finish = startAnalysisPolling({ ...options, active: false });
    await vi.advanceTimersByTimeAsync(30_000);
    expect(api).toHaveBeenCalledTimes(5);
    expect(api).toHaveBeenLastCalledWith("analysis_results", {
      scanId: "s",
      entryId: 1,
      revalidate: true,
    });
    finish();
  });
  it("does not overlap requests or update a closed detail panel", async () => {
    let resolve!: (value: unknown[]) => void;
    vi.mocked(api).mockReturnValue(
      new Promise((r) => {
        resolve = r;
      }),
    );
    const onResults = vi.fn();
    const stop = startAnalysisPolling({
      scanId: "s",
      entryId: 1,
      active: true,
      onResults,
      onError: vi.fn(),
    });
    await vi.advanceTimersByTimeAsync(9000);
    expect(api).toHaveBeenCalledTimes(1);
    stop();
    resolve([]);
    await vi.advanceTimersByTimeAsync(3000);
    expect(onResults).not.toHaveBeenCalled();
  });
});

describe("scan status polling", () => {
  it("uses lightweight status instead of loading startup data every four seconds", async () => {
    const f = fixture();
    await vi.advanceTimersByTimeAsync(28_000);
    expect(api).toHaveBeenCalledTimes(7);
    expect(
      vi
        .mocked(api)
        .mock.calls.every(([command]) => command === "runtime_status"),
    ).toBe(true);
    expect(f.onBootstrap).not.toHaveBeenCalled();
    f.stop();
  });
  it("refreshes overview metadata slowly and never refreshes it on other pages", async () => {
    const f = fixture();
    f.setOverview(false);
    await vi.advanceTimersByTimeAsync(40_000);
    expect(f.onBootstrap).not.toHaveBeenCalled();
    f.setOverview(true);
    expect(f.onBootstrap).not.toHaveBeenCalled(); // Switching pages itself does no IO.
    await vi.advanceTimersByTimeAsync(4000);
    expect(f.onBootstrap).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(20_000);
    expect(f.onBootstrap).toHaveBeenCalledTimes(1);
    f.stop();
  });
  it("does not overlap slow reads or apply a result after the scan changes", async () => {
    let resolve!: (value: RuntimeStatus) => void;
    vi.mocked(api).mockImplementationOnce(
      () =>
        new Promise((r) => {
          resolve = r;
        }),
    );
    const f = fixture();
    await vi.advanceTimersByTimeAsync(12_000);
    expect(api).toHaveBeenCalledTimes(1);
    f.stop();
    resolve(status);
    await vi.advanceTimersByTimeAsync(4000);
    expect(f.onStatus).not.toHaveBeenCalled();
    expect(f.onBootstrap).not.toHaveBeenCalled();
  });
  it("keeps polling after a failed read", async () => {
    vi.mocked(api).mockRejectedValueOnce(new Error("read failed"));
    const f = fixture();
    await vi.advanceTimersByTimeAsync(8000);
    expect(f.onError).toHaveBeenCalledTimes(1);
    expect(f.onStatus).toHaveBeenCalledWith(status);
    f.stop();
  });
  it("detects unchanged snapshots without hiding activity changes", () => {
    expect(sameSnapshot(status, structuredClone(status))).toBe(true);
    expect(
      sameSnapshot(status, {
        ...status,
        scan: { ...status.scan, status: "scanning" },
      }),
    ).toBe(false);
  });
});
