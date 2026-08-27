import { useRef, useState } from "react";
import { Trash2 } from "lucide-react";
import type { FileRecord, CleanupPreview, HistoryItem } from "../lib/types";
import { api, bytes } from "../lib/api";
import { selectedUsage } from "../lib/selection";

export default function BasketPage({
  items,
  scanId,
  onRemove,
  onDone,
  onError,
  onFindFiles,
  onClear,
  onBusyChange,
}: {
  items: FileRecord[];
  scanId: string;
  onRemove: (f: FileRecord) => void;
  onDone: (result: HistoryItem[]) => void;
  onError: (e: unknown) => void;
  onFindFiles: () => void;
  onClear: () => void;
  onBusyChange?: (busy: boolean) => void;
}) {
  const [phase, setPhase] = useState<"idle" | "checking" | "recycling">("idle");
  const running = useRef(false);
  const busy = phase !== "idle";

  async function execute() {
    if (running.current || !items.length) return;
    running.current = true;
    setPhase("checking");
    onBusyChange?.(true);
    try {
      const preview = await api<CleanupPreview>("preview_cleanup", {
        scanId,
        entryIds: items.map((file) => file.id),
      });
      setPhase("recycling");
      const result = await api<HistoryItem[]>("execute_cleanup", {
        previewId: preview.id,
        acknowledgeRisk: true,
      });
      onDone(result);
    } catch (error) {
      onError(error);
    } finally {
      running.current = false;
      setPhase("idle");
      onBusyChange?.(false);
    }
  }

  return (
    <section className="panel">
      <div className="section-heading">
        <h2>
          待清理 {items.length} 项
          {items.length > 0 ? ` · 占用约 ${bytes(selectedUsage(items))}` : ""}
        </h2>
        {items.length > 0 && (
          <button disabled={busy} onClick={onClear}>
            清空清单
          </button>
        )}
      </div>
      {items.length === 0 ? (
        <div className="empty">
          <Trash2 size={32} />
          <h3>待清理清单为空</h3>
          <p>从清理建议中添加文件。</p>
          <button className="primary" onClick={onFindFiles}>
            去查看清理建议
          </button>
        </div>
      ) : (
        items.map((file) => (
          <div className="basket-row" key={file.id}>
            <div>
              <strong>{file.name}</strong>
              <small className="path-text">{file.path}</small>
            </div>
            <span>{bytes(file.allocatedBytes ?? file.logicalBytes)}</span>
            <button onClick={() => onRemove(file)} disabled={busy}>
              移出清单
            </button>
          </div>
        ))
      )}
      <div className="actions">
        {phase === "recycling" && (
          <button onClick={() => api("cancel_cleanup").catch(onError)}>
            取消剩余项
          </button>
        )}
        <button
          className="primary"
          disabled={!items.length || busy}
          onClick={execute}
        >
          {phase === "checking"
            ? "正在检查文件…"
            : phase === "recycling"
              ? "正在移入回收站…"
              : "检查并移入回收站"}
        </button>
      </div>
    </section>
  );
}
