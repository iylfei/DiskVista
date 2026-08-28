import type { Bootstrap, Progress, Scan } from "./types";

const emptyProgress: Progress = {
  active: false,
  scanId: null,
  queued: 0,
  finished: 0,
  requests: 0,
  maxRequests: 0,
  message: "",
};

export function excludeDeletedScans<
  T extends { scans: Scan[]; analysisProgress?: Progress },
>(value: T, deleted: ReadonlySet<string>): T {
  return {
    ...value,
    scans: value.scans.filter((scan) => !deleted.has(scan.id)),
    ...(value.analysisProgress?.scanId &&
    deleted.has(value.analysisProgress.scanId)
      ? { analysisProgress: { ...emptyProgress } }
      : {}),
  };
}

export function afterScanDeletion(
  boot: Bootstrap,
  scans: Scan[],
  deletedId: string,
): Bootstrap {
  return {
    ...boot,
    scans,
    analysisProgress:
      boot.analysisProgress.scanId === deletedId
        ? { ...emptyProgress }
        : boot.analysisProgress,
  };
}
