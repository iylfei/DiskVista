import { date, statusText } from "../lib/api";
import { helpText } from "../lib/helpText";
import type { AnalysisResult } from "../lib/types";
import AnalysisHistory from "./AnalysisHistory";
import HelpTip from "./HelpTip";
import "./analysis.css";

export default function AnalysisResultCard({
  result,
}: {
  result: AnalysisResult;
}) {
  const assessment = result.assessment;
  const references = result.historyReferences ?? [];
  const details = result.evidenceDetails ?? [];
  return (
    <div className="analysis-card">
      <span
        className={`badge ${result.status === "stale" ? "review" : "neutral"}`}
      >
        {statusText(result.status)}
      </span>
      <small>
        {result.includedContent ? "扫描记录与授权文本" : "基于扫描记录"}
        {references.length > 0 ? ` · 参考 ${references.length} 条回收历史` : ""}
        {" · "}
        {date(result.created)}
        {" · "}
        {(result.requestItemCount ?? 1) > 1 &&
          `本批 ${result.requestItemCount} 项合计 · `}
        {result.promptTokens == null
          ? "用量未知"
          : `输入 ${result.promptTokens} / 输出 ${result.completionTokens ?? "未知"} tokens`}
        <HelpTip label="Token 用量" text={helpText.tokens} />
      </small>
      {result.status === "stale" && (
        <p className="warning-text">
          {result.message || "分析条件已变化，请重新分析。"}
        </p>
      )}
      {assessment ? (
        <>
          <h4 className="analysis-advice">
            {
              {
                consider_delete: "可考虑删除",
                keep: "建议保留",
                review: "需要核实",
              }[assessment.deletionAdvice]
            }
          </h4>
          <p className="analysis-reason">{assessment.reason}</p>
          {references.length > 0 && (
            <details>
              <summary>当时参考的回收历史</summary>
              <AnalysisHistory
                references={references}
                matches={assessment.historyMatches ?? []}
              />
            </details>
          )}
          <details>
            <summary>分析参考信息</summary>
            {details.length > 0 ? (
              details.map((item) => (
                <div className="evidence" key={item.id}>
                  <strong>{item.source}</strong>
                  <p>{item.detail}</p>
                </div>
              ))
            ) : (
              <p className="muted">这条分析未保存可读的依据详情。</p>
            )}
          </details>
        </>
      ) : (
        <p>{result.message}</p>
      )}
    </div>
  );
}
