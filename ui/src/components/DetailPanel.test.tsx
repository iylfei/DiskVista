import { describe, it, expect, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import DetailPanel from "./DetailPanel";
import AnalysisResultCard from "./AnalysisResultCard";
import FileAnalysisBadge from "./FileAnalysisBadge";
import EntryTable from "./EntryTable";
import type { AnalysisResult, AnalysisSummary, FileRecord } from "../lib/types";
vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: ({ count }: { count: number }) => ({
    getTotalSize: () => count * 65,
    getVirtualItems: () =>
      Array.from({ length: count }, (_, index) => ({
        index,
        size: 65,
        start: index * 65,
      })),
  }),
}));
vi.mock("../lib/useAnalysisSummaries", () => ({
  useAnalysisSummaries: () => ({ summaries: new Map(), failed: false }),
}));
function file(risk = "review"): FileRecord {
  return {
    id: 1,
    path: "D:\\fixture\\<script>alert(1)</script>.txt",
    name: "<script>alert(1)</script>.txt",
    parent: "D:\\fixture",
    isDir: false,
    logicalBytes: 32,
    allocatedBytes: 32,
    modified: 0,
    latestChange: 0,
    accessed: 0,
    created: 0,
    identity: "1:1",
    attributes: 0,
    links: 1,
    fileCount: 1,
    issue: null,
    complete: true,
    hasBlockedChildren: false,
    assessment: {
      category: "unknown",
      owner: null,
      confidence: "low",
      risk,
      purpose: "Unknown",
      consequence: "Review required",
      recovery: "Recycle Bin",
      recommendation: "Review",
      ruleId: null,
      protectedReason: risk === "protected" ? "Protected" : null,
      evidence: [],
    },
  };
}
const noop = () => {};

const summary: AnalysisSummary = {
  analysisId: "analysis-one",
  entryId: 1,
  created: 1,
  status: "success",
  summary: "用途相似，请确认当前是否仍需保留",
  historyMatchCount: 1,
};

describe("AI evidence and list summaries", () => {
  it("hides routine review badges in file lists without hiding protection or change dates", () => {
    const html = renderToStaticMarkup(
      <EntryTable
        scanId="s"
        items={[
          file(),
          { ...file("protected"), id: 2 },
          { ...file("low"), id: 3 },
        ]}
        selected={new Set()}
        onSelect={noop}
        onDetail={noop}
        onOpen={noop}
        total={3}
        page={0}
        onPage={noop}
      />,
    );
    expect(html).not.toContain("需要你确认");
    expect(html).not.toContain('class="badge review"');
    expect(html).toContain("受保护");
    expect(html).toContain("较低风险");
    expect(html).toContain("最近变化");
  });
  it("only labels a valid history match as similar", () => {
    const render = (result?: AnalysisSummary) =>
      renderToStaticMarkup(<FileAnalysisBadge result={result} onOpen={noop} />);
    expect(render()).toBe("");
    expect(render(summary)).toContain("与删除历史相似");
    expect(render({ ...summary, historyMatchCount: 0 })).not.toContain(
      "与删除历史相似",
    );
    const stale = render({ ...summary, status: "stale" });
    expect(stale).toContain("AI 已过期");
    expect(stale).not.toContain("与删除历史相似");
  });

  it("shows stale reasons, readable evidence and the referenced history safely", () => {
    const result: AnalysisResult = {
      formatVersion: 3,
      id: "analysis-one",
      entryId: 1,
      created: 1,
      status: "stale",
      message: "授权范围已变化",
      promptTokens: null,
      completionTokens: null,
      includedContent: false,
      assessment: {
        deletionAdvice: "review",
        reason: "可能是缓存；确认不再需要后再考虑删除。",
        evidence: [],
        historyMatches: [
          { historyId: "history:old", reason: "同一应用的缓存目录" },
        ],
      },
      evidenceDetails: [
        { id: "local:0", source: "安装记录", detail: "关联到示例应用" },
      ],
      historyReferences: [
        {
          id: "history:old",
          path: "<script>cache</script>",
          bytes: 32,
          recycledAt: 1,
          owner: "示例应用",
          category: "cache",
          matchBasis: ["同一应用"],
        },
      ],
    };
    const html = renderToStaticMarkup(<AnalysisResultCard result={result} />);
    expect(html).toContain("授权范围已变化");
    expect(html).toContain("关联到示例应用");
    expect(html).not.toContain("local:0");
    expect(html).toContain("同一应用的缓存目录");
    expect(html).not.toContain("<script>");
    expect(html).toContain("&lt;script&gt;");
    expect(html).toContain("不代表相似文件可以安全删除");
    expect(html).not.toContain("<details open");
    expect(html).not.toContain("待你确认：");
    expect(html).not.toContain("模型自评置信度");
    const batch = renderToStaticMarkup(
      <AnalysisResultCard
        result={{
          ...result,
          status: "success",
          requestId: "batch-1",
          requestItemCount: 12,
          promptTokens: 1300,
          completionTokens: 2500,
        }}
      />,
    );
    expect(batch).toContain("本批 12 项合计");
    expect(batch).toContain("输入 1300 / 输出 2500 tokens");
  });
});
describe("evidence panel safety", () => {
  it("escapes untrusted filenames", () => {
    const html = renderToStaticMarkup(
      <DetailPanel
        file={file()}
        scanId="s"
        llmEnabled={false}
        onClose={noop}
        onSelect={noop}
        onChanged={noop}
        onError={noop}
      />,
    );
    expect(html).not.toContain("<script>");
    expect(html).toContain("&lt;script&gt;");
    expect(html).toContain("在设置中启用 AI");
  });
  it("never permits selecting a protected target", () => {
    const html = renderToStaticMarkup(
      <DetailPanel
        file={file("protected")}
        scanId="s"
        llmEnabled
        onClose={noop}
        onSelect={noop}
        onChanged={noop}
        onError={noop}
      />,
    );
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>选择此项/);
    expect(html).toContain("Protected");
    expect(html).toContain("解除文件受保护状态");
  });
  it("only offers the protection override for protected targets", () => {
    const render = (risk: string) =>
      renderToStaticMarkup(
        <DetailPanel
          file={file(risk)}
          scanId="s"
          llmEnabled={false}
          onClose={noop}
          onSelect={noop}
          onChanged={noop}
          onError={noop}
        />,
      );
    expect(render("review")).not.toContain("解除文件受保护状态");
    expect(render("protected")).toContain("解除文件受保护状态");
  });
  it("queues files and directories while retaining protection", () => {
    for (const target of [
      file(),
      { ...file(), isDir: true },
      file("protected"),
      { ...file(), complete: false },
    ]) {
      const html = renderToStaticMarkup(
        <DetailPanel
          file={target}
          scanId="s"
          llmEnabled={false}
          onClose={noop}
          onSelect={noop}
          onAddToBasket={noop}
          onChanged={noop}
          onError={noop}
        />,
      );
      const action = [...html.matchAll(/<button\b[^>]*>[\s\S]*?<\/button>/g)]
        .map(([button]) => button)
        .find((button) => button.includes("加入待清理清单"));
      expect(action).toBeDefined();
      expect(action!.includes('disabled=""')).toBe(
        target.assessment.risk === "protected" || !target.complete,
      );
      expect(html).toContain("选择此项");
      expect(html).not.toContain("移入回收站");
    }
  });
  it("disables repeated queue actions while adding or already queued", () => {
    for (const state of [{ queued: true }, { addingToBasket: true }]) {
      const html = renderToStaticMarkup(
        <DetailPanel
          file={file()}
          scanId="s"
          llmEnabled={false}
          {...state}
          onClose={noop}
          onSelect={noop}
          onAddToBasket={noop}
          onChanged={noop}
          onError={noop}
        />,
      );
      const footer = html.match(/<footer>[\s\S]*?<\/footer>/)![0];
      const buttons = [
        ...footer.matchAll(/<button\b[^>]*>[\s\S]*?<\/button>/g),
      ];
      expect(buttons.length).toBe("queued" in state ? 1 : 2);
      expect(buttons.every(([button]) => button.includes('disabled=""'))).toBe(
        true,
      );
      expect(footer).toContain(
        "queued" in state ? "已加入待清理清单" : "正在添加…",
      );
    }
  });
});

describe("basket selection state", () => {
  it("shows removal only after an item has been selected", () => {
    const render = (selected: boolean) =>
      renderToStaticMarkup(
        <DetailPanel
          file={file()}
          scanId="s"
          llmEnabled={false}
          selected={selected}
          onClose={noop}
          onSelect={noop}
          onChanged={noop}
          onError={noop}
        />,
      );
    expect(render(false)).toContain('aria-pressed="false"');
    expect(render(false)).toContain("选择此项");
    expect(render(true)).toContain('aria-pressed="true"');
    expect(render(true)).toContain("取消选择");
    expect(render(true)).not.toContain("选择此项");
  });
});
