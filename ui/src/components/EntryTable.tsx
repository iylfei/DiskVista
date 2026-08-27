import { useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Folder, File, ChevronRight, ShieldCheck } from "lucide-react";
import type { FileRecord } from "../lib/types";
import { bytes, date, riskText } from "../lib/api";
import HelpTip from "./HelpTip";
import { helpText } from "../lib/helpText";
import { fileSource } from "../lib/filePresentation";
interface Props {
  emptyTitle?: string;
  emptyDescription?: string;
  onClearFilters?: () => void;
  quietReview?: boolean;
  items: FileRecord[];
  selected: Set<number>;
  queued?: Set<number>;
  onSelect: (file: FileRecord) => void;
  onDetail: (file: FileRecord) => void;
  onOpen: (file: FileRecord) => void;
  total: number;
  page: number;
  onPage: (page: number) => void;
  busy?: boolean;
}
export default function EntryTable({
  items,
  selected,
  queued,
  onSelect,
  onDetail,
  onOpen,
  total,
  page,
  onPage,
  busy,
  quietReview = false,
  emptyTitle = "这里暂时没有项目",
  emptyDescription = "可以返回上一级，或调整搜索条件。",
  onClearFilters,
}: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const rows = useVirtualizer({
    count: items.length,
    getScrollElement: () => ref.current,
    estimateSize: () => 65,
    overscan: 6,
  });
  return (
    <div className="table-wrap">
      <div className="file-row table-head">
        <span />
        <span>名称 / 路径</span>
        <span>
          占用空间
          <HelpTip label="实际占用" text={helpText.diskUsage} />
        </span>
        <span>类型 / 所属应用</span>
        <span>
          提示
          <HelpTip label="清理风险" text={helpText.risk} />
        </span>
        <span />
      </div>
      <div className="table-scroll" ref={ref} aria-busy={busy}>
        {items.length === 0 ? (
          <div className="empty compact">
            <Folder size={28} />
            <p>{busy ? "正在载入扫描结果…" : emptyTitle}</p>
            {!busy && <small>{emptyDescription}</small>}
            {!busy && onClearFilters && (
              <button onClick={onClearFilters}>清除筛选</button>
            )}
          </div>
        ) : (
          <div style={{ height: rows.getTotalSize(), position: "relative" }}>
            {rows.getVirtualItems().map((row) => {
              const f = items[row.index];
              const protectedItem = f.assessment.risk === "protected";
              const inBasket = queued?.has(f.id) ?? false;
              return (
                <div
                  className="file-row data-row"
                  key={f.id}
                  style={{
                    height: row.size,
                    transform: `translateY(${row.start}px)`,
                  }}
                >
                  <input
                    type="checkbox"
                    aria-label={`选择 ${f.name}`}
                    checked={inBasket || selected.has(f.id)}
                    disabled={protectedItem || inBasket}
                    title={
                      inBasket
                        ? "已在待清理清单中"
                        : (f.assessment.protectedReason ?? "选择此项")
                    }
                    onChange={() => onSelect(f)}
                  />
                  <button
                    className="file-name"
                    onClick={() => onDetail(f)}
                    title={f.path}
                  >
                    {f.isDir ? <Folder size={19} /> : <File size={18} />}
                    <span>
                      <strong>{f.name || f.path}</strong>
                      <small>{f.path}</small>
                    </span>
                  </button>
                  <div className="size-cell">
                    <strong>{bytes(f.allocatedBytes ?? f.logicalBytes)}</strong>
                    {f.allocatedBytes === null && <small>估算</small>}
                  </div>
                  <div className="ellipsis">
                    <span>{fileSource(f)}</span>
                    {f.assessment.category !== "unknown" && (
                      <small title={f.assessment.purpose}>
                        {f.assessment.purpose}
                      </small>
                    )}
                  </div>
                  <div>
                    {!(
                      quietReview &&
                      ["review", "unknown"].includes(f.assessment.risk)
                    ) && (
                      <span className={`badge ${f.assessment.risk}`}>
                        {protectedItem && <ShieldCheck size={12} />}{" "}
                        {riskText(f.assessment.risk)}
                      </span>
                    )}
                    <small>
                      {!f.complete
                        ? "不完整"
                        : `最近变化 ${date(f.latestChange)}`}
                    </small>
                  </div>
                  <button
                    className="icon-button"
                    aria-label={f.isDir ? `打开 ${f.name}` : `查看 ${f.name}`}
                    onClick={() => (f.isDir ? onOpen(f) : onDetail(f))}
                  >
                    <ChevronRight size={16} />
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </div>
      <div className="table-footer">
        <span>共 {total.toLocaleString()} 项 · 每页 100 项</span>
        <div>
          <button
            disabled={page === 0 || busy}
            onClick={() => {
              ref.current?.scrollTo(0, 0);
              onPage(page - 1);
            }}
          >
            上一页
          </button>
          <span>
            {page + 1} / {Math.max(1, Math.ceil(total / 100))}
          </span>
          <button
            disabled={(page + 1) * 100 >= total || busy}
            onClick={() => {
              ref.current?.scrollTo(0, 0);
              onPage(page + 1);
            }}
          >
            下一页
          </button>
        </div>
      </div>
    </div>
  );
}
