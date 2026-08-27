import { useEffect, useState } from "react";
import { HardDrive, Square } from "lucide-react";
import type { Scan } from "../lib/types";
import { bytes } from "../lib/api";

export default function ScanProgress({
  scan,
  onCancel,
}: {
  scan: Scan;
  onCancel: () => void;
}) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, []);
  const seconds = Math.max(0, Math.floor(now / 1000 - scan.started));
  const phase =
    scan.status === "aggregating"
      ? "正在汇总占用和用途"
      : scan.status === "queued"
        ? "正在准备扫描"
        : "正在读取文件信息";
  return (
    <div className="scan-progress">
      <div className="scan-bar">
        <HardDrive size={16} />
        <strong>{phase}</strong>
        <span className="path-text">{scan.root}</span>
        <span>
          {scan.files.toLocaleString()} 个文件 · {bytes(scan.logicalBytes)}
        </span>
        <button onClick={onCancel}>
          <Square size={12} />
          取消扫描
        </button>
      </div>
      <p>
        已用 {Math.floor(seconds / 60)} 分 {seconds % 60} 秒 ·{" "}
        {scan.status === "aggregating"
          ? "正在生成清理建议。"
          : "扫描耗时取决于文件数量。"}
      </p>
    </div>
  );
}
