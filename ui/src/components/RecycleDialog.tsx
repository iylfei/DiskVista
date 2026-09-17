import { useEffect, useRef, useState } from "react";
import type { FileRecord, HistoryItem } from "../lib/types";
import {
  createRecycleSession,
  initialRecycleState,
  type RecycleDialogState,
} from "../lib/recycleSession";
import Modal from "./Modal";
import "./recycle.css";
interface Props {
  scanId: string;
  files: FileRecord[];
  onClose: () => void;
  onDone: (result: HistoryItem[]) => void;
  onBusyChange: (busy: boolean) => void;
}
export function RecycleDialogView({
  files,
  state,
  onClose,
  onRetry,
  onCancelRemaining,
}: {
  files: FileRecord[];
  state: RecycleDialogState;
  onClose: () => void;
  onRetry: () => void;
  onCancelRemaining: () => void;
}) {
  const executing = state.phase === "executing";
  const checking = state.phase === "checking";
  const progress = checking ? state.checkProgress : null;
  return (
    <Modal
      title="移入回收站"
      onClose={onClose}
      closeDisabled={executing || state.cancelRequested}
    >
      <div
        className="modal-body recycle-dialog-body"
        aria-busy={checking || executing}
      >
        <p className="recycle-target path-text">
          {files.length === 1
            ? files[0].path
            : `待清理清单：${files.length} 项`}
        </p>
        {checking && (
          <p role="status">
            {state.cancelRequested ? "正在取消…" : "正在准备…"}
          </p>
        )}
        {executing && (
          <p role="status">
            {state.cancelRequested
              ? "正在取消剩余项…"
              : "正在移入回收站…"}
          </p>
        )}
        {(state.phase === "checking" || executing) && (
          <progress
            className="recycle-progress"
            value={
              progress && !state.cancelRequested
                ? progress.targetsDone
                : undefined
            }
            max={progress?.targetsTotal || undefined}
            aria-label={
              executing
                ? "正在移入回收站…"
                : "正在准备…"
            }
          />
        )}
        {state.error && (
          <div className="recycle-error" role="alert">
            <p>{state.error}</p>
            <button type="button" onClick={onRetry}>
              重试
            </button>
          </div>
        )}
        {state.cancelError && (
          <p className="warning-text" role="alert">
            取消失败：{state.cancelError}
          </p>
        )}
      </div>
      <footer>
        {executing && (
          <button
            type="button"
            onClick={onCancelRemaining}
            disabled={state.cancelRequested}
          >
            {state.cancelRequested ? "已请求取消剩余项" : "取消剩余项"}
          </button>
        )}
        <button
          type="button"
          onClick={onClose}
          disabled={executing || state.cancelRequested}
        >
          {checking ? "取消并关闭" : "关闭"}
        </button>
      </footer>
    </Modal>
  );
}
export default function RecycleDialog({
  scanId,
  files,
  onClose,
  onDone,
  onBusyChange,
}: Props) {
  const key = JSON.stringify([
    scanId,
    files.map((file) => [file.id, file.isDir]),
  ]);
  const [snapshot, setSnapshot] = useState({ key, state: initialRecycleState });
  const callbacks = useRef({ onDone, onBusyChange });
  callbacks.current = { onDone, onBusyChange };
  const session = useRef<{
    key: string;
    value: ReturnType<typeof createRecycleSession>;
  } | null>(null);
  useEffect(() => {
    const value = createRecycleSession({
      scanId,
      files,
      onState: (state) => setSnapshot({ key, state }),
      onDone: (result) => callbacks.current.onDone(result),
      onBusyChange: (busy) => callbacks.current.onBusyChange(busy),
    });
    session.current = { key, value };
    return () => {
      value.dispose();
      if (session.current?.value === value) session.current = null;
    };
  }, [key]);
  return (
    <RecycleDialogView
      files={files}
      state={snapshot.key === key ? snapshot.state : initialRecycleState}
      onClose={() => {
        const current = session.current;
        if (!current) onClose();
        else
          void current.value.close().then((closed) => {
            if (closed && session.current === current) onClose();
          });
      }}
      onRetry={() => {
        if (session.current?.key === key) void session.current.value.run();
      }}
      onCancelRemaining={() => {
        if (session.current?.key === key)
          void session.current.value.cancelRemaining();
      }}
    />
  );
}
