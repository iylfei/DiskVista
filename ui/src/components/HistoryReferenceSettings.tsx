import { useEffect, useState, type ReactNode } from "react";
import { api, bytes } from "../lib/api";
import type { HistoryPage } from "../lib/types";
import { localeName } from "../i18n/locale";

export default function HistoryReferenceSettings({
  active,
  ids,
  busy,
  onChange,
  saveAction,
}: {
  active: boolean;
  ids: string[] | null;
  busy: boolean;
  onChange: (ids: string[] | null) => void;
  saveAction: ReactNode;
}) {
  const [page, setPage] = useState(0);
  const [data, setData] = useState<HistoryPage | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [reload, setReload] = useState(0);
  useEffect(() => {
    if (!active || ids === null) return;
    let cancelled = false;
    setLoading(true);
    setError("");
    api<HistoryPage>("history_page", {
      offset: page * 20,
      limit: 20,
      recycledOnly: true,
    })
      .then((value) => {
        if (!cancelled) setData(value);
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [active, page, ids === null, reload]);
  return (
    <section className="panel">
      <h2>历史回收参考</h2>
      <p>是否发送回收历史由语言模型配置中的授权开关控制。</p>
      <label className="check-line">
        <input
          type="radio"
          name="history-reference-mode"
          checked={ids === null}
          disabled={busy}
          onChange={() => {
            onChange(null);
            setPage(0);
          }}
        />
        默认：近 180 天内与当前文件有明确匹配线索的成功回收记录，最多 6 条
      </label>
      <label className="check-line">
        <input
          type="radio"
          name="history-reference-mode"
          checked={ids !== null}
          disabled={busy}
          onChange={() => {
            onChange([]);
            setPage(0);
          }}
        />
        手动选择记录（最多 100 条）
      </label>
      <p className="muted">
        手动选择时，选中的记录即使较旧或没有匹配线索也可作为参考。只使用本软件确认成功回收的记录；受保护、敏感或禁止发送的路径仍会排除。历史不能证明当前文件可删除，也无法确认是否已还原。超出上下文容量时只发送部分参考。
      </p>
      {ids !== null && <p>{`已选择 ${ids.length} 条；不勾选则不发送历史。`}</p>}
      {ids !== null && (
        <div aria-busy={loading}>
          {loading ? (
            <p role="status">正在读取操作历史…</p>
          ) : error ? (
            <p role="alert">{error}</p>
          ) : data?.items.length === 0 ? (
            <p>暂无成功回收记录</p>
          ) : (
            data?.items.map((item) => (
              <label className="history-reference-row" key={item.id}>
                <input
                  type="checkbox"
                  disabled={
                    busy || (ids.length >= 100 && !ids.includes(item.id))
                  }
                  checked={ids.includes(item.id)}
                  onChange={(e) =>
                    onChange(
                      e.target.checked
                        ? [...(ids ?? []), item.id]
                        : (ids ?? []).filter((id) => id !== item.id),
                    )
                  }
                />
                <span>
                  <strong className="path-text">{item.path}</strong>
                  <small>
                    {new Date(item.time * 1000).toLocaleString(localeName())} ·{" "}
                    {bytes(item.bytes)}
                  </small>
                </span>
              </label>
            ))
          )}
        </div>
      )}
      {ids !== null && (
        <div className="actions history-reference-pager">
          <button disabled={loading} onClick={() => setReload((n) => n + 1)}>
            刷新记录
          </button>
          <>
            <button
              disabled={loading || page === 0}
              onClick={() => setPage((n) => n - 1)}
            >
              上一页
            </button>
            <span>
              {page + 1} / {Math.max(1, Math.ceil((data?.total ?? 0) / 20))}
            </span>
            <button
              disabled={loading || !data || (page + 1) * 20 >= data.total}
              onClick={() => setPage((n) => n + 1)}
            >
              下一页
            </button>
          </>
        </div>
      )}
      {saveAction}
    </section>
  );
}
