import { useCallback, useState } from "react";

/** Retain refresh results, but never display them under another query's controls. */
export function useQueryResult<T>(key: string) {
  const [result, setResult] = useState<{ key: string; data: T | null } | null>(
    null,
  );
  const setData = useCallback(
    (data: T | null) => setResult({ key, data }),
    [key],
  );
  return [result?.key === key ? result.data : null, setData] as const;
}
