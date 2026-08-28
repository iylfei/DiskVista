import type { ApplicationUnit, FileRecord, UnitComponent } from "./types";

export function fileCleanupBlockReason(file: FileRecord): string | null {
  if (file.assessment.risk === "protected")
    return file.assessment.protectedReason || "此项目受保护，不能回收";
  if (!file.complete) return "此项目未扫描完整，请重新扫描后再操作";
  if (file.hasBlockedChildren) return "目录内包含受保护或未完整扫描的内容";
  return null;
}

export function componentCleanupBlockReason(
  component: UnitComponent,
): string | null {
  if (component.role === "unclassified")
    return "此处是未归属内容的统计，请查看文件后选择具体项目";
  if (component.protected) return "此项目受保护或未扫描完整，请查看详情";
  return null;
}

export function unitCleanupBlockReason(unit: ApplicationUnit): string | null {
  if (unit.kind === "unassigned")
    return "此处是未归属内容的统计，请查看文件后选择具体项目";
  if (unit.components.length > 1) return "包含多个文件位置，请展开后分别添加";
  if (!unit.components.length) return "没有可操作的文件位置";
  return componentCleanupBlockReason(unit.components[0]);
}
