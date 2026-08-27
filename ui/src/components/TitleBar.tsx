import { useEffect, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { HardDrive, Minus, Square, Copy, X } from "lucide-react";
import { observeMaximized } from "../lib/windowState";
import "./titlebar.css";

type WindowAction = "minimize" | "toggleMaximize" | "close";

export function TitleBarView({
  maximized,
  available,
  onAction,
}: {
  maximized: boolean;
  available: boolean;
  onAction: (action: WindowAction) => void;
}) {
  const maximizeLabel = maximized ? "还原窗口" : "最大化窗口";
  return (
    <header className="window-titlebar" aria-label="应用标题栏">
      <div className="window-drag-region" data-tauri-drag-region>
        <HardDrive size={15} aria-hidden="true" />
        <span>DiskVista</span>
      </div>
      <div className="window-controls" role="group" aria-label="窗口控制">
        <button
          type="button"
          aria-label="最小化窗口"
          title="最小化窗口"
          disabled={!available}
          onClick={() => onAction("minimize")}
        >
          <Minus size={15} aria-hidden="true" />
        </button>
        <button
          type="button"
          aria-label={maximizeLabel}
          title={maximizeLabel}
          disabled={!available}
          onClick={() => onAction("toggleMaximize")}
        >
          {maximized ? (
            <Copy size={13} aria-hidden="true" />
          ) : (
            <Square size={12} aria-hidden="true" />
          )}
        </button>
        <button
          type="button"
          className="window-close"
          aria-label="关闭窗口"
          title="关闭窗口"
          disabled={!available}
          onClick={() => onAction("close")}
        >
          <X size={16} aria-hidden="true" />
        </button>
      </div>
    </header>
  );
}

export default function TitleBar({
  onError,
}: {
  onError: (error: unknown) => void;
}) {
  const [maximized, setMaximized] = useState(false);
  const available = isTauri();
  useEffect(() => {
    if (!available) return;
    return observeMaximized(getCurrentWindow(), setMaximized, onError);
  }, [available, onError]);
  const act = (action: WindowAction) => {
    if (available) void getCurrentWindow()[action]().catch(onError);
  };
  return (
    <TitleBarView maximized={maximized} available={available} onAction={act} />
  );
}
