import { useEffect, useState } from "react";
import { ChevronRight, ChevronDown, Folder } from "lucide-react";
import { api } from "../lib/api";
import type { EntryPage } from "../lib/types";
interface Props {
  scanId: string;
  path: string;
  selected: string;
  onOpen: (path: string) => void;
  depth?: number;
}
function Branch({ scanId, path, selected, onOpen, depth = 0 }: Props) {
  const [expanded, setExpanded] = useState(depth === 0);
  const [data, setData] = useState<EntryPage | null>(null);
  const [page, setPage] = useState(0);
  useEffect(() => {
    if (!expanded) return;
    let live = true;
    api<EntryPage>("query_entries", {
      query: {
        scanId,
        parent: path,
        search: null,
        risk: null,
        owner: null,
        suggestions: false,
        minimumBytes: 0,
        offset: page * 50,
        limit: 50,
        sort: "name",
        directoriesOnly: true,
      },
    })
      .then((r) => {
        if (live) setData(r);
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [expanded, scanId, path, page]);
  return (
    <div>
      <div
        className={`tree-node ${path === selected ? "selected" : ""}`}
        style={{ paddingLeft: depth * 13 }}
      >
        <button
          aria-label={expanded ? "折叠目录" : "展开目录"}
          onClick={() => setExpanded(!expanded)}
        >
          {expanded ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
        </button>
        <button title={path} onClick={() => onOpen(path)}>
          <Folder size={14} />
          <span>{depth === 0 ? "扫描起点" : path.split("\\").pop()}</span>
        </button>
      </div>
      {expanded && (
        <>
          {data?.items.map((f) => (
            <Branch
              key={f.id}
              scanId={scanId}
              path={f.path}
              selected={selected}
              onOpen={onOpen}
              depth={depth + 1}
            />
          ))}
          {data && data.total > 50 && (
            <div className="tree-paging">
              <button disabled={page === 0} onClick={() => setPage(page - 1)}>
                上一页
              </button>
              <span>{page + 1}</span>
              <button
                disabled={(page + 1) * 50 >= data.total}
                onClick={() => setPage(page + 1)}
              >
                下一页
              </button>
            </div>
          )}
        </>
      )}
    </div>
  );
}
export default function DirectoryTree(props: Props) {
  return (
    <div className="directory-tree" aria-label="目录树">
      <Branch {...props} />
    </div>
  );
}
