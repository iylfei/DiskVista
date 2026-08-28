import { api } from "./api";
import { selectionLimit } from "./selection";
import type { CleanupPreview, FileRecord, HistoryItem } from "./types";

type Target = Pick<FileRecord, "id" | "isDir">;
export interface RecycleDialogState {
  phase: "previewing" | "ready" | "executing" | "error" | "complete";
  preview: CleanupPreview | null;
  acknowledged: boolean;
  error: string;
  cancelRequested: boolean;
  cancelError: string;
}

export const initialRecycleState: RecycleDialogState = {
  phase: "previewing",
  preview: null,
  acknowledged: false,
  error: "",
  cancelRequested: false,
  cancelError: "",
};

export function needsRecycleAcknowledgement(
  files: readonly Target[],
  preview: CleanupPreview | null,
) {
  return (
    files.some((file) => file.isDir) || !!preview?.requiresExtraConfirmation
  );
}

export function canExecuteRecycle(
  files: readonly Target[],
  state: RecycleDialogState,
) {
  return (
    state.phase === "ready" &&
    !!state.preview?.items.some((item) => item.allowed) &&
    (!needsRecycleAcknowledgement(files, state.preview) || state.acknowledged)
  );
}

export function createRecycleSession({
  scanId,
  files,
  onState,
  onDone,
  onBusyChange,
}: {
  scanId: string;
  files: readonly Target[];
  onState: (state: RecycleDialogState) => void;
  onDone: (result: HistoryItem[]) => void;
  onBusyChange: (busy: boolean) => void;
}) {
  let live = true;
  let request = 0;
  let state = initialRecycleState;
  const targets = files.map(({ id, isDir }) => ({ id, isDir }));
  const targetIds = new Set(targets.map((file) => file.id));
  const update = (value: RecycleDialogState) => {
    state = value;
    if (live) onState(value);
  };
  const failed = (error: unknown) =>
    update({
      ...initialRecycleState,
      phase: "error",
      error: error instanceof Error ? error.message : String(error),
    });

  async function preview() {
    if (!live || state.phase === "executing" || state.phase === "complete")
      return;
    const current = ++request;
    update({ ...initialRecycleState });
    try {
      if (
        !scanId ||
        targets.length < 1 ||
        targets.length > selectionLimit ||
        targetIds.size !== targets.length
      )
        throw new Error(`每次只能检查 1–${selectionLimit} 个不重复的项目。`);
      const result = await api<CleanupPreview>("preview_cleanup", {
        scanId,
        entryIds: targets.map((file) => file.id),
      });
      if (!live || current !== request) return;
      const receivedIds = new Set<number>();
      if (
        !result ||
        !result.id ||
        result.scanId !== scanId ||
        !Array.isArray(result.items) ||
        result.items.length < 1 ||
        result.items.length > targetIds.size ||
        result.items.some((item) => {
          if (
            !item ||
            !targetIds.has(item.entryId) ||
            receivedIds.has(item.entryId)
          )
            return true;
          receivedIds.add(item.entryId);
          return false;
        })
      )
        throw new Error("回收预览与当前清单不一致，请重新检查。");
      update({
        ...initialRecycleState,
        phase: "ready",
        preview: result,
      });
    } catch (error) {
      if (live && current === request) failed(error);
    }
  }

  async function execute() {
    if (!live || !canExecuteRecycle(targets, state) || !state.preview) return;
    const { preview, acknowledged } = state;
    update({ ...state, phase: "executing" });
    onBusyChange(true);
    let result: HistoryItem[] | null = null;
    try {
      result = await api<HistoryItem[]>("execute_cleanup", {
        previewId: preview.id,
        acknowledgeRisk: acknowledged,
      });
      update({ ...state, phase: "complete" });
    } catch (error) {
      failed(error);
    } finally {
      onBusyChange(false);
    }
    // Once execution starts, its outcome still belongs to the parent if the view unmounts.
    if (result !== null) onDone(result);
  }

  async function cancelRemaining() {
    if (!live || state.phase !== "executing" || state.cancelRequested) return;
    const current = request;
    update({ ...state, cancelRequested: true, cancelError: "" });
    try {
      await api("cancel_cleanup");
    } catch (error) {
      if (live && current === request && state.phase === "executing")
        update({
          ...state,
          cancelRequested: false,
          cancelError: error instanceof Error ? error.message : String(error),
        });
    }
  }

  function dispose() {
    live = false;
    request += 1;
  }

  const session = {
    preview,
    execute,
    cancelRemaining,
    acknowledge(value: boolean) {
      if (
        live &&
        state.phase === "ready" &&
        state.preview?.items.some((item) => item.allowed)
      )
        update({ ...state, acknowledged: value });
    },
    close() {
      if (!live || state.phase === "executing") return false;
      dispose();
      return true;
    },
    dispose,
  };
  void preview();
  return session;
}
