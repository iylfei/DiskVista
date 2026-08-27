import type { Scan, Settings } from "./types";

export function analysisStatus(
  llm: Settings["llm"],
  active = false,
  scan: Pick<Scan, "status"> | null = null,
) {
  if (active)
    return {
      label: "AI 分析中",
      description: "结果会显示在对应文件详情的“AI 辅助解释”中。",
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
        : "发送当前扫描中符合门槛的未识别项目的基本信息进行分析，结果在文件详情中查看。",
    attention: false,
    action: "analyze" as const,
    disabled: scan?.status !== "complete",
  };
}
