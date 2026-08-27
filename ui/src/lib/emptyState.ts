export function suggestionEmpty(status: string, risk: string, search: string) {
  if (["queued", "scanning", "aggregating"].includes(status))
    return {
      title: "正在扫描，完成后会显示清理建议",
      description:
        "扫描只查看文件信息，不会删除文件。文件较多时需要等待一会儿。",
      reset: false,
    };
  if (status !== "complete")
    return {
      title: "这次扫描没有完成",
      description: "可以重新扫描，或在空间地图中查看已经扫描到的内容。",
      reset: false,
    };
  if (search)
    return {
      title: "没有找到匹配的文件",
      description: "试试其他文件名，或清除筛选。",
      reset: true,
    };
  if (risk === "low")
    return {
      title: "没有找到较低风险的项目",
      description: "这不代表磁盘没有可清理的文件，可以查看其他类别后自行确认。",
      reset: true,
    };
  if (risk)
    return {
      title: "没有符合当前筛选的项目",
      description: "可以清除筛选，查看其他清理建议。",
      reset: true,
    };
  return {
    title: "没有发现可供选择的清理建议",
    description: "可以在空间地图查看占用分布，或扫描下载、临时文件等其他位置。",
    reset: false,
  };
}
