import { useState } from "react";
import { Trash2, ShieldCheck } from "lucide-react";
import type { FileRecord, CleanupPreview, HistoryItem } from "../lib/types";
import { api, bytes, riskText } from "../lib/api";
import Modal from "../components/Modal";
import HelpTip from "../components/HelpTip";
import { helpText } from "../lib/helpText";
import { selectedUsage } from "../lib/selection";
export default function BasketPage({
  items,
  scanId,
  onRemove,
  onDone,
  onError,
  onFindFiles,
  onClear,
}: {
  items: FileRecord[];
  scanId: string;
  onRemove: (f: FileRecord) => void;
  onDone: (result: HistoryItem[]) => void;
  onError: (e: unknown) => void;
  onFindFiles: () => void;
  onClear: () => void;
}) {
  const [preview, setPreview] = useState<CleanupPreview | null>(null);
  const [busy, setBusy] = useState(false);
  const [ack, setAck] = useState(false);
  const [executing, setExecuting] = useState(false);
  async function createPreview() {
    setBusy(true);
    try {
      setPreview(
        await api("preview_cleanup", {
          scanId,
          entryIds: items.map((f) => f.id),
        }),
      );
      setAck(false);
    } catch (e) {
      onError(e);
    } finally {
      setBusy(false);
    }
  }
  async function execute() {
    if (!preview) return;
    setExecuting(true);
    try {
      const result = await api<HistoryItem[]>("execute_cleanup", {
        previewId: preview.id,
        acknowledgeRisk: ack,
      });
      setPreview(null);
      onDone(result);
    } catch (e) {
      onError(e);
    } finally {
      setExecuting(false);
    }
  }
  return (
    <>
      <div className="notice">
        <ShieldCheck size={19} />
        <div>
          <strong>先预览，再确认；只移入回收站</strong>
          <p>
            移入回收站通常不会立刻腾出空间。删错后可以在 Windows
            回收站中选择“还原”；本程序不会自动清空回收站。
          </p>
        </div>
      </div>
      <section className="panel">
        <div className="section-heading">
          <h2>
            已选 {items.length} 项
            {items.length > 0 ? ` · 占用约 ${bytes(selectedUsage(items))}` : ""}
          </h2>
          <span className="muted">文件夹和其中的文件不会重复处理</span>
          {items.length > 0 && (
            <button disabled={busy || executing} onClick={onClear}>
              取消全部选择
            </button>
          )}
        </div>
        {items.length === 0 ? (
          <div className="empty">
            <Trash2 size={32} />
            <h3>还没有选择要清理的文件</h3>
            <p>先查看清理建议，确认哪些文件已经不需要。</p>
            <button className="primary" onClick={onFindFiles}>
              去查看清理建议
            </button>
          </div>
        ) : (
          items.map((f) => (
            <div className="basket-row" key={f.id}>
              <div>
                <strong>{f.name}</strong>
                <small className="path-text">{f.path}</small>
              </div>
              <span>{bytes(f.allocatedBytes ?? f.logicalBytes)}</span>
              <span className={`badge ${f.assessment.risk}`}>
                {riskText(f.assessment.risk)}
              </span>
              <button onClick={() => onRemove(f)} disabled={executing}>
                取消选择
              </button>
            </div>
          ))
        )}
        <div className="actions">
          <button
            className="primary"
            disabled={!items.length || busy || executing}
            onClick={createPreview}
          >
            {busy ? "正在检查文件…" : "检查并预览清理"}
          </button>
        </div>
      </section>
      {preview && (
        <Modal
          title="确认移入 Windows 回收站"
          wide
          onClose={() => {
            if (!executing) setPreview(null);
          }}
        >
          <div className="modal-body">
            <p>
              以下是准备移入回收站的内容。执行前会再次检查；文件有变化、受保护或无法安全处理时会跳过或停止。
            </p>
            {preview.items.map((p) => (
              <div className="preview-item" key={p.entryId}>
                <strong className="path-text">{p.path}</strong>
                <span>
                  {bytes(p.bytes)} · {p.allowed ? riskText(p.risk) : "将跳过"}
                </span>
                <p className={p.allowed ? "muted" : "warning-text"}>
                  {p.reason}
                </p>
              </div>
            ))}
            <div className="metric compact-metric">
              <span>
                从回收站永久删除这些文件后，预计可释放
                <HelpTip label="预计可释放空间" text={helpText.pendingSpace} />
              </span>
              <strong>{bytes(preview.pendingBytes)}</strong>
              <small>这是估算值，不是当前已经释放的空间。</small>
            </div>
            {preview.requiresExtraConfirmation && (
              <label className="check-line warning-text">
                <input
                  type="checkbox"
                  checked={ack}
                  onChange={(e) => setAck(e.target.checked)}
                  disabled={executing}
                />
                我已检查这些用途不明或包含个人数据的文件，理解清理可能丢失数据或影响应用；回收站不能代替备份。
              </label>
            )}
            <p className="muted">
              不会强行关闭应用或更改文件权限。无法确认能安全移入回收站时，会停止处理。
            </p>
          </div>
          <footer>
            {executing ? (
              <>
                <span>正在逐项检查并移入回收站…</span>
                <button onClick={() => api("cancel_cleanup").catch(onError)}>
                  取消剩余项
                </button>
              </>
            ) : (
              <>
                <button onClick={() => setPreview(null)}>返回检查</button>
                <button
                  className="primary"
                  disabled={
                    preview.items.every((p) => !p.allowed) ||
                    (preview.requiresExtraConfirmation && !ack)
                  }
                  onClick={execute}
                >
                  确认移入回收站
                </button>
              </>
            )}
          </footer>
        </Modal>
      )}
    </>
  );
}
