import { useRef, useState } from "react";
import Modal from "./Modal";
import { Sparkles, RotateCw } from "lucide-react";
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
  const [confirmScan, setConfirmScan] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);
  const startingRef = useRef(false);
  const status = analysisStatus(settings, active || starting, scan);
  async function analyze(fresh = false) {
    if (disabled || status.disabled || startingRef.current) return;
    if (status.action === "settings") {
      onSettings();
      return;
    }
    if (!scan) return;
    startingRef.current = true;
    setStarting(true);
    try {
      onStarted(
        await api<Progress>("analyze_scan", { scanId: scan.id, fresh }),
      );
      setConfirmScan(null);
    } catch (error) {
      onError(error);
    } finally {
      startingRef.current = false;
      setStarting(false);
    }
  }
  return (
    <>
      <button
        className={`ai-indicator${status.attention ? " needs-attention" : ""}`}
        title={status.description}
        aria-label={
          status.action === "settings"
            ? `${status.label}，打开 AI 设置`
            : status.label
        }
        disabled={disabled || status.disabled}
        onClick={() => analyze()}
      >
        <Sparkles size={14} />
        {starting ? "AI 准备分析…" : status.label}
      </button>
      {scan?.status === "complete" && status.action === "analyze" && (
        <button
          disabled={disabled || status.disabled || starting}
          title="按当前配置重新请求 AI，不复用上次结果"
          onClick={() => setConfirmScan(scan.id)}
        >
          <RotateCw size={14} />
          重新分析
        </button>
      )}
      {confirmScan && confirmScan === scan?.id && (
        <Modal
          title="重新分析当前扫描"
          compact
          closeDisabled={starting}
          onClose={() => setConfirmScan(null)}
        >
          <div className="modal-body">
            <p>
              将按当前配置重新分析符合条件的文件，不复用旧结果。旧结果保留为过期记录，本轮重新计算语言模型的请求预算，可能产生新的
              API 费用。
            </p>
          </div>
          <footer>
            <button disabled={starting} onClick={() => setConfirmScan(null)}>
              取消
            </button>
            <button
              className="primary"
              disabled={disabled || status.disabled || starting}
              onClick={() => analyze(true)}
            >
              开始重新分析
            </button>
          </footer>
        </Modal>
      )}
    </>
  );
}
