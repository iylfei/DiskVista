import { api } from "./api";
import { selectionLimit } from "./selection";
import type { CleanupPreview, FileRecord, HistoryItem } from "./types";

type Target = Pick<FileRecord, "id" | "isDir">;
export interface RecycleDialogState {
  phase: "checking" | "executing" | "error" | "complete";
  error: string;
  cancelRequested: boolean;
  cancelError: string;
}
export const initialRecycleState: RecycleDialogState = {
  phase: "checking",
  error: "",
  cancelRequested: false,
  cancelError: "",
};
function validatePreview(
  result: CleanupPreview,
  scanId: string,
  targetIds: ReadonlySet<number>,
) {
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
    throw new Error("回收检查与当前清单不一致，请重试。");
}
function deniedMessage(preview: CleanupPreview) {
  const reasons = preview.items
    .filter((item) => !item.allowed)
    .slice(0, 3)
    .map((item) => `${item.path}：${item.reason}`);
  return `没有项目通过安全检查${reasons.length ? `：${reasons.join("；")}` : "。"}`;
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
  let running = false;
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
  async function run() {
    if (
      !live ||
      running ||
      state.phase === "executing" ||
      state.phase === "complete"
    )
      return;
    running = true;
    try {
      const current = ++request;
      update({ ...initialRecycleState });
      let preview: CleanupPreview;
      try {
        if (
          !scanId ||
          targets.length < 1 ||
          targets.length > selectionLimit ||
          targetIds.size !== targets.length
        )
          throw new Error(`每次只能处理 1–${selectionLimit} 个不重复的项目。`);
        preview = await api<CleanupPreview>("preview_cleanup", {
          scanId,
          entryIds: targets.map((file) => file.id),
        });
        if (!live || current !== request) return;
        validatePreview(preview, scanId, targetIds);
        if (!preview.items.some((item) => item.allowed))
          throw new Error(deniedMessage(preview));
      } catch (error) {
        if (live && current === request) failed(error);
        return;
      }
      update({ ...initialRecycleState, phase: "executing" });
      onBusyChange(true);
      let result: HistoryItem[] | null = null;
      try {
        result = await api<HistoryItem[]>("execute_cleanup", {
          previewId: preview.id,
          acknowledgeRisk: true,
        });
        update({ ...initialRecycleState, phase: "complete" });
      } catch (error) {
        failed(error);
      } finally {
        onBusyChange(false);
      }
      if (result !== null) onDone(result);
    } finally {
      running = false;
    }
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
    run,
    cancelRemaining,
    close() {
      if (!live || state.phase === "executing") return false;
      dispose();
      return true;
    },
    dispose,
  };
  void run();
  return session;
}
