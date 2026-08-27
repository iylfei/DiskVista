import { useRef, useState } from "react";
import { Sparkles } from "lucide-react";
import { api } from "../lib/api";
import { analysisStatus } from "../lib/analysisStatus";
import type { LlmSettings, Progress, Scan } from "../lib/types";

interface Props {
  settings: LlmSettings;
  scan: Scan | null;
  active: boolean;
  disabled: boolean;
  onSettings: () => void;
  onStarted: (progress: Progress) => void;
  onError: (error: unknown) => void;
}

export default function ScanAnalysisButton({
  settings,
  scan,
  active,
  disabled,
  onSettings,
  onStarted,
  onError,
}: Props) {
  const [starting, setStarting] = useState(false);
  const startingRef = useRef(false);
  const status = analysisStatus(settings, active || starting, scan);
  async function analyze() {
    if (disabled || status.disabled || startingRef.current) return;
    if (status.action === "settings") {
      onSettings();
      return;
    }
    if (!scan) return;
    startingRef.current = true;
    setStarting(true);
    try {
      onStarted(await api<Progress>("analyze_scan", { scanId: scan.id }));
    } catch (error) {
      onError(error);
    } finally {
      startingRef.current = false;
      setStarting(false);
    }
  }
  return (
    <button
      className={`ai-indicator${status.attention ? " needs-attention" : ""}`}
      title={status.description}
      aria-label={
        status.action === "settings"
          ? `${status.label}，打开 AI 设置`
          : status.label
      }
      disabled={disabled || status.disabled}
      onClick={analyze}
    >
      <Sparkles size={14} />
      {starting ? "AI 准备分析…" : status.label}
    </button>
  );
}
