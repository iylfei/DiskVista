import { api } from "./api";
import { fileCleanupBlockReason } from "./cleanupTarget";
import { removeRecycledSelection } from "./selection";
import type { FileRecord, HistoryItem } from "./types";

export function createBasketEntryLoader({
  onAdd,
  onPending,
  onError,
}: {
  onAdd: (file: FileRecord) => void;
  onPending: (entryIds: Set<number>) => void;
  onError: (error: unknown) => void;
}) {
  let generation = 0;
  const pending = new Map<
    string,
    { entryId: number; recycled: HistoryItem[] }
  >();
  const notify = () =>
    onPending(new Set([...pending.values()].map((item) => item.entryId)));
  return {
    async add(scanId: string, entryId: number) {
      const key = JSON.stringify([scanId, entryId]);
      if (pending.has(key)) return;
      const current = generation;
      const request = { entryId, recycled: [] as HistoryItem[] };
      pending.set(key, request);
      notify();
      try {
        const file = await api<FileRecord>("entry_detail", { scanId, entryId });
        if (current !== generation) return;
        if (file.id !== entryId)
          throw new Error("读取的项目不一致，请重新选择");
        if (
          !removeRecycledSelection(new Map([[file.id, file]]), request.recycled)
            .size
        )
          return;
        const blocked = fileCleanupBlockReason(file);
        if (blocked) throw new Error(blocked);
        onAdd(file);
      } catch (error) {
        if (current === generation) onError(error);
      } finally {
        if (current === generation) {
          pending.delete(key);
          notify();
        }
      }
    },
    discardRecycled(results: HistoryItem[]) {
      const recycled = results.filter((item) => item.status === "recycled");
      for (const request of pending.values())
        request.recycled.push(...recycled);
    },
    reset() {
      generation += 1;
      pending.clear();
      notify();
    },
  };
}
