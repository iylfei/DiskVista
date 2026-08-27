import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import { sameSnapshot, startScanPolling } from "./scanPolling";
import { startAnalysisPolling } from "./analysisPolling";
import type { RuntimeStatus } from "./types";

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
