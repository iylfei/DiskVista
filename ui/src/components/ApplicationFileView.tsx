import { useEffect, useRef, useState } from "react";
import { ArrowLeft, Search } from "lucide-react";
import { api } from "../lib/api";
import { useSearchReady } from "../lib/useSearchReady";
import type { EntryPage, FileRecord } from "../lib/types";
import EntryTable from "./EntryTable";
import PathBreadcrumb from "./PathBreadcrumb";
import SpaceMap from "./SpaceMap";

const emptySelection = new Set<number>();

export default function ApplicationFileView({
  scanId,
  status,
  revision,
  root,
  title,
  queued,
  adding,
  onBack,
  onDetail,
  onCloseDetail,
  onAddToBasket,
  onError,
}: {
  scanId: string;
  status: string;
  revision: number;
  root: FileRecord;
  title: string;
  queued: Set<number>;
  adding: Set<number>;
  onBack: () => void;
  onDetail: (file: FileRecord) => void;
  onCloseDetail: () => void;
  onAddToBasket: (entryId: number) => void;
  onError: (error: unknown) => void;
}) {
  const [parent, setParent] = useState(root.path);
  const [items, setItems] = useState<FileRecord[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(0);
  const queryScope = useRef("");
  const [search, setSearch] = useState("");
  const searchReady = useSearchReady(search);
  const [sort, setSort] = useState("size");
  const [busy, setBusy] = useState(true);
  const [reload, setReload] = useState(0);
  const fileList = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setParent(root.path);
    setItems([]);
    setTotal(0);
    setPage(0);
    setSearch("");
  }, [root.id, root.path]);

  useEffect(() => {
    if (status !== "complete") return;
    let live = true;
    setBusy(true);
    setItems([]);
    const scope = JSON.stringify([scanId, parent, search, sort]);
    if (queryScope.current !== scope) {
      queryScope.current = scope;
      if (page !== 0) {
        setPage(0);
        return;
      }
    }
    if (!searchReady) return;
    api<EntryPage>("query_entries", {
      query: {
        scanId,
        parent,
        search: search || null,
        risk: null,
        analysisStatus: "",
        suggestions: false,
        minimumBytes: 0,
        offset: page * 100,
        limit: 100,
        sort,
      },
    })
      .then((result) => {
        if (!live) return;
        setPage((current) =>
          Math.min(current, Math.max(0, Math.ceil(result.total / 100) - 1)),
        );
        setItems(result.items);
        setTotal(result.total);
      })
      .catch((error) => {
        if (live) onError(error);
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
    revision,
    parent,
    search,
    searchReady,
    sort,
    page,
    reload,
    onError,
  ]);

  useEffect(() => setPage(0), [parent, search, sort]);

  function navigate(path: string) {
    onCloseDetail();
    setParent(path);
    setSearch("");
    setPage(0);
  }

  function open(file: FileRecord) {
    if (file.isDir) navigate(file.path);
    else onDetail(file);
  }

  return (
    <div className="application-file-view">
      <header className="application-file-heading">
        <button type="button" onClick={onBack}>
          <ArrowLeft size={16} />
          返回
        </button>
        <div>
          <h2>{title}</h2>
          <small className="path-text">{root.path}</small>
        </div>
      </header>
      <PathBreadcrumb root={root.path} path={parent} onNavigate={navigate} />
      <SpaceMap
        scanId={scanId}
        parent={parent}
        revision={`${status}:${revision}`}
        cacheable={status === "complete"}
        queued={queued}
        adding={adding}
        disabledReason={
          status === "complete" ? null : "请先完成扫描，再加入待清理清单"
        }
        onOpen={open}
        onShowFiles={() => fileList.current?.scrollIntoView({ block: "start" })}
        onAddToBasket={(file) => onAddToBasket(file.id)}
      />
      <div ref={fileList} className="file-list-section application-file-list">
        <div className="section-heading">
          <h2>当前目录的文件</h2>
        </div>
        <div className="list-toolbar file-filters">
          <div className="search-box">
            <Search size={16} />
            <input
              aria-label="搜索应用文件"
              placeholder="搜索当前目录"
              value={search}
              onChange={(event) => setSearch(event.target.value)}
            />
          </div>
          <select
            aria-label="应用文件排序"
            value={sort}
            onChange={(event) => setSort(event.target.value)}
          >
            <option value="size">按大小</option>
            <option value="name">按路径</option>
            <option value="activity_desc">最近变化：从新到旧</option>
            <option value="activity_asc">最近变化：从旧到新</option>
          </select>
          <button
            type="button"
            disabled={busy}
            onClick={() => setReload((value) => value + 1)}
          >
            刷新
          </button>
        </div>
        <EntryTable
          scanId={scanId}
          analysisRevision={String(revision)}
          items={items}
          selected={emptySelection}
          queued={queued}
          adding={adding}
          total={total}
          page={page}
          busy={busy}
          onSelect={(file) => onAddToBasket(file.id)}
          onDetail={onDetail}
          onOpen={open}
          onAddToBasket={(file) => onAddToBasket(file.id)}
          onPage={setPage}
          emptyTitle="当前目录没有文件"
          emptyDescription="可以返回上一级，或调整搜索条件。"
          onClearFilters={search ? () => setSearch("") : undefined}
        />
      </div>
    </div>
  );
}
