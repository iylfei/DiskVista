import { invoke } from "@tauri-apps/api/core";
export const api = <T>(command: string, args: Record<string, unknown> = {}) => {
  if (!("__TAURI_INTERNALS__" in window))
    return Promise.reject<T>(
      new Error("请使用 Windows 桌面程序，普通浏览器不能访问本地磁盘。"),
    );
  return invoke<T>(command, args);
};
export function bytes(value: number | null | undefined) {
  if (value == null) return "未知";
  if (value === 0) return "0 B";
  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  const i = Math.min(
    Math.floor(Math.log(Math.max(1, value)) / Math.log(1024)),
    4,
  );
  return `${(value / 1024 ** i).toLocaleString("zh-CN", { maximumFractionDigits: i > 1 ? 1 : 0 })} ${units[i]}`;
}
export const date = (value: number | null | undefined) =>
  value && value > 0
    ? new Date(value * 1000).toLocaleDateString("zh-CN")
    : "未知";
export const riskText = (risk: string) =>
  ({
    low: "较低风险",
    review: "需要你确认",
    protected: "受保护",
    keep: "建议保留",
    unknown: "未知",
  })[risk] ?? "未知";
export const statusText = (status: string) =>
  ({
    queued: "准备扫描",
    scanning: "扫描中",
    aggregating: "正在汇总",
    complete: "扫描完成",
    cancelled: "已取消",
    interrupted: "未完成",
    failed: "失败",
    recycled: "已移入回收站",
    skipped: "已跳过",
    success: "分析完成",
    stale: "已过期",
  })[status] ?? status;
