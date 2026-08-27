import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import HelpTip from "./HelpTip";
import OverviewPage from "../pages/OverviewPage";
import BasketPage from "../pages/BasketPage";

describe("plain-language help", () => {
  it("provides a named keyboard button without a native-title-only tooltip", () => {
    const html = renderToStaticMarkup(
      <HelpTip label="逻辑大小" text="文件内容的大小" />,
    );
    expect(html).toContain('type="button"');
    expect(html).toContain('aria-label="关于逻辑大小"');
    expect(html).not.toContain("title=");
    expect(html).not.toContain('role="tooltip"');
  });
  it("escapes help labels", () => {
    const html = renderToStaticMarkup(
      <HelpTip label="<script>" text="Explanation" />,
    );
    expect(html).not.toContain("<script>");
    expect(html).toContain("&lt;script&gt;");
  });
  it("keeps the help button in a non-breaking superscript", () => {
    const html = renderToStaticMarkup(
      <span>
        逻辑大小
        <HelpTip label="逻辑大小" text="文件内容的大小" />
      </span>,
    );
    expect(html).toContain('逻辑大小<sup class="help-anchor">\u2060<button');
    expect(html).toContain('width="12" height="12"');
    expect(html).toContain("</button></sup>");
  });
  it("uses task-focused overview copy instead of a slogan", () => {
    const html = renderToStaticMarkup(
      <OverviewPage
        volumes={[]}
        locations={[]}
        disabled={false}
        scan={null}
        onScan={() => {}}
        onBrowse={() => {}}
        onMap={() => {}}
      />,
    );
    expect(html).toContain("找出占用空间，选择不再需要的文件");
    expect(html).not.toContain("每个文件，都值得");
    expect(html).toContain("清理前须知");
  });
  it("offers direct recycling without repeated confirmation copy", () => {
    const html = renderToStaticMarkup(
      <BasketPage
        items={[]}
        onFindFiles={() => {}}
        onClear={() => {}}
        scanId="s"
        onRemove={() => {}}
        onDone={() => {}}
        onError={() => {}}
      />,
    );
    expect(html).toContain("检查并移入回收站");
    expect(html).not.toContain("先预览，再确认");
    expect(html).not.toContain("需要你确认");
    expect(html).toContain("去查看清理建议");
  });
  it("offers the supplied scan locations and disables scanning controls while busy", () => {
    const html = renderToStaticMarkup(
      <OverviewPage
        volumes={[
          {
            path: "D:\\",
            label: "数据",
            freeBytes: 10,
            totalBytes: 100,
            fileSystem: "NTFS",
            removable: false,
            identity: "fixture-volume",
          },
        ]}
        locations={[
          {
            id: "downloads",
            name: "查看下载中的大文件",
            path: "D:\\迁移后的下载",
            description: "扫描下载文件夹",
          },
        ]}
        disabled
        scan={null}
        onScan={() => {}}
        onBrowse={() => {}}
        onMap={() => {}}
      />,
    );
    expect(html).toContain("扫描 D 盘");
    expect(html).toContain('title="D:\\迁移后的下载"');
    const buttons = html.match(/<button[^>]*>/g) ?? [];
    expect(buttons.length).toBe(4);
    expect(buttons.every((button) => button.includes('disabled=""'))).toBe(
      true,
    );
  });
});
