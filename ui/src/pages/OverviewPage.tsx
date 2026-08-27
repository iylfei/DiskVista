import {
  HardDrive,
  FolderSearch,
  ArrowRight,
  ShieldCheck,
  Sparkles,
} from "lucide-react";
import { useEffect, useState } from "react";
import type { Scan, Volume, ScanLocation } from "../lib/types";
import { bytes, date, statusText } from "../lib/api";
import HelpTip from "../components/HelpTip";
import { helpText } from "../lib/helpText";
import ScanIssues from "../components/ScanIssues";
export default function OverviewPage({
  volumes,
  scan,
  onScan,
  onBrowse,
  onMap,
  locations,
  disabled,
}: {
  volumes: Volume[];
  scan: Scan | null;
  onScan: (path: string) => void;
  onBrowse: () => void;
  onMap: () => void;
  locations: ScanLocation[];
  disabled: boolean;
}) {
  const [volume, setVolume] = useState(volumes[0]?.path ?? "");
  const [showIssues, setShowIssues] = useState(false);
  useEffect(() => {
    if (!volumes.some((item) => item.path === volume))
      setVolume(volumes[0]?.path ?? "");
  }, [volumes, volume]);
  return (
    <>
      <section className="welcome">
        <div>
          <h2>找出占用空间，选择不再需要的文件</h2>
          <p>选择磁盘或文件夹，查看空间占用。</p>
          <div className="scan-start-actions">
            <select
              aria-label="选择要扫描的磁盘"
              value={volume}
              disabled={disabled || !volumes.length}
              onChange={(event) => setVolume(event.target.value)}
            >
              {volumes.map((item) => (
                <option key={item.path} value={item.path}>
                  {item.label || "本地磁盘"} {item.path}
                </option>
              ))}
            </select>
            <button
              className="primary"
              disabled={disabled || !volume}
              onClick={() => onScan(volume)}
            >
              <HardDrive size={17} />
              {volume ? `扫描 ${volume.slice(0, 1)} 盘` : "扫描所选磁盘"}
            </button>
            <button onClick={onBrowse} disabled={disabled}>
              <FolderSearch size={17} />
              选择文件夹
            </button>
          </div>
        </div>
        <div className="welcome-symbol">
          <HardDrive size={52} strokeWidth={1} />
        </div>
      </section>
      {locations.length > 0 && (
        <div className="scan-shortcuts">
          {locations.map((location) => (
            <button
              key={location.id}
              onClick={() => onScan(location.path)}
              disabled={disabled}
              title={location.path}
            >
              <FolderSearch size={20} />
              <span>
                <strong>{location.name}</strong>
                <small>{location.description}</small>
              </span>
              <ArrowRight size={16} />
            </button>
          ))}
        </div>
      )}
      <div className="volume-grid">
        {volumes.map((v) => (
          <button
            className="volume-card"
            key={v.path}
            onClick={() => onScan(v.path)}
            disabled={disabled}
            aria-label={`扫描 ${v.path}`}
          >
            <div>
              <HardDrive size={20} />
              <strong>
                {v.label || "本地磁盘"} <span>{v.path}</span>
              </strong>
              <ArrowRight size={17} />
            </div>
            <div className="capacity-track">
              <span
                style={{
                  width: `${Math.max(0, Math.min(100, (1 - v.freeBytes / v.totalBytes) * 100))}%`,
                }}
              />
            </div>
            <footer>
              <span>可用 {bytes(v.freeBytes)}</span>
              <span>共 {bytes(v.totalBytes)}</span>
            </footer>
            <small>
              {v.fileSystem} · {v.removable ? "外接存储" : "固定磁盘"}
            </small>
          </button>
        ))}
      </div>
      <div className="overview-columns">
        <section className="panel">
          <div className="section-heading">
            <h2>最近一次扫描</h2>
            {scan && (
              <button onClick={onMap}>
                查看结果 <ArrowRight size={14} />
              </button>
            )}
          </div>
          {scan ? (
            <>
              <p className="path-text">{scan.root}</p>
              <div className="mini-stats">
                <div>
                  <strong>{bytes(scan.logicalBytes)}</strong>
                  <span>
                    文件总大小
                    <HelpTip label="文件总大小" text={helpText.logicalSize} />
                  </span>
                </div>
                <div>
                  <strong>{scan.files.toLocaleString()}</strong>
                  <span>文件</span>
                </div>
                <div>
                  <strong>{scan.issues}</strong>
                  <span>
                    处未能扫描
                    <HelpTip
                      label="未能扫描的原因"
                      text={helpText.incomplete}
                    />
                    {scan.issues > 0 && (
                      <button
                        className="text-button"
                        onClick={() => setShowIssues(true)}
                      >
                        查看原因
                      </button>
                    )}
                  </span>
                </div>
              </div>
              <p className="muted">
                {statusText(scan.status)} · {date(scan.started)} · {scan.mode}
              </p>
              {scan.status !== "complete" && <p>{scan.message}</p>}
            </>
          ) : (
            <div className="empty compact">
              <FolderSearch size={30} />
              <p>还没有扫描记录</p>
              <small>点击上方的磁盘，或选择一个文件夹开始扫描。</small>
            </div>
          )}
        </section>
        <section className="panel principle-panel">
          <h2>清理前须知</h2>
          <div>
            <ShieldCheck size={20} />
            <p>
              <strong>重要内容会受到保护</strong>
              <span>系统文件、驱动和已安装程序不能直接清理。</span>
            </p>
          </div>
          <div>
            <Sparkles size={20} />
            <p>
              <strong>不使用 AI 也能正常清理</strong>
              <span>默认不开启 AI，不联网也可以扫描与回收。</span>
            </p>
          </div>
          <div>
            <HardDrive size={20} />
            <p>
              <strong>清理需要你确认</strong>
              <span>文件只移入回收站。关闭程序后，不会继续在后台扫描。</span>
            </p>
          </div>
        </section>
      </div>
      {showIssues && scan && (
        <ScanIssues scanId={scan.id} onClose={() => setShowIssues(false)} />
      )}
    </>
  );
}
