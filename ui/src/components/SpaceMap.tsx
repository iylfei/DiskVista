import { useEffect, useMemo, useRef, useState } from "react";
import { List } from "lucide-react";
import type { FileRecord } from "../lib/types";
import { bytes } from "../lib/api";
import { treemap } from "../lib/treemap";
import { useSpaceMap } from "../lib/useSpaceMap";
import { useSpaceMapMenu } from "./SpaceMapMenu";
import HelpTip from "./HelpTip";
import { helpText } from "../lib/helpText";
import { localeName } from "../i18n/locale";

export default function SpaceMap({
  scanId,
  parent,
  revision,
  onOpen,
  onShowFiles,
  onAddToBasket,
  queued,
  adding,
  disabledReason,
  cacheable,
}: {
  scanId: string;
  parent: string;
  revision: string;
  onOpen: (f: FileRecord) => void;
  onShowFiles: () => void;
  onAddToBasket: (file: FileRecord) => void;
  queued: Set<number>;
  adding: Set<number>;
  disabledReason?: string | null;
  cacheable: boolean;
}) {
  const { data, error, retry } = useSpaceMap({
    scanId,
    parent,
    revision,
    cacheable,
  });
  const scope = JSON.stringify([scanId, parent, revision, cacheable]);
  const menu = useSpaceMapMenu({
    scope,
    cacheable,
    queued,
    adding,
    disabledReason,
    onAddToBasket,
    onShowFiles,
  });
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
    setHover(null);
  }, [scope]);
  const visible = useMemo(
    () => data?.items.filter((f) => f.logicalBytes > 0) ?? [],
    [data],
  );
  const totalBytes = data?.parent.logicalBytes ?? 0;
  const values = useMemo(() => {
    const rest = Math.max(
      0,
      totalBytes - visible.reduce((sum, file) => sum + file.logicalBytes, 0),
    );
    return [...visible.map((f) => f.logicalBytes), rest];
  }, [visible, totalBytes]);
  const tiles = useMemo(
    () => treemap(values, size.width, size.height),
    [values, size],
  );
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
              onContextMenu={(event) =>
                menu.onContextMenu(event, file ?? null, label)
              }
              onKeyDown={(event) => menu.onKeyDown(event, file ?? null, label)}
              onMouseEnter={() => setHover(file ?? null)}
              onMouseLeave={() => setHover(null)}
              onFocus={() => setHover(file ?? null)}
              onBlur={() => setHover(null)}
              aria-haspopup="menu"
              aria-expanded={
                !!menu.target && menu.target.file === (file ?? null)
              }
              aria-controls={
                menu.target?.file === (file ?? null) ? menu.id : undefined
              }
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
              <button
                onClick={() => {
                  menu.close();
                  retry();
                }}
              >
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
        {!hover && data && (
          <span>{data.total.toLocaleString(localeName())} 项</span>
        )}
        {data && !data.parent.complete && <span>未扫描完整</span>}
      </div>
      {menu.element}
    </section>
  );
}
