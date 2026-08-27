import { describe, expect, it } from "vitest";
import { addSelection, cleanupResult, selectedUsage } from "./selection";
import { suggestionEmpty } from "./emptyState";
import type { FileRecord, HistoryItem } from "./types";

function file(
  id: number,
  path: string,
  size: number,
  isDir = false,
): FileRecord {
  return {
    id,
    path,
    allocatedBytes: size,
    logicalBytes: size,
    isDir,
    assessment: { risk: "review" },
  } as FileRecord;
}

describe("cleanup selection", () => {
  it("does not count selected descendants or duplicate paths twice", () => {
    expect(
      selectedUsage([
        file(1, "D:\\App", 100, true),
        file(2, "d:\\app\\data", 40),
        file(3, "D:\\App-copy", 30),
        file(4, "d:/APP/", 100, true),
      ]),
    ).toBe(130);
  });
  it("adds a group idempotently without adding protected files or changing the previous selection", () => {
    const a = file(1, "D:\\a", 10),
      b = file(2, "D:\\b", 20);
    const blocked = {
      ...file(3, "D:\\system", 30),
      assessment: { ...a.assessment, risk: "protected" },
    };
    const current = new Map([[a.id, a]]);
    const next = addSelection(current, [a, b, blocked]);
    expect([...next.keys()]).toEqual([1, 2]);
    expect(current.size).toBe(1);
    expect(addSelection(next, [a, b]).size).toBe(2);
  });
  it("keeps the existing 500-item preview boundary", () => {
    expect(() =>
      addSelection(
        new Map(),
        Array.from({ length: 501 }, (_, id) => file(id, `D:\\${id}`, 1)),
      ),
    ).toThrow("500");
  });
  it("reports outcomes without calling recycled bytes released space", () => {
    const items = [
      { status: "recycled", bytes: 100 },
      { status: "skipped", bytes: 50 },
      { status: "failed", bytes: 70 },
    ] as HistoryItem[];
    expect(cleanupResult(items)).toEqual({
      recycled: 1,
      skipped: 1,
      failed: 1,
      bytes: 100,
    });
  });
});

describe("contextual empty states", () => {
  it("does not ask a scanning user to choose a disabled folder action", () => {
    expect(suggestionEmpty("scanning", "low", "").title).toContain("正在扫描");
    expect(suggestionEmpty("scanning", "low", "").reset).toBe(false);
    expect(suggestionEmpty("cancelled", "", "").title).toContain("没有完成");
  });
  it("distinguishes no low-risk results from an empty search", () => {
    expect(suggestionEmpty("complete", "low", "").title).toContain("较低风险");
    expect(suggestionEmpty("complete", "low", "example").title).toContain(
      "匹配",
    );
    expect(suggestionEmpty("complete", "", "").reset).toBe(false);
  });
});
