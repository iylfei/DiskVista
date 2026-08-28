import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { SpaceMapMenuItems } from "./SpaceMapMenu";
import SpaceMap from "./SpaceMap";
import { useSpaceMap } from "../lib/useSpaceMap";
import type { FileRecord } from "../lib/types";

vi.mock("../lib/useSpaceMap", () => ({ useSpaceMap: vi.fn() }));

const file = {
  id: 7,
  name: "<img src=x onerror=alert(1)>",
  path: "D:\\Fixture\\<img src=x>",
  isDir: true,
  logicalBytes: 1024,
  complete: true,
  assessment: { risk: "review" },
} as FileRecord;

describe("space map menu content", () => {
  it("shows an explicit add action and escapes file labels", () => {
    const onAdd = vi.fn();
    const html = renderToStaticMarkup(
      <SpaceMapMenuItems
        target={{ file, label: file.name }}
        reason={null}
        reasonId="reason"
        adding={false}
        onAdd={onAdd}
        onShowFiles={vi.fn()}
      />,
    );
    expect(html).toContain('role="menuitem"');
    expect(html).toContain("加入待清理清单");
    expect(html).not.toContain('disabled=""');
    expect(html).not.toContain("查看文件列表");
    expect(html).not.toContain("<img");
    expect(html).toContain("&lt;img");
    expect(onAdd).not.toHaveBeenCalled();
  });

  it("explains disabled items and offers file-list navigation for aggregates", () => {
    const html = renderToStaticMarkup(
      <SpaceMapMenuItems
        target={{ file: null, label: "其余 4 项" }}
        reason="这是其余项目的合计，请在文件列表中选择具体项目。"
        reasonId="aggregate-reason"
        adding={false}
        onAdd={vi.fn()}
        onShowFiles={vi.fn()}
      />,
    );
    expect(html).toContain(
      'role="menuitem" disabled="" aria-describedby="aggregate-reason"',
    );
    expect(html).toContain('id="aggregate-reason"');
    expect(html).toContain("合计");
    expect(html).toContain("查看文件列表");
  });

  it("reports adding as busy and disabled", () => {
    const html = renderToStaticMarkup(
      <SpaceMapMenuItems
        target={{ file, label: file.name }}
        reason="正在添加…"
        reasonId="reason"
        adding
        onAdd={vi.fn()}
        onShowFiles={vi.fn()}
      />,
    );
    expect(html).toContain('disabled="" aria-busy="true"');
    expect(html).toContain("正在添加…");
  });

  it("uses the shared loader and makes every tile keyboard-menu discoverable without opening a menu", () => {
    const onAddToBasket = vi.fn();
    vi.mocked(useSpaceMap).mockReturnValue({
      data: {
        parent: { ...file, logicalBytes: 4096 },
        items: [file],
        total: 4,
      },
      error: "",
      retry: vi.fn(),
    });
    const html = renderToStaticMarkup(
      <SpaceMap
        scanId="scan-one"
        parent={"D:\\Fixture"}
        revision="revision-one"
        cacheable
        queued={new Set()}
        adding={new Set()}
        onAddToBasket={onAddToBasket}
        onOpen={vi.fn()}
        onShowFiles={vi.fn()}
      />,
    );
    expect(useSpaceMap).toHaveBeenCalledWith({
      scanId: "scan-one",
      parent: "D:\\Fixture",
      revision: "revision-one",
      cacheable: true,
    });
    expect(html.match(/aria-haspopup="menu"/g)).toHaveLength(2);
    expect(html).toContain("其余 3 项");
    expect(html).not.toContain('role="menu"');
    expect(onAddToBasket).not.toHaveBeenCalled();
  });
});
