import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
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
import AnalysisFilter from "./components/AnalysisFilter";
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
import ScanAnalysisButton from "./components/ScanAnalysisButton";
import { useCleanupSelection } from "./lib/useCleanupSelection";
import HistoryPage from "./pages/HistoryPage";
import RulesPage from "./pages/RulesPage";
import TitleBar from "./components/TitleBar";
import { version as appVersion } from "../../package.json";
import { sameSnapshot, startScanPolling } from "./lib/scanPolling";
import { mergeFileDetail } from "./lib/fileDetail";
import { afterScanDeletion, excludeDeletedScans } from "./lib/scanRecords";

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
  const [scan, setScan] = useState<Scan | null>(null);
  const scanIdRef = useRef<string | null>(null);
  const deletedScans = useRef(new Set<string>());
  const deletingScan = useRef<string | null>(null);
  useLayoutEffect(() => {
    pageRef.current = page;
    scanIdRef.current = scan?.id ?? null;
  }, [page, scan?.id]);
  const [parent, setParent] = useState("");
  const [items, setItems] = useState<FileRecord[]>([]);
  const [total, setTotal] = useState(0);
  const [index, setIndex] = useState(0);
  const [search, setSearch] = useState("");
  const [risk, setRisk] = useState("");
  const [analysisStatus, setAnalysisStatus] = useState("");
  const [sort, setSort] = useState("size");
  const [showMapFiles, setShowMapFiles] = useState(false);
  const main = useRef<HTMLElement>(null);
  const fileList = useRef<HTMLDivElement>(null);
  const [detail, setDetail] = useState<FileRecord | null>(null);
  const [revision, setRevision] = useState(0);
  const [recentResult, setRecentResult] = useState<HistoryItem[]>([]);
  const [busy, setBusy] = useState(false);
  const [cleaning, setCleaningState] = useState(false);
  const cleaningRef = useRef(false);
  const setCleaning = useCallback((value: boolean) => {
    cleaningRef.current = value;
    setCleaningState(value);
  }, []);
  const [dismissedAnalysis, setDismissedAnalysis] = useState("");
  const analysis = boot?.analysisProgress;
  const analysisRevision = `${revision}:${
    analysis?.scanId === scan?.id
      ? `${analysis?.finished ?? 0}:${analysis?.active ?? false}`
      : "idle"
  }`;
  const filteredAnalysisRevision = analysisStatus ? analysisRevision : "";
  useEffect(() => {
    if (boot?.analysisProgress.active) setDismissedAnalysis("");
  }, [boot?.analysisProgress.active]);
  const fail = useCallback(
    (e: unknown) =>
      setError(
        typeof e === "string" ? e : e instanceof Error ? e.message : String(e),
      ),
    [],
  );
  const refresh = useCallback(() => setRevision((v) => v + 1), []);
  const {
    basket,
    pending,
    adding,
    message: selectionMessage,
    select,
    selectMany,
    addToBasket,
    addEntryToBasket,
    removeFromBasket,
    removeRecycled,
    clearBasket,
    reset: resetSelection,
  } = useCleanupSelection(fail);
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
      intervalMs: scanning ? 700 : boot?.analysisProgress.active ? 1000 : 4000,
      isOverview: () => pageRef.current === "overview",
      onStatus: (current) => {
        if (deletingScan.current || deletedScans.current.has(current.scan.id))
          return;
        current = excludeDeletedScans(current, deletedScans.current);
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
      onBootstrap: (value) => {
        if (deletingScan.current) return;
        value = excludeDeletedScans(value, deletedScans.current);
        setBoot((previous) =>
          sameSnapshot(previous, value) ? previous : value,
        );
      },
      onError: (error) => {
        if (!deletingScan.current && !deletedScans.current.has(scan.id))
          fail(error);
      },
    });
  }, [
    scan?.id,
    scan?.status,
    scanning,
    boot?.analysisProgress.active,
    fail,
    refresh,
  ]);
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
          analysisStatus,
          suggestions: false,
          minimumBytes: 0,
          offset: index * 100,
          limit: 100,
          sort,
        };
        const r = await api<EntryPage>("query_entries", { query: q });
        if (live) {
          setIndex((previous) =>
            Math.min(previous, Math.max(0, Math.ceil(r.total / 100) - 1)),
          );
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
    analysisStatus,
    index,
    sort,
    revision,
    filteredAnalysisRevision,
    fail,
  ]);
  useEffect(() => {
    setIndex(0);
  }, [scan?.id, page, parent, search, risk, analysisStatus, sort]);
  async function start(root: string) {
    if (cleaning) return;
    if (scanning) {
      fail("请先完成或取消当前扫描");
      return;
    }
    try {
      const s = await api<Scan>("start_scan", { root });
      setScan(s);
      setParent(s.root);
      resetSelection();
      setDetail(null);
      setPage("suggestions");
      setSearch("");
      setRisk("");
      setAnalysisStatus("");
      setItems([]);
      setShowMapFiles(false);
      refresh();
    } catch (e) {
      fail(e);
    }
  }
  async function browse() {
    if (cleaning) return;
    try {
      const path = await api<string | null>("choose_folder");
      if (path) await start(path);
    } catch (e) {
      fail(e);
    }
  }
  async function showDetail(f: FileRecord) {
    if (!scan || scanIdRef.current !== scan.id) return;
    const target = { scanId: scan.id, entryId: f.id };
    setDetail(f);
    try {
      const full = await api<FileRecord>("entry_detail", target);
      setDetail((current) =>
        mergeFileDetail(current, scanIdRef.current, target, full),
      );
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
      setAnalysisStatus("");
    } else void showDetail(f);
  }
  function queueEntry(entryId: number) {
    if (!scan || scan.id !== scanIdRef.current || cleaningRef.current) return;
    if (scan.status !== "complete") {
      fail("请先完成扫描，再加入待清理清单");
      return;
    }
    addEntryToBasket(scan.id, entryId);
  }
  async function changed() {
    const target =
      detail && scan ? { scanId: scan.id, entryId: detail.id } : null;
    try {
      const b = await api<Bootstrap>("bootstrap");
      setBoot(excludeDeletedScans(b, deletedScans.current));
      if (target && scanIdRef.current === target.scanId) {
        const full = await api<FileRecord>("entry_detail", target);
        setDetail((current) =>
          mergeFileDetail(current, scanIdRef.current, target, full),
        );
      }
    } catch (e) {
      fail(e);
    } finally {
      refresh();
    }
  }
  function save(settings: Settings) {
    setBoot((b) => (b ? { ...b, settings } : b));
    api<Bootstrap>("bootstrap")
      .then((b) => setBoot(excludeDeletedScans(b, deletedScans.current)))
      .catch(fail);
    refresh();
  }
  const selected = new Set(pending.keys());
  async function deleteScan(target: Scan) {
    if (deletingScan.current) throw new Error("已有扫描记录正在删除");
    deletingScan.current = target.id;
    try {
      const remaining = await api<Scan[]>("delete_scan", { scanId: target.id });
      deletedScans.current.add(target.id);
      setBoot((b) => b && afterScanDeletion(b, remaining, target.id));
      if (scanIdRef.current === target.id) {
        const next = remaining[0] ?? null;
        scanIdRef.current = next?.id ?? null;
        setScan(next);
        setParent(next?.root ?? "");
        setSearch("");
        setRisk("");
        setAnalysisStatus("");
        setShowMapFiles(false);
        setItems([]);
        setTotal(0);
        setIndex(0);
        resetSelection();
        setDetail(null);
      }
      refresh();
    } finally {
      deletingScan.current = null;
    }
  }
  const queued = new Set(basket.keys());
  const title = navigation.find((n) => n[0] === page)?.[1];
  const analysisMessageKey = `${analysis?.scanId}:${analysis?.message}`;
  return (
    <div className="app-shell">
      <TitleBar onError={fail} />
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-icon">
            <img
              src="/app-icon.png"
              width={38}
              height={38}
              alt=""
              draggable={false}
            />
          </span>
          <div>
            <strong>DiskVista</strong>
            <span>空间分析与清理</span>
          </div>
        </div>
        <div className="nav-label">功能</div>
        <nav>
          {navigation.map(([id, label, Icon]) => (
            <button
              key={id}
              disabled={cleaning}
              className={page === id ? "active" : ""}
              onClick={() => {
                setPage(id);
                setDetail(null);
                setSearch("");
                setRisk("");
                setAnalysisStatus("");
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
              <ScanAnalysisButton
                settings={boot.settings.llm}
                scan={scan}
                active={boot.analysisProgress.active}
                disabled={cleaning}
                onSettings={() => {
                  setDetail(null);
                  setPage("settings");
                }}
                onStarted={(progress) => {
                  setError("");
                  setDismissedAnalysis("");
                  setBoot((b) => b && { ...b, analysisProgress: progress });
                  refresh();
                }}
                onError={fail}
              />
            )}
            <button onClick={browse} disabled={!!scanning || cleaning}>
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
        {analysis &&
          (analysis.active ||
            (analysis.message && analysisMessageKey !== dismissedAnalysis)) && (
            <div className="scan-bar ai-bar" role="status">
              <Sparkles size={15} />
              <span>
                {analysis.active
                  ? `AI 分析：已处理 ${analysis.finished}/${analysis.queued} 项，请求 ${analysis.requests}/${analysis.maxRequests}`
                  : analysis.message}
                {analysis.active && analysis.message && (
                  <small>{analysis.message}</small>
                )}
                {!analysis.active && analysis.finished > 0 && (
                  <small>分析结果可在对应文件详情的“AI 辅助解释”中查看。</small>
                )}
                {analysis.scanId && analysis.scanId !== scan?.id && (
                  <small>
                    分析位置：
                    {boot?.scans.find((s) => s.id === analysis.scanId)?.root ??
                      "其他扫描记录"}
                  </small>
                )}
              </span>
              {analysis.active ? (
                <button onClick={() => api("cancel_analysis").catch(fail)}>
                  取消分析
                </button>
              ) : (
                <button
                  className="icon-button"
                  aria-label="关闭 AI 分析提示"
                  onClick={() => setDismissedAnalysis(analysisMessageKey)}
                >
                  <X size={16} />
                </button>
              )}
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
                      disabled={!!scanning || cleaning}
                      deleteDisabled={
                        !!scanning || cleaning || boot.analysisProgress.active
                      }
                      onDelete={deleteScan}
                      onSelect={(s) => {
                        setScan(s);
                        setParent(s.root);
                        setSearch("");
                        setRisk("");
                        setAnalysisStatus("");
                        setShowMapFiles(false);
                        resetSelection();
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
                      <p>扫描后可查看空间分布与清理建议。</p>
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
                              setAnalysisStatus("");
                              setDetail(null);
                            }}
                          />
                          <SpaceMap
                            scanId={scan.id}
                            parent={parent}
                            revision={`${scan.status}:${revision}`}
                            cacheable={scan.status === "complete"}
                            queued={queued}
                            adding={adding}
                            disabledReason={
                              cleaning
                                ? "正在回收，请等待操作完成"
                                : scan.status !== "complete"
                                  ? "请先完成扫描，再加入待清理清单"
                                  : null
                            }
                            onAddToBasket={(file) => queueEntry(file.id)}
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
                          onOpen={(file) => {
                            open(file);
                            if (file.isDir) setShowMapFiles(true);
                          }}
                          onDetail={setDetail}
                          onAddToBasket={queueEntry}
                          queued={queued}
                          adding={adding}
                          onError={fail}
                        />
                      )}
                      {page === "suggestions" && (
                        <SuggestionsPage
                          key={scan.id}
                          scan={scan}
                          revision={revision}
                          analysisRevision={analysisRevision}
                          selected={selected}
                          queued={queued}
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
                          <div className="list-toolbar file-filters">
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
                            <AnalysisFilter
                              value={analysisStatus}
                              onChange={(value) => {
                                setAnalysisStatus(value);
                                setIndex(0);
                              }}
                            />
                            <select
                              aria-label="排序"
                              value={sort}
                              onChange={(e) => {
                                setSort(e.target.value);
                                setIndex(0);
                              }}
                            >
                              <option value="size">按大小</option>
                              <option value="name">按路径</option>
                              <option value="activity_desc">
                                最近变化：从新到旧
                              </option>
                              <option value="activity_asc">
                                最近变化：从旧到新
                              </option>
                            </select>
                            <button
                              onClick={() => scan && start(scan.root)}
                              disabled={!!scanning}
                            >
                              重新扫描
                            </button>
                          </div>
                          <EntryTable
                            scanId={scan.id}
                            analysisRevision={analysisRevision}
                            items={items}
                            selected={selected}
                            queued={queued}
                            onSelect={select}
                            onDetail={showDetail}
                            onOpen={open}
                            onAddToBasket={(file) => queueEntry(file.id)}
                            adding={adding}
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
                              search || risk || analysisStatus
                                ? () => {
                                    setSearch("");
                                    setRisk("");
                                    setAnalysisStatus("");
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
                    addingCount={adding.size}
                    scanId={scan?.id ?? ""}
                    onRemove={removeFromBasket}
                    onFindFiles={() => setPage("suggestions")}
                    onClear={clearBasket}
                    onBusyChange={setCleaning}
                    onDone={(result) => {
                      setRecentResult(result);
                      removeRecycled(result);
                      setDetail(null);
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
              analysisActive={boot.analysisProgress.active}
              revision={revision}
              selected={selected.has(detail.id)}
              queued={queued.has(detail.id)}
              onClose={() => setDetail(null)}
              onSelect={() => select(detail)}
              onAddToBasket={() => queueEntry(detail.id)}
              addingToBasket={adding.has(detail.id)}
              onChanged={changed}
              onError={fail}
            />
          )}
        </div>
        {["map", "apps", "suggestions"].includes(page) && (
          <SelectionBar
            items={[...pending.values()]}
            message={selectionMessage}
            onAdd={addToBasket}
          />
        )}
      </div>
    </div>
  );
}
