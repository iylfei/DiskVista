import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { UnitSummary } from "./ApplicationUnits";
import { ComponentSummary } from "./ApplicationUnitRow";
import AddToBasketButton from "./AddToBasketButton";
import { applicationRows, expandedSearchResults } from "../lib/applicationTree";
import {
  componentCleanupBlockReason,
  unitCleanupBlockReason,
} from "../lib/cleanupTarget";
import type { ApplicationUnit, UnitComponent } from "../lib/types";
const unit: ApplicationUnit = {
  id: "one",
  name: "Game A",
  kind: "application",
  confidence: "high",
  logicalBytes: 1024,
  occupiedBytes: 1024,
  fileCount: 100,
  estimated: false,
  complete: true,
  components: [],
  children: [],
};
describe("application-level results", () => {
  it("shows one application rather than selectable internal files", () => {
    const html = renderToStaticMarkup(
      <UnitSummary unit={unit} onClick={() => {}} />,
    );
    expect(html).toContain("Game A");
    expect(html).toContain("已识别应用");
    expect(html).toContain("100");
    expect(html).not.toContain('type="checkbox"');
  });
  it("does not present executable-layout clues as confirmed applications", () => {
    const html = renderToStaticMarkup(
      <UnitSummary
        unit={{
          ...unit,
          name: "<script>test</script>",
          kind: "possible_application",
          confidence: "low",
          complete: false,
          estimated: true,
        }}
        onClick={() => {}}
      />,
    );
    expect(html).not.toContain("疑似独立应用");
    expect(html).not.toContain("低置信度");
    expect(html).not.toContain("已识别应用");
    expect(html).toContain("未扫描完整");
    expect(html).toContain("估算");
    expect(html).not.toContain("<script>");
  });
});

describe("application hierarchy", () => {
  const runtime = {
    ...unit,
    id: "runtime",
    name: "runtime",
    kind: "possible_application",
  };
  const parent = { ...unit, name: "GPT-SoVITS", children: [runtime] };
  it("keeps children hidden until expanded and restores the collapsed list", () => {
    expect(applicationRows([parent], new Set()).map((r) => r.key)).toEqual([
      "one",
    ]);
    const open = applicationRows([parent], new Set(["one"]));
    expect(open.map((r) => [r.key, r.depth])).toEqual([
      ["one", 0],
      ["runtime", 1],
    ]);
    expect(applicationRows([parent], new Set()).length).toBe(1);
  });
  it("exposes expand state and includes child size in the parent label", () => {
    const html = renderToStaticMarkup(
      <UnitSummary
        unit={{ ...parent, complete: false, estimated: true }}
        onClick={() => {}}
      />,
    );
    expect(html).toContain('aria-expanded="false"');
    expect(html).toContain("展开 1 项");
    expect(html).toContain("含子项");
    expect(html).toContain('title="已识别应用 · 估算 · 未扫描完整 · 含子项"');
    expect(html).not.toContain("处组成");
    expect(expandedSearchResults([parent]).has("one")).toBe(true);
  });
});

describe("application basket targets", () => {
  const component: UnitComponent = {
    entryId: 17,
    path: "D:\\App\\Cache",
    role: "cache",
    evidence: "缓存目录",
    logicalBytes: 100,
    occupiedBytes: 100,
    fileCount: 5,
    protected: false,
  };
  const single = { ...unit, components: [component] };

  it("only queues an individual location instead of an aggregate", () => {
    expect(unitCleanupBlockReason(single)).toBeNull();
    expect(unitCleanupBlockReason(unit)).toContain("没有可操作");
    expect(unitCleanupBlockReason({ ...single, kind: "unassigned" })).toContain(
      "统计",
    );
    expect(
      unitCleanupBlockReason({
        ...single,
        components: [component, { ...component, entryId: 18 }],
      }),
    ).toContain("展开后分别添加");
  });

  it("blocks residual and protected locations without hiding their details", () => {
    for (const target of [
      { ...component, protected: true },
      { ...component, role: "unclassified" },
    ]) {
      expect(componentCleanupBlockReason(target)).toBeTruthy();
      const html = renderToStaticMarkup(
        <ComponentSummary
          component={target}
          onOpen={() => {}}
          onDetail={() => {}}
          onAddToBasket={() => {}}
        />,
      );
      expect(html).toMatch(/aria-label="加入待清理清单 [^"]+"[^>]*disabled=""/);
      expect(html).toContain("查看详情");
      expect(html).toContain("查看文件");
    }
  });

  it("renders an accessible basket action without nesting it inside navigation", () => {
    const html = renderToStaticMarkup(
      <ComponentSummary
        component={component}
        onOpen={() => {}}
        onDetail={() => {}}
        onAddToBasket={() => {}}
      />,
    );
    expect(html).toContain('aria-label="加入待清理清单 D:\\App\\Cache"');
    expect(html).not.toContain("移入回收站");
    expect(html).not.toContain('disabled=""');
    expect(html).not.toMatch(/<button[^>]*>(?:(?!<\/button>)[\s\S])*<button/);
    const loading = renderToStaticMarkup(
      <AddToBasketButton name="<script>" disabled onClick={() => {}} />,
    );
    expect(loading).toContain('disabled=""');
    expect(loading).toContain("&lt;script&gt;");
    expect(loading).not.toContain("<script>");
  });

  it("prevents duplicate additions while loading or already in the basket", () => {
    for (const state of [{ queued: true }, { adding: true }]) {
      const html = renderToStaticMarkup(
        <AddToBasketButton name="Cache" {...state} onClick={() => {}} />,
      );
      expect(html).toContain('disabled=""');
      expect(html).toContain(
        "queued" in state ? "已加入待清理清单" : "正在添加…",
      );
    }
  });
});
