import { useEffect, useRef, useState } from "react";
import { Trash2 } from "lucide-react";
import type { FileRecord, HistoryItem } from "../lib/types";
import { bytes } from "../lib/api";
import { selectedUsage } from "../lib/selection";
import RecycleDialog from "../components/RecycleDialog";

export default function BasketPage({
  items,
  scanId,
  addingCount = 0,
  onRemove,
  onDone,
  onFindFiles,
  onClear,
  onBusyChange,
}: {
  items: FileRecord[];
  scanId: string;
  addingCount?: number;
  onRemove: (f: FileRecord) => void;
  onDone: (result: HistoryItem[]) => void;
  onError: (e: unknown) => void;
  onFindFiles: () => void;
  onClear: () => void;
  onBusyChange?: (busy: boolean) => void;
}) {
  const [busy, setBusy] = useState(false);
  const running = useRef(false);
  const [confirmation, setConfirmation] = useState<{
    scanId: string;
    files: FileRecord[];
  } | null>(null);
  const confirmationRef = useRef<typeof confirmation>(null);

  function openConfirmation() {
    if (
      running.current ||
      confirmationRef.current ||
      !items.length ||
      addingCount > 0
    )
      return;
    const target = { scanId, files: [...items] };
    confirmationRef.current = target;
    setConfirmation(target);
  }

  function closeConfirmation() {
    if (running.current) return;
    confirmationRef.current = null;
    setConfirmation(null);
  }

  useEffect(() => {
    if (confirmationRef.current?.scanId !== scanId) closeConfirmation();
  }, [scanId]);

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
        {addingCount > 0 && (
          <span className="muted" role="status" style={{ alignSelf: "center" }}>
            正在添加 {addingCount} 项…
          </span>
        )}
        <button
          className="primary"
          disabled={!items.length || busy || !!confirmation || addingCount > 0}
          onClick={openConfirmation}
        >
          {busy ? "正在移入回收站…" : "检查并移入回收站"}
        </button>
      </div>
      {confirmation && (confirmation.scanId === scanId || busy) && (
        <RecycleDialog
          scanId={confirmation.scanId}
          files={confirmation.files}
          onClose={closeConfirmation}
          onDone={(result) => {
            closeConfirmation();
            onDone(result);
          }}
          onBusyChange={(value) => {
            running.current = value;
            setBusy(value);
            onBusyChange?.(value);
          }}
        />
      )}
    </section>
  );
}
