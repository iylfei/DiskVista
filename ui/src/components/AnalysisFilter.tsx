import "./file-filters.css";

export default function AnalysisFilter({
  value,
  onChange,
}: {
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <select
      aria-label="AI 分析筛选"
      title="AI 已分析仅包含当前有效结果；失败和过期结果归为未分析。"
      value={value}
      onChange={(event) => onChange(event.target.value)}
    >
      <option value="">全部 AI 状态</option>
      <option value="analyzed">AI 已分析</option>
      <option value="unanalyzed">AI 未分析</option>
    </select>
  );
}
