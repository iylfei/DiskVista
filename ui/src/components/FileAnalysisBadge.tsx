import { Sparkles } from "lucide-react";
import { analysisLabel } from "../lib/analysisSummaries";
import type { AnalysisSummary } from "../lib/types";
import "./analysis.css";

export default function FileAnalysisBadge({
  result,
  onOpen,
}: {
  result?: AnalysisSummary;
  onOpen: () => void;
}) {
  if (!result) return null;
  const stale = result.status === "stale";
  const label = analysisLabel(result);
  return (
    <button
      className={`file-analysis-badge${stale ? " stale" : ""}`}
      onClick={onOpen}
      aria-label={`查看 AI 分析：${label}`}
      title={
        stale
          ? "分析已过期，打开详情查看原因"
          : `${result.summary || label}；仅供参考，点击查看完整分析`
      }
    >
      <Sparkles size={11} />
      <span>{label}</span>
    </button>
  );
}
