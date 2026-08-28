import { useEffect, useRef, useState } from "react";
import { Trash2 } from "lucide-react";
import { bytes, riskText } from "../lib/api";
import type { FileRecord, HistoryItem } from "../lib/types";
import {
  canExecuteRecycle,
  createRecycleSession,
  initialRecycleState,
  needsRecycleAcknowledgement,
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
  onAcknowledge,
  onConfirm,
  onCancelRemaining,
}: {
  files: FileRecord[];
  state: RecycleDialogState;
  onClose: () => void;
  onRetry: () => void;
  onAcknowledge: (value: boolean) => void;
  onConfirm: () => void;
  onCancelRemaining: () => void;
}) {
  const busy = state.phase === "executing";
  const preview = state.preview;
  const allowedCount =
    preview?.items.filter((item) => item.allowed).length ?? 0;
  const allowed = allowedCount > 0;
  const hasDirectories = files.some((file) => file.isDir);
  const targets = new Map(files.map((file) => [file.id, file]));
  return (
    <Modal title="移入回收站" onClose={onClose} closeDisabled={busy}>
      <div className="modal-body recycle-dialog-body" aria-busy={busy}>
        <p className="recycle-target path-text">
          {files.length === 1
            ? files[0].path
            : `待清理清单：${files.length} 项`}
        </p>
        {hasDirectories && (
          <p className="warning-text recycle-directory-warning">
            对检查通过的目录，将回收整个目录及其全部内容，包括单独列出的子目录。
          </p>
        )}
        {state.phase === "previewing" && <p role="status">正在检查回收条件…</p>}
        {state.error && (
          <div className="recycle-error" role="alert">
            <p>{state.error}</p>
            <button type="button" onClick={onRetry}>
              重新检查
            </button>
          </div>
        )}
        {preview && (
          <>
            <p className="recycle-preview-count">
              实际预览 {preview.items.length} 项 · 检查通过 {allowedCount} 项 ·
              不可回收 {preview.items.length - allowedCount} 项
            </p>
            {preview.items.length < files.length && (
              <p className="muted recycle-deduplication">
                同时选择父目录与其子项时，预览会合并重复范围。
              </p>
            )}
            <ul className="recycle-preview" aria-label="实际回收预览">
              {preview.items.map((item) => {
                const file = targets.get(item.entryId);
                return (
                  <li key={item.entryId}>
                    <p className="path-text">{item.path}</p>
                    <div className="recycle-preview-meta">
                      <span>{bytes(item.bytes)}</span>
                      <span
                        className={`badge ${item.allowed ? "neutral" : "protected"}`}
                      >
                        {item.allowed ? "检查通过" : "不可回收"}
                      </span>
                      <span className={`badge ${item.risk}`}>
                        {riskText(item.risk)}
                      </span>
                    </div>
                    <p className={item.allowed ? "muted" : "warning-text"}>
                      {item.reason}
                    </p>
                    {file?.isDir && (
                      <p className="muted recycle-snapshot">
                        目录扫描记录：{file.fileCount.toLocaleString()} 个文件 ·
                        占用 {bytes(file.allocatedBytes ?? file.logicalBytes)}
                        {file.allocatedBytes === null ? "（估算）" : ""}
                        {!file.complete ? " · 未完整扫描" : ""}
                      </p>
                    )}
                    {file?.assessment?.consequence && (
                      <p className="recycle-impact">
                        清理影响：{file.assessment.consequence}
                      </p>
                    )}
                  </li>
                );
              })}
            </ul>
            <p className="recycle-preview-total">
              {allowed
                ? `本次回收大小：${bytes(preview.pendingBytes)}`
                : "没有可回收的项目。"}
            </p>
            {needsRecycleAcknowledgement(files, preview) && (
              <label className="check-line recycle-acknowledgement">
                <input
                  type="checkbox"
                  checked={state.acknowledged}
                  disabled={state.phase !== "ready" || !allowed}
                  onChange={(event) => onAcknowledge(event.target.checked)}
                />
                <span>
                  {hasDirectories
                    ? "我确认回收检查通过的项目，包括其中目录的全部内容。"
                    : "我已核实上述路径和清理影响，确认不再需要这些文件。"}
                </span>
              </label>
            )}
          </>
        )}
        <p className="muted recycle-rescan-note">
          仅移入 Windows 回收站，不会永久删除；清空回收站前通常不会释放空间。
          回收后可重新扫描更新空间占用。
        </p>
        {busy && (
          <p role="status">
            {state.cancelRequested
              ? "已请求取消尚未开始的项目，正在等待操作结果。"
              : "正在移入回收站，请等待操作完成。"}
          </p>
        )}
        {busy && state.cancelError && (
          <p className="warning-text" role="alert">
            取消请求失败：{state.cancelError}。回收仍在进行，可重试取消剩余项。
          </p>
        )}
      </div>
      <footer>
        {busy && (
          <button
            type="button"
            onClick={onCancelRemaining}
            disabled={state.cancelRequested}
          >
            {state.cancelRequested ? "已请求取消剩余项" : "取消剩余项"}
          </button>
        )}
        <button type="button" onClick={onClose} disabled={busy}>
          取消
        </button>
        <button
          type="button"
          className="primary"
          disabled={!canExecuteRecycle(files, state)}
          onClick={onConfirm}
        >
          <Trash2 size={16} />
          {busy
            ? "正在移入回收站…"
            : state.phase === "complete"
              ? "操作已完成"
              : "确认移入回收站"}
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
        if (!session.current || session.current.value.close()) onClose();
      }}
      onRetry={() => {
        if (session.current?.key === key) void session.current.value.preview();
      }}
      onAcknowledge={(value) => {
        if (session.current?.key === key)
          session.current.value.acknowledge(value);
      }}
      onConfirm={() => {
        if (session.current?.key === key) void session.current.value.execute();
      }}
      onCancelRemaining={() => {
        if (session.current?.key === key)
          void session.current.value.cancelRemaining();
      }}
    />
  );
}
