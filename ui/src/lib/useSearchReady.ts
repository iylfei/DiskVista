import { useEffect, useState } from "react";

/** Debounce text edits without delaying navigation, sorting or pagination. */
export function useSearchReady(search: string, delayMs = 180) {
  const [settled, setSettled] = useState(search);
  useEffect(() => {
    if (search === settled) return;
    if (!search) {
      setSettled("");
      return;
    }
    const timer = setTimeout(() => setSettled(search), delayMs);
    return () => clearTimeout(timer);
  }, [search, settled, delayMs]);
  return !search || search === settled;
}
