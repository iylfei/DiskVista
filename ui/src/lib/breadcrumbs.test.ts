import { describe, expect, it } from "vitest";
import { breadcrumbs } from "./breadcrumbs";

describe("folder breadcrumbs", () => {
  it("makes each intermediate folder a navigation target", () => {
    expect(
      breadcrumbs("D:\\games", "D:\\games\\GPT-SoVITS\\runtime\\Lib"),
    ).toEqual([
      { label: "D:\\games", path: "D:\\games" },
      { label: "GPT-SoVITS", path: "D:\\games\\GPT-SoVITS" },
      { label: "runtime", path: "D:\\games\\GPT-SoVITS\\runtime" },
      { label: "Lib", path: "D:\\games\\GPT-SoVITS\\runtime\\Lib" },
    ]);
  });
  it("handles drive roots, trailing separators, and case without crossing the scan boundary", () => {
    expect(breadcrumbs("C:\\", "c:\\Users\\测试 文件")[2]?.path).toBe(
      "C:\\Users\\测试 文件",
    );
    expect(breadcrumbs("D:/games/", "d:/games/工具")).toHaveLength(2);
    expect(breadcrumbs("D:\\games", "D:\\games-copy\\file")).toHaveLength(1);
    expect(breadcrumbs("D:\\games", "D:\\")).toHaveLength(1);
  });
});
