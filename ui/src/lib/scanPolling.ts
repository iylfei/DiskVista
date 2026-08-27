import { api } from "./api";
import type { Bootstrap, RuntimeStatus } from "./types";

export const OVERVIEW_REFRESH_MS = 30_000;

interface PollingOptions {
  scanId: string;
  intervalMs: number;
  isOverview: () => boolean;
  onStatus: (value: RuntimeStatus) => void;
  onBootstrap: (value: Bootstrap) => void;
  onError: (error: unknown) => void;
}

/** One in-flight request chain. Navigation does not restart the poll or reload startup data. */
export function startScanPolling(options: PollingOptions) {
  let live = true;
  let pending = false;
  let lastOverviewRefresh = Date.now();
  const timer = setInterval(async () => {
    if (!live || pending) return;
    pending = true;
    try {
      const status = await api<RuntimeStatus>("runtime_status", {
        scanId: options.scanId,
      });
      if (!live) return;
      options.onStatus(status);
      // Volume capacity/credential presence are not scan progress. Keep the
      // displayed overview fresh at a slower cadence, without reloading on tab clicks.
      if (
        live &&
        options.isOverview() &&
        Date.now() - lastOverviewRefresh >= OVERVIEW_REFRESH_MS
      ) {
        lastOverviewRefresh = Date.now();
        const boot = await api<Bootstrap>("bootstrap");
        if (live) options.onBootstrap(boot);
      }
    } catch (error) {
      if (live) options.onError(error);
    } finally {
      pending = false;
    }
  }, options.intervalMs);
  return () => {
    live = false;
    clearInterval(timer);
  };
}

/** Preserve stable props when a finished snapshot has not changed. */
export function sameSnapshot<T>(previous: T, next: T): boolean {
  return previous === next || JSON.stringify(previous) === JSON.stringify(next);
}
