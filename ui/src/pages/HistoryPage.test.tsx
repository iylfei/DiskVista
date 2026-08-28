import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { HistoryPageView } from "./HistoryPage";
import {
  initialHistoryState,
  type HistoryPagingState,
} from "../lib/historyPaging";
import type { HistoryItem } from "../lib/types";

const item: HistoryItem = {
  id: "history",
  batchId: "batch",
  path: "D:\\历史\\示例.bin",
  bytes: 128,
  time: 1,
  status: "recycled",
  message: "已回收",
  freeSpaceDelta: 0,
};

function render(state: HistoryPagingState, recent: HistoryItem[] = []) {
  return renderToStaticMarkup(
    <HistoryPageView
      state={state}
      recent={recent}
      onPage={() => {}}
      onRetry={() => {}}
      onOpenRecycleBin={() => {}}
    />,
  );
}

describe("history pagination view", () => {
  it("distinguishes initial loading from empty history and preserves the recycle-bin entry", () => {
    const html = render(initialHistoryState);
    expect(html).toContain("正在读取操作历史");
    expect(html).not.toContain("尚未执行清理操作");
    expect(html).toContain("打开 Windows 回收站");
    expect(html).toContain("每页 20 条");
    expect(html).not.toContain("500 条");
    expect(html.match(/disabled=""/g)).toHaveLength(2);
  });

  it("renders only the current page while keeping total and current cleanup results visible", () => {
    const html = render(
      {
        page: 30,
        total: 607,
        status: "ready",
        error: "",
        items: Array.from({ length: 7 }, (_, index) => ({
          ...item,
          id: `history-${index}`,
        })),
      },
      [{ ...item }, { ...item, id: "failed", status: "failed" }],
    );
    expect(html.match(/class="history-row"/g)).toHaveLength(7);
    expect(html).toContain("共 607 条");
    expect(html).toContain("31 / 31");
    expect(html).toContain("本次处理结果");
    expect(html).toContain("已移入回收站 1 项");
    expect(html).toContain("未成功 1 项");
    expect(html).toMatch(/<button disabled="">下一页<\/button>/);
    expect(html).toContain("<button>上一页</button>");
  });

  it("shows retryable load errors without presenting old rows as the failed page", () => {
    const html = render({
      ...initialHistoryState,
      status: "error",
      error: "数据库忙",
      items: [item],
    });
    expect(html).toContain('role="alert"');
    expect(html).toContain("数据库忙");
    expect(html).toContain("重试");
    expect(html).not.toContain("history-row");
    expect(html).not.toContain("尚未执行清理操作");
  });

  it("shows the empty state only after a successful empty page", () => {
    const html = render({ ...initialHistoryState, status: "ready", total: 0 });
    expect(html).toContain("尚未执行清理操作");
    expect(html).toContain("共 0 条");
    expect(html).not.toContain("正在读取");
    expect(html.match(/disabled=""/g)).toHaveLength(2);
  });
});
