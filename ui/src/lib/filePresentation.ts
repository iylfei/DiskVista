import type { FileRecord } from "./types";

export function fileKind(file: FileRecord): string {
  if (file.isDir) return "文件夹";
  const extension = file.name.split(".").pop()?.toLowerCase() ?? "";
  if (["zip", "rar", "7z", "tar", "gz"].includes(extension)) return "压缩包";
  if (["mp4", "mkv", "avi", "mov", "webm"].includes(extension))
    return "视频文件";
  if (["jpg", "jpeg", "png", "gif", "webp", "heic"].includes(extension))
    return "图片";
  if (["mp3", "wav", "flac", "m4a", "ogg"].includes(extension))
    return "音频文件";
  if (
    ["pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt"].includes(
      extension,
    )
  )
    return "文档";
  if (extension === "exe") return "程序文件";
  if (extension === "msi") return "安装包";
  if (extension === "iso") return "磁盘镜像";
  if (extension === "dll") return "程序组件";
  return "文件";
}

export function fileSource(file: FileRecord): string {
  const a = file.assessment;
  if (a.owner) return `${a.confidence === "low" ? "可能关联：" : ""}${a.owner}`;
  if (a.category === "application_container") return "应用文件夹";
  if (a.category === "system") return "Windows 系统";
  return fileKind(file);
}
