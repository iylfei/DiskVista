import { useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Boxes, FolderOpen, Search } from "lucide-react";
import { api, bytes } from "../lib/api";
import type { ApplicationUnitPage, FileRecord } from "../lib/types";
import {
  applicationRows,
  canExpand,
  expandedSearchResults,
} from "../lib/applicationTree";
import { UnitSummary, ComponentSummary } from "./ApplicationUnitRow";
import HelpTip from "./HelpTip";
import { helpText } from "../lib/helpText";
import "./units.css";

export { UnitSummary } from "./ApplicationUnitRow";

export default function ApplicationUnits({
  scanId,
  status,
  revision,
  onOpen,
  onDetail,
  onError,
}: {
  scanId: string;
  status: string;
  revision: number;
  onOpen: (f: FileRecord) => void;
  onDetail: (f: FileRecord) => void;
  onError: (e: unknown) => void;
}) {
  const [data, setData] = useState<ApplicationUnitPage | null>(null);
  const [search, setSearch] = useState("");
  const [page, setPage] = useState(0);
  const [busy, setBusy] = useState(true);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [reload, setReload] = useState(0);
  const scroll = useRef<HTMLDivElement>(null);
  const pending = status !== "complete";
  const visible = useMemo(
    () => applicationRows(data?.items ?? [], expanded),
    [data, expanded],
  );
  const rows = useVirtualizer({
    count: visible.length,
    getScrollElement: () => scroll.current,
    estimateSize: () => 64,
    overscan: 6,
    getItemKey: (index) => visible[index].key,
  });
  useEffect(() => {
    setPage(0);
    setData(null);
    setExpanded(new Set());
  }, [scanId]);
  useEffect(() => {
    if (pending) return;
    let live = true;
    setBusy(true);
    const timer = setTimeout(() => {
      api<ApplicationUnitPage>("application_units", {
        scanId,
        search,
        offset: page * 100,
        limit: 100,
      })
        .then((result) => {
          if (!live) return;
          setData(result);
          setExpanded(search ? expandedSearchResults(result.items) : new Set());
        })
        .catch((e) => {
          if (live) onError(e);
        })
        .finally(() => {
          if (live) setBusy(false);
        });
    }, 180);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [scanId, status, pending, search, page, revision, reload, onError]);

  async function inspect(entryId: number, open: boolean) {
    try {
      const file = await api<FileRecord>("entry_detail", { scanId, entryId });
      if (open) onOpen(file);
      else onDetail(file);
    } catch (e) {
      onError(e);
    }
  }
  function toggle(id: string) {
    setExpanded((old) => {
      const next = new Set(old);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }
  function changePage(next: number) {
    setPage(next);
    scroll.current?.scrollTo(0, 0);
  }
  if (pending)
    return (
      <div className="notice">
        <Boxes size={20} />
        <span>
          {["queued", "scanning", "aggregating"].includes(status)
            ? "正在扫描，完成后可查看各应用和文件夹的占用。"
            : "本次扫描没有完成。请选择已完成的扫描记录，或重新扫描。"}
        </span>
      </div>
    );
  return (
    <section className="application-units" aria-label="应用与文件夹占用">
      <p className="unit-intro">
        点击箭头展开子项，点击“查看文件”进入目录。应用识别仅供参考，不代表可以删除。
        <HelpTip label="应用如何分组" text={helpText.appBoundary} />
      </p>
      <div className="list-toolbar">
        <div className="search-box">
          <Search size={16} />
          <input
            aria-label="搜索应用"
            placeholder="搜索应用名称或所在路径"
            value={search}
            onChange={(e) => {
              setSearch(e.target.value);
              changePage(0);
            }}
          />
        </div>
        {data && (
          <span className="unit-total">
            合计 {bytes(data.occupiedBytes)}
            {data.estimated ? "（估算）" : ""}
            <HelpTip label="实际占用" text={helpText.diskUsage} />
          </span>
        )}
        <button onClick={() => setReload((v) => v + 1)} disabled={busy}>
          重新统计
        </button>
      </div>
      <div className="unit-list" ref={scroll} aria-busy={busy}>
        {!visible.length ? (
          <div className="empty compact">
            <Boxes size={30} />
            <p>{busy ? "正在统计占用…" : "没有找到匹配的应用或文件夹"}</p>
          </div>
        ) : (
          <div style={{ height: rows.getTotalSize(), position: "relative" }}>
            {rows.getVirtualItems().map((row) => {
              const item = visible[row.index];
              const unit = item.unit;
              return (
                <div
                  key={item.key}
                  className="unit-tree-row"
                  data-depth={item.depth}
                  style={{
                    position: "absolute",
                    top: 0,
                    left: 0,
                    width: "100%",
                    height: row.size,
                    transform: `translateY(${row.start}px)`,
                    paddingLeft: Math.min(item.depth, 8) * 24,
                  }}
                >
                  {unit ? (
                    <>
                      <UnitSummary
                        unit={unit}
                        expanded={expanded.has(unit.id)}
                        onClick={() => {
                          if (canExpand(unit)) toggle(unit.id);
                          else if (unit.components[0])
                            void inspect(unit.components[0].entryId, true);
                        }}
                      />
                      {unit.children.length > 0 &&
                        unit.components.length === 1 && (
                          <button
                            className="icon-button unit-inspect"
                            aria-label={`查看全部文件 ${unit.name}`}
                            title="查看全部文件"
                            onClick={() =>
                              void inspect(unit.components[0].entryId, true)
                            }
                          >
                            <FolderOpen size={16} />
                          </button>
                        )}
                    </>
                  ) : (
                    <ComponentSummary
                      component={item.component}
                      onOpen={() => void inspect(item.component.entryId, true)}
                      onDetail={() =>
                        void inspect(item.component.entryId, false)
                      }
                    />
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>
      <div className="table-footer">
        <span>
          共 {data?.total ?? 0} 项{expanded.size > 0 ? " · 已展开子项" : ""}
        </span>
        <div>
          <button
            disabled={page === 0 || busy}
            onClick={() => changePage(page - 1)}
          >
            上一页
          </button>
          <span>
            {page + 1} / {Math.max(1, Math.ceil((data?.total ?? 0) / 100))}
          </span>
          <button
            disabled={(page + 1) * 100 >= (data?.total ?? 0) || busy}
            onClick={() => changePage(page + 1)}
          >
            下一页
          </button>
        </div>
      </div>
    </section>
  );
}
