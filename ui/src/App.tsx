import { useCallback, useEffect, useRef, useState } from "react";
import {
  HardDrive,
  LayoutDashboard,
  Map,
  Boxes,
  Lightbulb,
  ListChecks,
  History,
  ShieldCheck,
  Settings as SettingsIcon,
  Search,
  FolderOpen,
  AlertCircle,
  X,
  Sparkles,
} from "lucide-react";
import { api } from "./lib/api";
import type {
  Bootstrap,
  Scan,
  Settings,
  FileRecord,
  EntryPage,
  HistoryItem,
} from "./lib/types";
import ApplicationUnits from "./components/ApplicationUnits";
import EntryTable from "./components/EntryTable";
import SpaceMap from "./components/SpaceMap";
import PathBreadcrumb from "./components/PathBreadcrumb";
import ScanSummary from "./components/ScanSummary";
import SelectionBar from "./components/SelectionBar";
import ScanPicker from "./components/ScanPicker";
import DetailPanel from "./components/DetailPanel";
import OverviewPage from "./pages/OverviewPage";
import SettingsPage from "./pages/SettingsPage";
import BasketPage from "./pages/BasketPage";
import SuggestionsPage from "./pages/SuggestionsPage";
import ScanProgress from "./components/ScanProgress";
import { addSelection, selectionLimit } from "./lib/selection";
import HistoryPage from "./pages/HistoryPage";
import RulesPage from "./pages/RulesPage";
import TitleBar from "./components/TitleBar";
import { version as appVersion } from "../../package.json";
import { sameSnapshot, startScanPolling } from "./lib/scanPolling";

const navigation = [
  ["overview", "总览", LayoutDashboard],
  ["map", "空间地图", Map],
  ["apps", "应用空间", Boxes],
  ["suggestions", "清理建议", Lightbulb],
  ["basket", "待清理清单", ListChecks],
  ["history", "操作历史", History],
  ["rules", "规则中心", ShieldCheck],
  ["settings", "设置", SettingsIcon],
] as const;
type Page = (typeof navigation)[number][0];
export default function App() {
  const [boot, setBoot] = useState<Bootstrap | null>(null);
  const [error, setError] = useState("");
  const [page, setPage] = useState<Page>("overview");
  const pageRef = useRef<Page>(page);
  useEffect(() => {
    pageRef.current = page;
  }, [page]);
  const [scan, setScan] = useState<Scan | null>(null);
  const [parent, setParent] = useState("");
  const [items, setItems] = useState<FileRecord[]>([]);
  const [total, setTotal] = useState(0);
  const [index, setIndex] = useState(0);
  const [search, setSearch] = useState("");
  const [risk, setRisk] = useState("");
  const [sort, setSort] = useState("size");
  const [showMapFiles, setShowMapFiles] = useState(false);
  const main = useRef<HTMLElement>(null);
  const fileList = useRef<HTMLDivElement>(null);
  const [detail, setDetail] = useState<FileRecord | null>(null);
  const [basket, setBasket] = useState<Map<number, FileRecord>>(
    new globalThis.Map(),
  );
  const basketRef = useRef(basket);
  basketRef.current = basket;
  const [revision, setRevision] = useState(0);
  const [recentResult, setRecentResult] = useState<HistoryItem[]>([]);
  const [busy, setBusy] = useState(false);
  const fail = useCallback(
    (e: unknown) =>
      setError(
        typeof e === "string" ? e : e instanceof Error ? e.message : String(e),
      ),
    [],
  );
  const refresh = useCallback(() => setRevision((v) => v + 1), []);
  useEffect(() => {
    main.current?.scrollTo(0, 0);
  }, [page, scan?.id]);
  useEffect(() => {
    if (showMapFiles && page === "map")
      fileList.current?.scrollIntoView({ block: "start" });
  }, [showMapFiles, page]);
  useEffect(() => {
    api<Bootstrap>("bootstrap")
      .then((b) => {
        setBoot(b);
        if (b.scans[0]) {
          setScan(b.scans[0]);
          setParent(b.scans[0].root);
        }
      })
      .catch(fail);
  }, [fail]);
  const scanning =
    scan && ["queued", "scanning", "aggregating"].includes(scan.status);
  useEffect(() => {
    if (!scan) return;
    return startScanPolling({
      scanId: scan.id,
      intervalMs: scanning ? 700 : 4000,
      isOverview: () => pageRef.current === "overview",
      onStatus: (current) => {
        setScan((previous) =>
          sameSnapshot(previous, current.scan) ? previous : current.scan,
        );
        setBoot((previous) => {
          if (
            !previous ||
            (sameSnapshot(previous.scans, current.scans) &&
              sameSnapshot(previous.analysisProgress, current.analysisProgress))
          )
            return previous;
          return {
            ...previous,
            scans: current.scans,
            analysisProgress: current.analysisProgress,
          };
        });
        if (current.scan.status !== scan.status) refresh();
      },
      onBootstrap: (value) =>
        setBoot((previous) =>
          sameSnapshot(previous, value) ? previous : value,
        ),
      onError: fail,
    });
  }, [scan?.id, scan?.status, scanning, fail, refresh]);
  useEffect(() => {
    if (!scan || !(page === "map" && showMapFiles)) {
      setBusy(false);
      return;
    }
    let live = true;
    setBusy(true);
    setItems([]);
    const timer = setTimeout(async () => {
      setBusy(true);
      try {
        const q = {
          scanId: scan.id,
          parent: page === "map" ? parent : null,
          search: search || null,
          risk: risk || null,
          suggestions: false,
          minimumBytes: 0,
          offset: index * 100,
          limit: 100,
          sort,
        };
        const r = await api<EntryPage>("query_entries", { query: q });
        if (live) {
          setItems(r.items);
          setTotal(r.total);
        }
      } catch (e) {
        if (live) fail(e);
      } finally {
        if (live) setBusy(false);
      }
    }, 160);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [
    scan?.id,
    scan?.status,
    page,
    showMapFiles,
    parent,
    search,
    risk,
    index,
    sort,
    revision,
    fail,
  ]);
  useEffect(() => {
    setIndex(0);
  }, [page, parent, search, risk]);
  async function start(root: string) {
    if (scanning) {
      fail("请先完成或取消当前扫描");
      return;
    }
    try {
      const s = await api<Scan>("start_scan", { root });
      setScan(s);
      setParent(s.root);
      setBasket(new globalThis.Map());
      setDetail(null);
      setPage("suggestions");
      setSearch("");
      setRisk("");
      setItems([]);
      setShowMapFiles(false);
      refresh();
    } catch (e) {
      fail(e);
    }
  }
  async function browse() {
    try {
      const path = await api<string | null>("choose_folder");
      if (path) await start(path);
    } catch (e) {
      fail(e);
    }
  }
  function selectMany(files: FileRecord[]) {
    try {
      setBasket(addSelection(basketRef.current, files));
    } catch (error) {
      fail(error);
    }
  }
  function select(f: FileRecord) {
    if (f.assessment.risk === "protected") return;
    if (!basket.has(f.id) && basket.size >= selectionLimit) {
      fail("每次最多选择 500 项，请先处理当前已选内容。");
      return;
    }
    setBasket((current) => {
      const next = new globalThis.Map(current);
      if (next.has(f.id)) next.delete(f.id);
      else next.set(f.id, f);
      return next;
    });
  }
  async function showDetail(f: FileRecord) {
    if (!scan) return;
    setDetail(f);
    try {
      const full = await api<FileRecord>("entry_detail", {
        scanId: scan.id,
        entryId: f.id,
      });
      setDetail((current) => (current?.id === f.id ? full : current));
    } catch (e) {
      fail(e);
    }
  }
  function open(f: FileRecord) {
    if (f.isDir) {
      setDetail(null);
      setParent(f.path);
      setPage("map");
      setSearch("");
      setRisk("");
    } else void showDetail(f);
  }
  async function changed() {
    refresh();
    try {
      const b = await api<Bootstrap>("bootstrap");
      setBoot(b);
      if (detail && scan)
        setDetail(
          await api("entry_detail", { scanId: scan.id, entryId: detail.id }),
        );
    } catch (e) {
      fail(e);
    }
  }
  function save(settings: Settings) {
    setBoot((b) => (b ? { ...b, settings } : b));
    api<Bootstrap>("bootstrap").then(setBoot).catch(fail);
    refresh();
  }
  const selected = new Set(basket.keys());
  const title = navigation.find((n) => n[0] === page)?.[1];
  return (
    <div className="app-shell">
      <TitleBar onError={fail} />
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-icon">
            <HardDrive size={23} />
          </span>
          <div>
            <strong>DiskVista</strong>
            <span>磁盘空间分析与清理</span>
          </div>
        </div>
        <div className="nav-label">功能</div>
        <nav>
          {navigation.map(([id, label, Icon]) => (
            <button
              key={id}
              className={page === id ? "active" : ""}
              onClick={() => {
                setPage(id);
                setDetail(null);
                setSearch("");
                setRisk("");
              }}
            >
              <Icon size={18} />
              <span>{label}</span>
              {id === "basket" && basket.size > 0 && <em>{basket.size}</em>}
            </button>
          ))}
        </nav>
        <div className="sidebar-footer">v{appVersion}</div>
      </aside>
      <div className="workspace">
        <header className="topbar">
          <h1>{title}</h1>
          <div className="top-actions">
            {boot && (
              <span className="ai-indicator">
                <Sparkles size={14} />
                AI {boot.settings.llm.enabled ? "已启用" : "未启用"}
              </span>
            )}
            <button onClick={browse} disabled={!!scanning}>
              <FolderOpen size={16} />
              扫描文件夹
            </button>
          </div>
        </header>
        {error && (
          <div className="error-banner" role="alert">
            <AlertCircle size={18} />
            <span>{error}</span>
            <button
              className="icon-button"
              aria-label="关闭提示"
              onClick={() => setError("")}
            >
              <X size={16} />
            </button>
          </div>
        )}
        {scanning && (
          <ScanProgress
            scan={scan}
            onCancel={() => api("cancel_scan").catch(fail)}
          />
        )}
        {boot?.analysisProgress.active && (
          <div className="scan-bar ai-bar">
            <Sparkles size={15} />
            <span>
              AI 分析：已完成 {boot.analysisProgress.finished} 项，请求{" "}
              {boot.analysisProgress.requests}/
              {boot.analysisProgress.maxRequests}
            </span>
            <button onClick={() => api("cancel_analysis").catch(fail)}>
              取消分析
            </button>
          </div>
        )}
        <div className="content-and-detail">
          <main className={`main-content ${page}-page`} ref={main}>
            {!boot ? (
              <div className="empty">
                <HardDrive size={36} />
                <h2>{error ? "无法连接程序" : "正在读取设置和扫描记录"}</h2>
                <p>
                  {error
                    ? "请运行 Windows 桌面程序；普通浏览器不具备磁盘访问权限。"
                    : "读取设置与扫描记录，不会自动扫描磁盘。"}
                </p>
              </div>
            ) : (
              <>
                {scan &&
                  ["overview", "map", "apps", "suggestions"].includes(page) && (
                    <ScanPicker
                      scans={boot.scans}
                      current={scan.id}
                      disabled={!!scanning}
                      onSelect={(s) => {
                        setScan(s);
                        setParent(s.root);
                        setSearch("");
                        setRisk("");
                        setShowMapFiles(false);
                        setBasket(new globalThis.Map());
                        setDetail(null);
                        refresh();
                      }}
                    />
                  )}
                {page === "overview" && (
                  <OverviewPage
                    volumes={boot.volumes}
                    locations={boot.scanLocations ?? []}
                    disabled={!!scanning}
                    scan={scan}
                    onScan={start}
                    onBrowse={browse}
                    onMap={() => setPage("suggestions")}
                  />
                )}
                {["map", "apps", "suggestions"].includes(page) &&
                  (!scan ? (
                    <div className="empty">
                      <FolderOpen size={36} />
                      <h2>先选择一个扫描位置</h2>
                      <p>
                        扫描读取文件名、大小等基本信息，不读取普通文件正文。
                      </p>
                      <button className="primary" onClick={browse}>
                        扫描文件夹
                      </button>
                    </div>
                  ) : (
                    <>
                      {page !== "map" && <ScanSummary scan={scan} />}
                      {page === "map" && (
                        <>
                          <PathBreadcrumb
                            root={scan.root}
                            path={parent}
                            onNavigate={(path) => {
                              setParent(path);
                              setSearch("");
                              setRisk("");
                              setDetail(null);
                            }}
                          />
                          <SpaceMap
                            scanId={scan.id}
                            parent={parent}
                            revision={`${scan.status}:${revision}`}
                            onOpen={open}
                            onShowFiles={() => {
                              setShowMapFiles(true);
                              fileList.current?.scrollIntoView({
                                block: "start",
                              });
                            }}
                          />
                        </>
                      )}
                      {page === "apps" && (
                        <ApplicationUnits
                          scanId={scan.id}
                          status={scan.status}
                          revision={revision}
                          onOpen={open}
                          onDetail={setDetail}
                          onError={fail}
                        />
                      )}
                      {page === "suggestions" && (
                        <SuggestionsPage
                          key={scan.id}
                          scan={scan}
                          revision={revision}
                          selected={selected}
                          onSelect={select}
                          onSelectMany={selectMany}
                          onDetail={showDetail}
                          onOpen={open}
                          onError={fail}
                          onMap={() => {
                            setParent(scan.root);
                            setPage("map");
                          }}
                          onRescan={() => void start(scan.root)}
                        />
                      )}
                      {page === "map" && showMapFiles && (
                        <div ref={fileList} className="file-list-section">
                          {page === "map" && (
                            <div className="section-heading">
                              <h2>当前目录的文件</h2>
                              <button onClick={() => setShowMapFiles(false)}>
                                收起列表
                              </button>
                            </div>
                          )}
                          <div className="list-toolbar">
                            <div className="search-box">
                              <Search size={16} />
                              <input
                                aria-label="搜索路径"
                                placeholder="搜索路径或文件名"
                                value={search}
                                onChange={(e) => setSearch(e.target.value)}
                              />
                            </div>
                            <select
                              aria-label="风险筛选"
                              value={risk}
                              onChange={(e) => setRisk(e.target.value)}
                            >
                              <option value="">全部文件</option>
                              <option value="low">较低风险</option>
                              <option value="known">已识别用途</option>
                              <option value="unknown">未识别用途</option>
                              <option value="protected">仅看受保护项</option>
                            </select>
                            <select
                              aria-label="排序"
                              value={sort}
                              onChange={(e) => setSort(e.target.value)}
                            >
                              <option value="size">按大小</option>
                              <option value="name">按路径</option>
                              <option value="activity">按最近变化</option>
                            </select>
                            <button
                              onClick={() => scan && start(scan.root)}
                              disabled={!!scanning}
                            >
                              重新扫描
                            </button>
                          </div>
                          <EntryTable
                            items={items}
                            selected={selected}
                            onSelect={select}
                            onDetail={showDetail}
                            onOpen={open}
                            total={total}
                            page={index}
                            onPage={setIndex}
                            busy={busy}
                            emptyTitle={
                              scanning
                                ? "正在扫描这个目录"
                                : "没有符合条件的文件"
                            }
                            emptyDescription={
                              scanning
                                ? "目录内容还在读取，请等待扫描完成。"
                                : "可以返回上一级，或调整筛选条件。"
                            }
                            onClearFilters={
                              search || risk
                                ? () => {
                                    setSearch("");
                                    setRisk("");
                                  }
                                : undefined
                            }
                          />
                        </div>
                      )}
                    </>
                  ))}
                {page === "basket" && (
                  <BasketPage
                    items={[...basket.values()]}
                    scanId={scan?.id ?? ""}
                    onRemove={select}
                    onFindFiles={() => setPage("suggestions")}
                    onClear={() => setBasket(new globalThis.Map())}
                    onDone={(result) => {
                      setRecentResult(result);
                      setBasket(new globalThis.Map());
                      setPage("history");
                      refresh();
                    }}
                    onError={fail}
                  />
                )}
                {page === "history" && (
                  <HistoryPage onError={fail} recent={recentResult} />
                )}
                {page === "settings" && (
                  <SettingsPage
                    settings={boot.settings}
                    hasKey={boot.hasKey}
                    onSave={save}
                    onError={fail}
                  />
                )}
                {page === "rules" && (
                  <RulesPage
                    settings={boot.settings}
                    onSave={save}
                    onError={fail}
                  />
                )}
              </>
            )}
          </main>
          {detail && scan && boot && (
            <DetailPanel
              key={`${scan.id}:${detail.id}`}
              file={detail}
              scanId={scan.id}
              llmEnabled={boot.settings.llm.enabled}
              selected={selected.has(detail.id)}
              onClose={() => setDetail(null)}
              onSelect={() => select(detail)}
              onChanged={changed}
              onError={fail}
            />
          )}
        </div>
        {["map", "apps", "suggestions"].includes(page) && (
          <SelectionBar
            items={[...basket.values()]}
            onOpen={() => {
              setPage("basket");
              setDetail(null);
            }}
          />
        )}
      </div>
    </div>
  );
}
