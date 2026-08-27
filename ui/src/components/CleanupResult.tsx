import type { HistoryItem } from "../lib/types";
import { bytes } from "../lib/api";
import { cleanupResult } from "../lib/selection";

export default function CleanupResult({ items }: { items: HistoryItem[] }) {
  if (!items.length) return null;
  const result = cleanupResult(items);
  return (
    <div className="cleanup-result" role="status">
      <strong>本次处理结果</strong>
      <span>
        已移入回收站 {result.recycled} 项（约 {bytes(result.bytes)}）
      </span>
      <span>跳过 {result.skipped} 项</span>
      {result.failed > 0 && (
        <span className="warning-text">未成功 {result.failed} 项</span>
      )}
    </div>
  );
}
