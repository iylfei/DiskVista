import type { Scan, Settings } from "./types";

export function analysisStatus(
  llm: Settings["llm"],
  active = false,
  scan: Pick<Scan, "status"> | null = null,
) {
  if (active)
    return {
      label: "AI 分析中",
      description:
        "完成后可按“AI 已分析”筛选，每个文件查看删除建议与简短理由。",
      attention: false,
      action: "analyze" as const,
      disabled: true,
    };
  if (!llm.enabled)
    return {
      label: "AI 未启用",
      description: "在设置中启用 AI，辅助解释用途不明的文件。",
      attention: false,
      action: "settings" as const,
      disabled: false,
    };
  if (!llm.baseUrl.trim() || !llm.model.trim())
    return {
      label: "AI 待配置",
      description: "请在设置中填写服务地址和模型。",
      attention: true,
      action: "settings" as const,
      disabled: false,
    };
  return {
    label: "AI 分析当前扫描",
    description: !scan
      ? "请先扫描一个位置。"
      : scan.status !== "complete"
        ? "请先完成扫描，再分析扫描结果。"
        : "按目录分批分析来源未明确的大文件，文件须严格大于门槛（最低 100 MiB）；每项提供删除建议与简短理由。",
    attention: false,
    action: "analyze" as const,
    disabled: scan?.status !== "complete",
  };
}
