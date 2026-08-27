import { api } from "./api";
import type { AnalysisResult } from "./types";

export function startAnalysisPolling(options: {
  scanId: string;
  entryId: number;
  active: boolean;
  onResults: (results: AnalysisResult[]) => void;
  onError: (error: unknown) => void;
  onPending?: (pending: boolean) => void;
}) {
  let live = true;
  let pending = false;
  const refresh = async (revalidate: boolean) => {
    if (!live || pending) return;
    pending = true;
    options.onPending?.(true);
    try {
      const results = await api<AnalysisResult[]>("analysis_results", {
        scanId: options.scanId,
        entryId: options.entryId,
        revalidate,
      });
      if (live) options.onResults(results);
    } catch (error) {
      if (live) options.onError(error);
    } finally {
      pending = false;
      if (live) options.onPending?.(false);
    }
  };
  void refresh(!options.active);
  const timer = options.active
    ? setInterval(() => void refresh(false), 3000)
    : null;
  return () => {
    live = false;
    if (timer !== null) clearInterval(timer);
  };
}
