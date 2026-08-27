import { useEffect, useState } from "react";
import type { EntryPage } from "../lib/types";
import { api } from "../lib/api";
import Modal from "./Modal";

export default function ScanIssues({
  scanId,
  onClose,
}: {
  scanId: string;
  onClose: () => void;
}) {
  const [data, setData] = useState<EntryPage | null>(null);
  const [page, setPage] = useState(0);
  const [error, setError] = useState("");
  useEffect(() => {
    let live = true;
    setData(null);
    setError("");
    api<EntryPage>("query_entries", {
      query: {
        scanId,
        parent: null,
        search: null,
        risk: null,
        suggestions: false,
        minimumBytes: 0,
        offset: page * 50,
        limit: 50,
        sort: "name",
        issuesOnly: true,
      },
    })
      .then((result) => {
        if (live) setData(result);
      })
      .catch(() => {
        if (live) setError("无法读取具体原因，请稍后重试。");
      });
    return () => {
      live = false;
    };
  }, [scanId, page]);
  return (
    <Modal title="未能扫描的位置与原因" onClose={onClose} wide>
      <div className="modal-body">
        <p>这些位置的占用可能没有统计完整。不会为了读取它们而修改文件权限。</p>
        {error ? (
          <p role="alert">{error}</p>
        ) : !data ? (
          <p role="status">正在读取…</p>
        ) : data.items.length ? (
          data.items.map((file) => (
            <div className="preview-item" key={file.id}>
              <strong className="path-text">{file.path}</strong>
              <p>{file.issue}</p>
            </div>
          ))
        ) : (
          <p>这份扫描记录没有保留具体原因。可以重新扫描后查看。</p>
        )}
      </div>
      <footer>
        <button
          disabled={!data || page === 0}
          onClick={() => setPage((value) => value - 1)}
        >
          上一页
        </button>
        <span>
          {page + 1} / {Math.max(1, Math.ceil((data?.total ?? 0) / 50))}
        </span>
        <button
          disabled={!data || (page + 1) * 50 >= data.total}
          onClick={() => setPage((value) => value + 1)}
        >
          下一页
        </button>
        <button onClick={onClose}>关闭</button>
      </footer>
    </Modal>
  );
}
