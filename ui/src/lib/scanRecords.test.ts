import { expect, it } from "vitest";
import { afterScanDeletion, excludeDeletedScans } from "./scanRecords";
import type { Bootstrap, Scan } from "./types";

it("filters deleted IDs from late polling snapshots without losing newer scans", () => {
  const scans = ["gone", "keep", "new"].map((id) => ({ id }) as Scan);
  const snapshot = { scans, extra: "preserved" };
  expect(excludeDeletedScans(snapshot, new Set(["gone"]))).toEqual({
    scans: scans.slice(1),
    extra: "preserved",
  });
  expect(snapshot.scans).toHaveLength(3);
});

it("only resets analysis progress belonging to the removed scan", () => {
  const boot = {
    scans: [{ id: "gone" }],
    analysisProgress: { scanId: "gone", active: false, message: "old" },
  } as Bootstrap;
  expect(
    afterScanDeletion(boot, [], "gone").analysisProgress.scanId,
  ).toBeNull();
  expect(afterScanDeletion(boot, [], "other").analysisProgress).toBe(
    boot.analysisProgress,
  );
  const stale = excludeDeletedScans(boot, new Set(["gone"]));
  expect(stale.analysisProgress.scanId).toBeNull();
  expect(stale.analysisProgress.message).toBe("");
});
