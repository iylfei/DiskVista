import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { readFileSync } from "node:fs";
import { TitleBarView } from "./TitleBar";

const render = (maximized = false, available = true) =>
  renderToStaticMarkup(
    <TitleBarView
      maximized={maximized}
      available={available}
      onAction={() => {}}
    />,
  );

describe("custom title bar", () => {
  it("has accessible controls separate from the drag region", () => {
    const html = render();
    expect(html).toContain("DiskVista");
    expect(html).toContain('aria-label="最小化窗口"');
    expect(html).toContain('aria-label="最大化窗口"');
    expect(html).toContain('aria-label="关闭窗口"');
    expect(html.match(/data-tauri-drag-region/g)).toHaveLength(1);
    expect(html).not.toMatch(/<button[^>]*data-tauri-drag-region/);
  });

  it("reflects the actual maximized state and disables non-desktop controls", () => {
    expect(render(true)).toContain('aria-label="还原窗口"');
    expect(render(true)).not.toContain('aria-label="最大化窗口"');
    expect(render(false, false).match(/disabled=""/g)).toHaveLength(3);
  });

  it("ships without native decorations and grants only needed window actions", () => {
    const config = JSON.parse(
      readFileSync(
        new URL("../../../apps/desktop/tauri.conf.json", import.meta.url),
        "utf8",
      ),
    );
    const capability = JSON.parse(
      readFileSync(
        new URL(
          "../../../apps/desktop/capabilities/main.json",
          import.meta.url,
        ),
        "utf8",
      ),
    );
    expect(config.app.windows[0].decorations).toBe(false);
    expect(capability.windows).toEqual(["main"]);
    expect(
      capability.permissions.filter((p: string) =>
        p.startsWith("core:window:"),
      ),
    ).toEqual([
      "core:window:allow-close",
      "core:window:allow-minimize",
      "core:window:allow-toggle-maximize",
      "core:window:allow-start-dragging",
    ]);
  });
});
