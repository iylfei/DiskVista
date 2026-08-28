import { useEffect, useState } from "react";
import { startAnalysisSummaryLoad } from "./analysisSummaries";
import type { AnalysisSummary, FileRecord } from "./types";

const empty = new Map<number, AnalysisSummary>();

export function useAnalysisSummaries(
  scanId: string,
  items: FileRecord[],
  revision: string,
) {
  const ids = JSON.stringify(items.map((file) => file.id));
  const key = JSON.stringify([scanId, ids, revision]);
  const [data, setData] = useState<{
    key: string;
    summaries: Map<number, AnalysisSummary>;
    failed: boolean;
  } | null>(null);

  useEffect(
    () =>
      startAnalysisSummaryLoad({
        scanId,
        entryIds: JSON.parse(ids) as number[],
        onResults: (results) =>
          setData({
            key,
            summaries: new Map(
              results.map((result) => [result.entryId, result]),
            ),
            failed: false,
          }),
        onError: () => setData({ key, summaries: empty, failed: true }),
      }),
    [scanId, ids, key],
  );

  return data?.key === key ? data : { summaries: empty, failed: false };
}
