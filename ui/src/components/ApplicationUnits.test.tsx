import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { UnitSummary } from "./ApplicationUnits";
import { applicationRows, expandedSearchResults } from "../lib/applicationTree";
import type { ApplicationUnit } from "../lib/types";
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
      <UnitSummary unit={parent} onClick={() => {}} />,
    );
    expect(html).toContain('aria-expanded="false"');
    expect(html).toContain("展开 1 项");
    expect(html).toContain("含子项");
    expect(html).not.toContain("处组成");
    expect(expandedSearchResults([parent]).has("one")).toBe(true);
  });
});
