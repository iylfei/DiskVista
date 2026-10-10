const storageKey = "diskvista:feature-tour:v1";

export const featureTourSteps = [
  {
    target: "scan-folder",
    title: "扫描文件夹",
    description:
      "从这里选择要扫描的文件夹。扫描后，你就能查看空间占用和清理建议，也可以在总览中选择磁盘或常用位置。",
    placement: "bottom",
  },
  {
    target: "map",
    title: "空间地图",
    description:
      "用方块大小直观看出哪些文件和文件夹最占空间。点击文件夹可逐层查看，找到需要整理的内容。",
    placement: "right",
  },
  {
    target: "apps",
    title: "应用空间",
    description:
      "按应用查看空间占用，了解安装文件、缓存和数据分别占了多少空间，再决定如何整理。",
    placement: "right",
  },
  {
    target: "suggestions",
    title: "清理建议",
    description:
      "集中查看缓存、临时文件和大文件等候选项。先看用途与风险，再把确认不需要的内容加入待清理清单。",
    placement: "right",
  },
  {
    target: "basket",
    title: "待清理清单",
    description:
      "把准备清理的内容放在这里统一核对。确认后，DiskVista 会先检查保护状态和文件占用，再将可回收的项目移入回收站。",
    placement: "right",
  },
  {
    target: "ai-analysis",
    title: "AI 分析",
    description:
      "遇到用途不明的大文件，可以让 AI 提供用途说明和清理建议。先在设置中配置并启用 AI，再扫描并分析；结果仅供参考，清理由你确认。",
    placement: "bottom",
  },
] as const;

export function shouldShowFeatureTour(): boolean {
  try {
    return localStorage.getItem(storageKey) !== "seen";
  } catch {
    return true;
  }
}

export function rememberFeatureTour(): void {
  try {
    localStorage.setItem(storageKey, "seen");
  } catch {
    // Storage may be unavailable; the introduction must still be dismissible.
  }
}
