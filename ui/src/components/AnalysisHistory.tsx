import { bytes, date } from "../lib/api";
import type { HistoryReference, ModelAssessment } from "../lib/types";
import "./analysis.css";

export default function AnalysisHistory({
  references,
  matches = [],
}: {
  references: HistoryReference[];
  matches?: ModelAssessment["historyMatches"];
}) {
  if (references.length === 0) return null;
  return (
    <div className="analysis-history">
      <ul>
        {references.map((reference) => {
          const match = matches.find((item) => item.historyId === reference.id);
          return (
            <li key={reference.id}>
              <p className="path-text">{reference.path}</p>
              <small>
                {date(reference.recycledAt)} 已回收 · {bytes(reference.bytes)}
                {reference.owner ? ` · ${reference.owner}` : ""}
              </small>
              {match ? (
                <p>
                  <strong>相似之处：</strong>
                  {match.reason}
                </p>
              ) : (
                <p className="muted">
                  选取依据：{reference.matchBasis.join("；")}
                </p>
              )}
            </li>
          );
        })}
      </ul>
      <p className="muted">
        曾移入回收站不代表之后没有还原，也不代表相似文件可以安全删除。
      </p>
    </div>
  );
}
