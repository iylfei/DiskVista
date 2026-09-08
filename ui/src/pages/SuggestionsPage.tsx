import { useEffect, useRef, useState } from "react";
import { FolderSearch, Search } from "lucide-react";
import type { FileRecord, Scan, SuggestionPage } from "../lib/types";
import { api } from "../lib/api";
import { useSearchReady } from "../lib/useSearchReady";
import { useQueryResult } from "../lib/useQueryResult";
import { suggestionEmpty } from "../lib/emptyState";
import EntryTable from "../components/EntryTable";
import AnalysisFilter from "../components/AnalysisFilter";
import SuggestionGroups, {
  SuggestionListHeader,
} from "../components/SuggestionGroups";
import "./suggestions.css";

export default function SuggestionsPage({
  scan,
  revision,
  analysisRevision,
  selected,
  queued,
  onSelect,
  onSelectMany,
  onDetail,
  onOpen,
  onMap,
  onRescan,
  onError,
}: {
  scan: Scan;
  revision: number;
  analysisRevision: string;
  selected: Set<number>;
  queued: Set<number>;
  onSelect: (file: FileRecord) => void;
  onSelectMany: (files: FileRecord[]) => void;
  onDetail: (file: FileRecord) => void;
  onOpen: (file: FileRecord) => void;
  onMap: () => void;
  onRescan: () => void;
  onError: (error: unknown) => void;
}) {
  const [group, setGroup] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const searchReady = useSearchReady(search);
  const [risk, setRisk] = useState("");
  const [analysisStatus, setAnalysisStatus] = useState("");
  const [sort, setSort] = useState("size");
  const [page, setPage] = useState(0);
  const [busy, setBusy] = useState(false);
  const [selecting, setSelecting] = useState(false);
  const [loadError, setLoadError] = useState(false);
  const [retry, setRetry] = useState(0);
  const mounted = useRef(false);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  const query = {
    scanId: scan.id,
    group,
    search,
    risk,
    analysisStatus,
    sort,
    offset: page * 100,
    limit: 100,
  };
  const key = JSON.stringify(query);
  const [data, setData] = useQueryResult<SuggestionPage>(key);
  const filteredAnalysisRevision = analysisStatus ? analysisRevision : "";
  const requestKey = `${key}:${revision}:${filteredAnalysisRevision}`;
  const current = useRef(requestKey);
  current.current = requestKey;
  const active = ["queued", "scanning", "aggregating"].includes(scan.status);
  useEffect(() => {
    setGroup(null);
    setSearch("");
    setRisk("");
    setAnalysisStatus("");
    setPage(0);
  }, [scan.id]);
  useEffect(() => {
    if (scan.status !== "complete") return;
    let live = true;
    setBusy(true);
    setLoadError(false);
    if (!searchReady) return;
    api<SuggestionPage>("cleanup_suggestions", { query: JSON.parse(key) })
      .then((value) => {
        if (live) {
          setPage((previous) =>
            Math.min(previous, Math.max(0, Math.ceil(value.total / 100) - 1)),
          );
          setData(value);
        }
      })
      .catch((error) => {
        if (live) {
          setData(null);
          setLoadError(true);
          onError(error);
        }
      })
      .finally(() => {
        if (live) setBusy(false);
      });
    return () => {
      live = false;
    };
  }, [
    key,
    searchReady,
    scan.status,
    revision,
    filteredAnalysisRevision,
    retry,
    setData,
    onError,
  ]);
  function clear() {
    setSearch("");
    setRisk("");
    setAnalysisStatus("");
    setGroup(null);
    setPage(0);
  }
  async function selectGroup() {
    setSelecting(true);
    try {
      const files = await api<FileRecord[]>("select_suggestion_group", {
        query,
      });
      if (mounted.current && current.current === requestKey)
        onSelectMany(files);
    } catch (error) {
      if (mounted.current && current.current === requestKey) onError(error);
    } finally {
      if (mounted.current) setSelecting(false);
    }
  }
  const empty = suggestionEmpty(scan.status, risk, search, analysisStatus);
  const selectedGroup = data?.groups.find((item) => item.id === group);
  const filesShown = group !== null || search.length > 0;
  if (scan.status !== "complete")
    return (
      <div className="empty compact">
        <FolderSearch size={32} />
        <h2>{empty.title}</h2>
        <p>{empty.description}</p>
        {!active && (
          <div className="actions">
            <button className="primary" onClick={onRescan}>
              重新扫描
            </button>
            <button onClick={onMap}>查看空间地图</button>
          </div>
        )}
      </div>
    );
  return (
    <section className="suggestions-content">
      <p className="suggestions-intro">
        按类别查找文件，勾选后添加到待清理清单。
        {busy && data && !filesShown && (
          <span role="status">正在整理清理建议…</span>
        )}
      </p>
      <div className="list-toolbar file-filters">
        <div className="search-box">
          <Search size={16} />
          <input
            aria-label="搜索清理建议"
            placeholder="搜索文件名或路径"
            value={search}
            onChange={(event) => {
              setSearch(event.target.value);
              setPage(0);
            }}
          />
        </div>
        <select
          aria-label="清理建议筛选"
          value={risk}
          onChange={(event) => {
            setRisk(event.target.value);
            setPage(0);
          }}
        >
          <option value="">全部建议（不含受保护）</option>
          <option value="low">较低风险</option>
          <option value="known">已识别用途</option>
          <option value="unknown">未识别用途</option>
          <option value="protected">仅看受保护项</option>
        </select>
        <AnalysisFilter
          value={analysisStatus}
          onChange={(value) => {
            setAnalysisStatus(value);
            setPage(0);
          }}
        />
        <button onClick={onRescan}>重新扫描</button>
      </div>
      {risk === "protected" && (
        <div className="notice protected-suggestions">
          <span>
            这些内容不能由本程序直接清理。系统空间和已安装应用可以通过 Windows
            管理。
          </span>
          <button
            onClick={() =>
              api("open_system", { target: "storage" }).catch(onError)
            }
          >
            Windows 存储设置
          </button>
          <button
            onClick={() =>
              api("open_system", { target: "apps" }).catch(onError)
            }
          >
            卸载应用
          </button>
        </div>
      )}
      {filesShown && (
        <SuggestionListHeader
          group={selectedGroup}
          sort={sort}
          onBack={
            group
              ? () => {
                  setGroup(null);
                  setPage(0);
                }
              : undefined
          }
          onSort={(value) => {
            setSort(value);
            setPage(0);
          }}
        />
      )}
      {selectedGroup?.recognized && risk !== "protected" && (
        <div className="group-selection">
          <span>请先关闭相关应用。每次最多选择 500 项。</span>
          <button
            disabled={selecting || busy || !data?.items.length}
            onClick={() => onSelectMany(data?.items ?? [])}
          >
            选择本页文件
          </button>
          <button
            disabled={selecting || busy || !data?.total || data.total > 500}
            onClick={() => void selectGroup()}
            title={
              data && data.total > 500
                ? "每次最多选择 500 项，请缩小搜索范围或按页选择"
                : undefined
            }
          >
            {selecting ? "正在选择…" : "选择这一类"}
          </button>
        </div>
      )}
      {busy && !data ? (
        <div className="empty compact" role="status">
          正在整理清理建议…
        </div>
      ) : loadError ? (
        <div className="empty compact">
          <h2>无法读取清理建议</h2>
          <p>请稍后重试。</p>
          <button onClick={() => setRetry((value) => value + 1)}>重试</button>
        </div>
      ) : !data?.total ? (
        <div className="empty compact">
          <FolderSearch size={30} />
          <h2>{empty.title}</h2>
          <p>{empty.description}</p>
          <button onClick={empty.reset ? clear : onMap}>
            {empty.reset ? "清除筛选" : "查看空间地图"}
          </button>
        </div>
      ) : filesShown ? (
        <EntryTable
          scanId={scan.id}
          analysisRevision={analysisRevision}
          items={data.items}
          total={data.total}
          page={page}
          onPage={setPage}
          selected={selected}
          queued={queued}
          onSelect={onSelect}
          onDetail={onDetail}
          onOpen={onOpen}
          quietReview
          busy={busy || !searchReady}
        />
      ) : (
        <div inert={busy || !searchReady} aria-busy={busy}>
          <SuggestionGroups
            groups={data.groups}
            onOpen={(id) => {
              setGroup(id);
              setPage(0);
            }}
          />
        </div>
      )}
    </section>
  );
}
