import { useEffect, useRef, useState } from "react";
import { ArrowLeft, FolderSearch, Search } from "lucide-react";
import type { FileRecord, Scan, SuggestionPage } from "../lib/types";
import { api } from "../lib/api";
import { suggestionEmpty } from "../lib/emptyState";
import EntryTable from "../components/EntryTable";
import SuggestionGroups from "../components/SuggestionGroups";
import "./suggestions.css";

export default function SuggestionsPage({
  scan,
  revision,
  selected,
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
  selected: Set<number>;
  onSelect: (file: FileRecord) => void;
  onSelectMany: (files: FileRecord[]) => void;
  onDetail: (file: FileRecord) => void;
  onOpen: (file: FileRecord) => void;
  onMap: () => void;
  onRescan: () => void;
  onError: (error: unknown) => void;
}) {
  const [data, setData] = useState<SuggestionPage | null>(null);
  const [group, setGroup] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [risk, setRisk] = useState("");
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
    sort,
    offset: page * 100,
    limit: 100,
  };
  const key = JSON.stringify(query);
  const requestKey = `${key}:${revision}`;
  const current = useRef(requestKey);
  current.current = requestKey;
  const active = ["queued", "scanning", "aggregating"].includes(scan.status);
  useEffect(() => {
    setGroup(null);
    setSearch("");
    setRisk("");
    setPage(0);
  }, [scan.id]);
  useEffect(() => {
    if (scan.status !== "complete") return;
    let live = true;
    setBusy(true);
    setData(null);
    setLoadError(false);
    const timer = setTimeout(() => {
      api<SuggestionPage>("cleanup_suggestions", { query: JSON.parse(key) })
        .then((value) => {
          if (live) setData(value);
        })
        .catch((error) => {
          if (live) {
            setLoadError(true);
            onError(error);
          }
        })
        .finally(() => {
          if (live) setBusy(false);
        });
    }, 180);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [key, scan.status, revision, retry, onError]);
  function clear() {
    setSearch("");
    setRisk("");
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
  const empty = suggestionEmpty(scan.status, risk, search);
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
        先查看已识别的临时文件和缓存；下载、照片和用途不明的文件需要你确认。不会自动选择文件。
      </p>
      <div className="list-toolbar">
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
      {group && (
        <div className="suggestion-section-heading">
          <button
            onClick={() => {
              setGroup(null);
              setPage(0);
            }}
          >
            <ArrowLeft size={15} />
            返回分类
          </button>
          <strong>{selectedGroup?.name ?? "文件列表"}</strong>
          <select
            aria-label="建议排序"
            value={sort}
            onChange={(event) => setSort(event.target.value)}
          >
            <option value="size">按占用空间</option>
            <option value="name">按路径</option>
            <option value="activity">按最近变化</option>
          </select>
        </div>
      )}
      {selectedGroup && (
        <div className="suggestion-details">
          <p>{selectedGroup.purpose}</p>
          <p>清理影响：{selectedGroup.consequence}</p>
          {selectedGroup.recognized && risk !== "protected" && (
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
                {selecting ? "正在加入…" : "选择这一类"}
              </button>
            </div>
          )}
        </div>
      )}
      {busy ? (
        <div className="empty compact" role="status">
          正在整理清理建议…
        </div>
      ) : loadError ? (
        <div className="empty compact">
          <h2>无法读取清理建议</h2>
          <p>请重试，扫描记录不会被删除。</p>
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
          items={data.items}
          total={data.total}
          page={page}
          onPage={setPage}
          selected={selected}
          onSelect={onSelect}
          onDetail={onDetail}
          onOpen={onOpen}
          quietReview
        />
      ) : (
        <SuggestionGroups
          groups={data.groups}
          onOpen={(id) => {
            setGroup(id);
            setPage(0);
          }}
        />
      )}
    </section>
  );
}
