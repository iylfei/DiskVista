import { useEffect, useRef, useState } from "react";
import { List } from "lucide-react";
import type { FileRecord } from "../lib/types";
import { api, bytes } from "../lib/api";
import { treemap } from "../lib/treemap";
import HelpTip from "./HelpTip";
import { helpText } from "../lib/helpText";

interface Snapshot {
  parent: FileRecord;
  items: FileRecord[];
  total: number;
}

export default function SpaceMap({
  scanId,
  parent,
  revision,
  onOpen,
  onShowFiles,
}: {
  scanId: string;
  parent: string;
  revision: string;
  onOpen: (f: FileRecord) => void;
  onShowFiles: () => void;
}) {
  const [data, setData] = useState<Snapshot | null>(null);
  const [error, setError] = useState("");
  const [retry, setRetry] = useState(0);
  const [hover, setHover] = useState<FileRecord | null>(null);
  const [size, setSize] = useState({ width: 800, height: 400 });
  const container = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!container.current) return;
    const observer = new ResizeObserver(([entry]) => {
      if (entry.contentRect.width && entry.contentRect.height)
        setSize({
          width: entry.contentRect.width,
          height: entry.contentRect.height,
        });
    });
    observer.observe(container.current);
    return () => observer.disconnect();
  }, []);
  useEffect(() => {
    let live = true;
    setData(null);
    setHover(null);
    setError("");
    api<Snapshot>("space_map", { scanId, parent })
      .then((result) => {
        if (live) setData(result);
      })
      .catch(() => {
        if (live) setError("无法读取这个目录的空间分布，请重试。");
      });
    return () => {
      live = false;
    };
  }, [scanId, parent, revision, retry]);
  const visible = data?.items.filter((f) => f.logicalBytes > 0) ?? [];
  const totalBytes = data?.parent.logicalBytes ?? 0;
  const rest = Math.max(
    0,
    totalBytes - visible.reduce((sum, file) => sum + file.logicalBytes, 0),
  );
  const values = [...visible.map((f) => f.logicalBytes), rest];
  const tiles = treemap(values, size.width, size.height);
  return (
    <section className="space-map-section">
      <div className="map-caption">
        <span>
          方块越大，文件越大。点击文件夹继续查看。
          <HelpTip label="地图中的文件大小" text={helpText.logicalSize} />
        </span>
        <button onClick={onShowFiles}>
          <List size={15} />
          查看文件列表
        </button>
      </div>
      <div
        className="space-map"
        ref={container}
        aria-label="当前目录空间地图，按文件大小绘制"
        aria-busy={!data && !error}
      >
        {tiles.map((tile) => {
          const file = visible[tile.index];
          const label =
            file?.name ??
            `其余 ${Math.max(0, (data?.total ?? 0) - visible.length)} 项`;
          const amount = values[tile.index];
          return (
            <button
              key={file?.id ?? "rest"}
              className="map-tile"
              style={{
                left: `${(tile.x / size.width) * 100}%`,
                top: `${(tile.y / size.height) * 100}%`,
                width: `${(tile.width / size.width) * 100}%`,
                height: `${(tile.height / size.height) * 100}%`,
                background: [
                  "#e4ecf3",
                  "#e9edf0",
                  "#e5eeeb",
                  "#eeece5",
                  "#eae8ef",
                  "#e0e8ec",
                ][tile.index % 6],
              }}
              onClick={() => (file ? onOpen(file) : onShowFiles())}
              onMouseEnter={() => setHover(file ?? null)}
              onMouseLeave={() => setHover(null)}
              onFocus={() => setHover(file ?? null)}
              onBlur={() => setHover(null)}
              aria-label={`${file?.isDir ? "打开文件夹" : file ? "查看文件" : "查看文件列表"} ${label}，${bytes(amount)}`}
              title={`${file?.path ?? label} · ${bytes(amount)} · ${totalBytes ? ((amount / totalBytes) * 100).toFixed(1) : 0}%`}
            >
              {tile.width > 80 && tile.height > 48 && <strong>{label}</strong>}
              {tile.width > 65 && tile.height > 24 && (
                <span>{bytes(amount)}</span>
              )}
            </button>
          );
        })}
        {tiles.length === 0 && (
          <div className="map-empty" role={error ? "alert" : "status"}>
            <span>
              {error ||
                (data ? "这个目录没有已知的非零大小文件" : "正在汇总目录空间…")}
            </span>
            {error && (
              <button onClick={() => setRetry((value) => value + 1)}>
                重试
              </button>
            )}
          </div>
        )}
      </div>
      <div className="map-status">
        <span title={hover?.path ?? parent}>{hover?.name ?? "当前目录"}</span>
        <strong>{bytes(hover?.logicalBytes ?? totalBytes)}</strong>
        {hover && totalBytes > 0 && (
          <span>{((hover.logicalBytes / totalBytes) * 100).toFixed(1)}%</span>
        )}
        {!hover && data && <span>{data.total.toLocaleString()} 项</span>}
        {data && !data.parent.complete && <span>未扫描完整</span>}
      </div>
    </section>
  );
}
