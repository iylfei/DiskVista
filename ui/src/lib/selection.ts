import type { FileRecord, HistoryItem } from "./types";
export const selectionLimit = 500;

const key = (path: string) =>
  path.replaceAll("/", "\\").replace(/\\+$/, "").toLowerCase();

export function selectedUsage(files: FileRecord[]) {
  const directories = new Set(
    files.filter((file) => file.isDir).map((file) => key(file.path)),
  );
  const seen = new Set<string>();
  return files.reduce((total, file) => {
    const path = key(file.path);
    if (seen.has(path)) return total;
    seen.add(path);
    for (
      let end = path.lastIndexOf("\\");
      end >= 0;
      end = path.lastIndexOf("\\", end - 1)
    ) {
      if (directories.has(path.slice(0, end))) return total;
      if (end === 0) break;
    }
    return total + (file.allocatedBytes ?? file.logicalBytes);
  }, 0);
}

export function addSelection(
  current: Map<number, FileRecord>,
  files: FileRecord[],
) {
  const next = new Map(current);
  for (const file of files)
    if (file.assessment.risk !== "protected") next.set(file.id, file);
  if (next.size > selectionLimit)
    throw new Error(
      `每次最多选择 ${selectionLimit} 项。请先处理当前已选内容，或减少选择。`,
    );
  return next;
}

export function removeRecycledSelection(
  current: Map<number, FileRecord>,
  results: HistoryItem[],
) {
  const recycled = results
    .filter((item) => item.status === "recycled")
    .map((item) => ({
      path: key(item.path),
      isDir: item.snapshot?.isDir === true,
    }));
  return new Map(
    [...current].filter(([, file]) => {
      const path = key(file.path);
      return !recycled.some(
        (item) =>
          path === item.path ||
          (item.isDir && path.startsWith(`${item.path}\\`)),
      );
    }),
  );
}

export function cleanupResult(items: HistoryItem[]) {
  return items.reduce(
    (result, item) => {
      if (item.status === "recycled") {
        result.recycled++;
        result.bytes += item.bytes;
      } else if (item.status === "skipped") result.skipped++;
      else result.failed++;
      return result;
    },
    { recycled: 0, skipped: 0, failed: 0, bytes: 0 },
  );
}

export function addBasketSelection(
  current: {
    pending: Map<number, FileRecord>;
    basket: Map<number, FileRecord>;
  },
  files: FileRecord[],
) {
  const basket = addSelection(current.basket, files);
  const pending = new Map(current.pending);
  for (const id of basket.keys()) pending.delete(id);
  addSelection(basket, [...pending.values()]);
  return { basket, pending, added: basket.size - current.basket.size };
}
