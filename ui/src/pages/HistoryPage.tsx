import { useEffect, useRef, useState } from "react";
import type { HistoryItem } from "../lib/types";
import { api, bytes, statusText } from "../lib/api";
import {
  createHistoryPager,
  HISTORY_PAGE_SIZE,
  initialHistoryState,
  type HistoryPagingState,
} from "../lib/historyPaging";
import CleanupResult from "../components/CleanupResult";
import "./history.css";

export function HistoryPageView({
  state,
  recent = [],
  onPage,
  onRetry,
  onOpenRecycleBin,
}: {
  state: HistoryPagingState;
  recent?: HistoryItem[];
  onPage: (page: number) => void;
  onRetry: () => void;
  onOpenRecycleBin: () => void;
}) {
  const list = useRef<HTMLDivElement>(null);
  const busy = state.status === "loading";
  const pages = Math.max(1, Math.ceil((state.total ?? 0) / HISTORY_PAGE_SIZE));
  function changePage(page: number) {
    onPage(page);
    list.current?.scrollIntoView({ block: "start" });
  }
  return (
    <section className="panel history-page">
      <CleanupResult items={recent} />
      <div className="section-heading">
        <div>
          <h2>操作历史</h2>
          <p>查看每个文件的处理结果。</p>
        </div>
        <button onClick={onOpenRecycleBin}>打开 Windows 回收站</button>
      </div>
      <div className="notice">可在 Windows 回收站中选择“还原”恢复文件。</div>
      <div className="history-list" ref={list} aria-busy={busy}>
        {busy ? (
          <div className="empty compact" role="status">
            <p>正在读取操作历史…</p>
          </div>
        ) : state.status === "error" ? (
          <div className="empty compact" role="alert">
            <p>操作历史加载失败</p>
            <small>{state.error}</small>
            <button onClick={onRetry}>重试</button>
          </div>
        ) : state.items.length === 0 ? (
          <div className="empty compact">
            <p>尚未执行清理操作</p>
          </div>
        ) : (
          state.items.map((h) => (
            <div className="history-row" key={h.id}>
              <div>
                <strong className="path-text">{h.path}</strong>
                <small>
                  {new Date(h.time * 1000).toLocaleString("zh-CN")} · 文件占用约{" "}
                  {bytes(h.bytes)}
                </small>
                {h.status !== "recycled" && <p>{h.message}</p>}
              </div>
              <span
                className={`badge ${h.status === "recycled" ? "low" : "review"}`}
              >
                {statusText(h.status)}
              </span>
            </div>
          ))
        )}
      </div>
      <div className="table-footer" aria-label="操作历史分页">
        <span>
          {state.total !== null
            ? `共 ${state.total.toLocaleString()} 条 · `
            : ""}
          每页 {HISTORY_PAGE_SIZE} 条
        </span>
        <div>
          <button
            disabled={busy || state.page === 0}
            onClick={() => changePage(state.page - 1)}
          >
            上一页
          </button>
          <span aria-label="当前页码">
            {state.page + 1} / {pages}
          </span>
          <button
            disabled={busy || state.total === null || state.page + 1 >= pages}
            onClick={() => changePage(state.page + 1)}
          >
            下一页
          </button>
        </div>
      </div>
    </section>
  );
}

export default function HistoryPage({
  onError,
  recent = [],
}: {
  onError: (e: unknown) => void;
  recent?: HistoryItem[];
}) {
  const [state, setState] = useState(initialHistoryState);
  const pager = useRef<ReturnType<typeof createHistoryPager> | null>(null);
  const recentKey = recent.map((item) => item.id).join(",");
  useEffect(() => {
    const current = createHistoryPager({ onState: setState, onError });
    pager.current = current;
    return () => {
      current.dispose();
      if (pager.current === current) pager.current = null;
    };
  }, [onError, recentKey]);
  return (
    <HistoryPageView
      state={state}
      recent={recent}
      onPage={(page) => void pager.current?.load(page)}
      onRetry={() => void pager.current?.retry()}
      onOpenRecycleBin={() =>
        void api("open_system", { target: "recycle" }).catch(onError)
      }
    />
  );
}
