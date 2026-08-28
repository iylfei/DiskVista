import { api } from "./api";
import type { AnalysisSummary } from "./types";

export function analysisLabel(result: AnalysisSummary): string {
  if (result.status === "stale") return "AI 已过期";
  if (result.historyMatchCount > 0) return "与删除历史相似";
  return result.summary || "AI 已分析";
}

export function startAnalysisSummaryLoad(options: {
  scanId: string;
  entryIds: number[];
  onResults: (results: AnalysisSummary[]) => void;
  onError: (error: unknown) => void;
}) {
  let live = true;
  const entryIds = [...new Set(options.entryIds)];
  if (!options.scanId || entryIds.length === 0) {
    options.onResults([]);
    return () => {
      live = false;
    };
  }
  void api<AnalysisSummary[]>("analysis_summaries", {
    scanId: options.scanId,
    entryIds,
  }).then(
    (results) => {
      if (live) options.onResults(results);
    },
    (error) => {
      if (live) options.onError(error);
    },
  );
  return () => {
    live = false;
  };
}
