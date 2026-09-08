import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Boxes, FolderOpen, Info, Search } from "lucide-react";
import { api, bytes } from "../lib/api";
import { useSearchReady } from "../lib/useSearchReady";
import type { ApplicationUnitPage, FileRecord } from "../lib/types";
import { startFileDetailLoad } from "../lib/fileDetail";
import { unitCleanupBlockReason } from "../lib/cleanupTarget";
import {
  applicationRows,
  canExpand,
  expandedSearchResults,
} from "../lib/applicationTree";
import { UnitSummary, ComponentSummary } from "./ApplicationUnitRow";
import ApplicationFileView from "./ApplicationFileView";
import HelpTip from "./HelpTip";
import AddToBasketButton from "./AddToBasketButton";
import { helpText } from "../lib/helpText";
import { translateText } from "../i18n/translate";
import "./units.css";

export { UnitSummary } from "./ApplicationUnitRow";

export default function ApplicationUnits({
  scanId,
  status,
  revision,
  onDetail,
  onCloseDetail,
  onAddToBasket,
  queued,
  adding,
  onError,
}: {
  scanId: string;
  status: string;
  revision: number;
  onDetail: (f: FileRecord) => void;
  onCloseDetail: () => void;
  onAddToBasket: (entryId: number) => void;
  queued: Set<number>;
  adding: Set<number>;
  onError: (e: unknown) => void;
}) {
  const [data, setData] = useState<ApplicationUnitPage | null>(null);
  const [search, setSearch] = useState("");
  const searchReady = useSearchReady(search);
  const [page, setPage] = useState(0);
  const [busy, setBusy] = useState(true);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [reload, setReload] = useState(0);
  const [browseTarget, setBrowseTarget] = useState<{
    file: FileRecord;
    title: string;
  } | null>(null);
  const scroll = useRef<HTMLDivElement>(null);
  const listScrollTop = useRef(0);
  const restoreListScroll = useRef(false);
  const cancelInspection = useRef<(() => void) | null>(null);
  useLayoutEffect(
    () => () => cancelInspection.current?.(),
    [scanId, status, search, page, revision, reload],
  );
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
  useLayoutEffect(() => {
    if (!browseTarget && restoreListScroll.current && scroll.current) {
      scroll.current.scrollTop = listScrollTop.current;
      restoreListScroll.current = false;
    }
  }, [browseTarget, visible.length]);
  useEffect(() => {
    setPage(0);
    setData(null);
    setExpanded(new Set());
    setBrowseTarget(null);
    listScrollTop.current = 0;
  }, [scanId]);
  useEffect(() => {
    if (pending) return;
    let live = true;
    setBusy(true);
    if (!searchReady) return;
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
    return () => {
      live = false;
    };
  }, [
    scanId,
    status,
    pending,
    search,
    searchReady,
    page,
    revision,
    reload,
    onError,
  ]);

  function inspect(
    entryId: number,
    action: "open" | "detail",
    title = "应用文件",
  ) {
    cancelInspection.current?.();
    const previousScrollTop = scroll.current?.scrollTop ?? 0;
    cancelInspection.current = startFileDetailLoad({
      scanId,
      entryId,
      onResult:
        action === "open"
          ? (file) => {
              if (!file.isDir) {
                onDetail(file);
                return;
              }
              listScrollTop.current = previousScrollTop;
              setBrowseTarget({ file, title });
              onCloseDetail();
            }
          : onDetail,
      onError,
    });
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
  if (browseTarget)
    return (
      <section className="application-units" aria-label="应用文件">
        <ApplicationFileView
          scanId={scanId}
          status={status}
          revision={revision}
          root={browseTarget.file}
          title={browseTarget.title}
          queued={queued}
          adding={adding}
          onBack={() => {
            restoreListScroll.current = true;
            setBrowseTarget(null);
            onCloseDetail();
          }}
          onDetail={(file) => inspect(file.id, "detail")}
          onCloseDetail={onCloseDetail}
          onAddToBasket={onAddToBasket}
          onError={onError}
        />
      </section>
    );
  return (
    <section className="application-units" aria-label="应用与文件夹占用">
      <p className="unit-intro">
        展开应用可查看安装与数据位置，未归属内容单独列出。应用识别仅供参考，不代表可以删除。
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
            {translateText("合计")} {bytes(data.occupiedBytes)}
            {data.estimated ? translateText("（估算）") : ""}
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
                            void inspect(
                              unit.components[0].entryId,
                              "open",
                              unit.name,
                            );
                        }}
                      />
                      <div className="unit-actions">
                        {unit.components.length === 1 && (
                          <button
                            className="icon-button unit-inspect"
                            aria-label={`${unit.children.length ? "查看全部文件" : "查看详情"} ${unit.name}`}
                            title={
                              unit.children.length ? "查看全部文件" : "查看详情"
                            }
                            onClick={() =>
                              void inspect(
                                unit.components[0].entryId,
                                unit.children.length ? "open" : "detail",
                                unit.name,
                              )
                            }
                          >
                            {unit.children.length ? (
                              <FolderOpen size={16} />
                            ) : (
                              <Info size={16} />
                            )}
                          </button>
                        )}
                        <AddToBasketButton
                          name={unit.name}
                          disabled={busy}
                          queued={
                            unit.components.length === 1 &&
                            queued.has(unit.components[0].entryId)
                          }
                          adding={
                            unit.components.length === 1 &&
                            adding.has(unit.components[0].entryId)
                          }
                          disabledReason={unitCleanupBlockReason(unit)}
                          onClick={() => {
                            if (!unitCleanupBlockReason(unit))
                              onAddToBasket(unit.components[0].entryId);
                          }}
                        />
                      </div>
                    </>
                  ) : (
                    <ComponentSummary
                      component={item.component}
                      onOpen={() =>
                        void inspect(
                          item.component.entryId,
                          "open",
                          item.component.path,
                        )
                      }
                      onDetail={() =>
                        void inspect(item.component.entryId, "detail")
                      }
                      onAddToBasket={() =>
                        onAddToBasket(item.component.entryId)
                      }
                      busy={busy}
                      queued={queued.has(item.component.entryId)}
                      adding={adding.has(item.component.entryId)}
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
          {translateText("共")} {data?.total ?? 0} {translateText("项").trim()}
          {expanded.size > 0 ? translateText(" · 已展开子项") : ""}
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
