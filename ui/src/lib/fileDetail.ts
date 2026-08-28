import { api } from "./api";
import type { FileRecord } from "./types";

export function mergeFileDetail(
  current: FileRecord | null,
  currentScanId: string | null,
  target: { scanId: string; entryId: number },
  result: FileRecord,
): FileRecord | null {
  return currentScanId === target.scanId &&
    current?.id === target.entryId &&
    result.id === target.entryId
    ? result
    : current;
}

export function startFileDetailLoad(options: {
  scanId: string;
  entryId: number;
  onResult: (file: FileRecord) => void;
  onError: (error: unknown) => void;
}) {
  let live = true;
  void api<FileRecord>("entry_detail", {
    scanId: options.scanId,
    entryId: options.entryId,
  }).then(
    (file) => {
      if (live) options.onResult(file);
    },
    (error) => {
      if (live) options.onError(error);
    },
  );
  return () => {
    live = false;
  };
}
