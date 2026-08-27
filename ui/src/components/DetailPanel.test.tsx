import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import DetailPanel from "./DetailPanel";
import type { FileRecord } from "../lib/types";
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
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>加入待清理/);
    expect(html).toContain("Protected");
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
    expect(render(false)).toContain("加入待清理");
    expect(render(true)).toContain('aria-pressed="true"');
    expect(render(true)).toContain("取消选择");
    expect(render(true)).not.toContain("加入待清理");
  });
});
