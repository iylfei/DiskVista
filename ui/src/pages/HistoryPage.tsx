import { useEffect, useState } from "react";
import type { HistoryItem } from "../lib/types";
import { api, bytes, statusText } from "../lib/api";
import HelpTip from "../components/HelpTip";
import { helpText } from "../lib/helpText";
import CleanupResult from "../components/CleanupResult";
export default function HistoryPage({
  onError,
  recent = [],
}: {
  onError: (e: unknown) => void;
  recent?: HistoryItem[];
}) {
  const [items, setItems] = useState<HistoryItem[]>([]);
  useEffect(() => {
    api<HistoryItem[]>("history").then(setItems).catch(onError);
  }, [onError]);
  return (
    <section className="panel">
      <CleanupResult items={recent} />
      <div className="section-heading">
        <div>
          <h2>操作历史</h2>
          <p>查看每个文件的处理结果，显示最近 500 条记录。</p>
        </div>
        <button
          onClick={() =>
            api("open_system", { target: "recycle" }).catch(onError)
          }
        >
          打开 Windows 回收站
        </button>
      </div>
      <div className="notice">
        需要恢复？在 Windows
        回收站中找到原文件，选择“还原”。本程序不会自动清空回收站。
      </div>
      <p className="footnote">
        可用空间变化
        <HelpTip label="可用空间变化" text={helpText.freeSpaceChange} />
        不一定等于本次清理释放的空间。
      </p>
      {items.length === 0 ? (
        <div className="empty">
          <p>尚未执行清理操作</p>
        </div>
      ) : (
        items.map((h) => (
          <div className="history-row" key={h.id}>
            <div>
              <strong className="path-text">{h.path}</strong>
              <small>
                {new Date(h.time * 1000).toLocaleString("zh-CN")} · 文件占用约{" "}
                {bytes(h.bytes)} · 可用空间变化{" "}
                {h.freeSpaceDelta >= 0 ? "+" : "−"}
                {bytes(Math.abs(h.freeSpaceDelta))}
              </small>
              <p>{h.message}</p>
            </div>
            <span
              className={`badge ${h.status === "recycled" ? "low" : "review"}`}
            >
              {statusText(h.status)}
            </span>
          </div>
        ))
      )}
    </section>
  );
}
