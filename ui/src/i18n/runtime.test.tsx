import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";
import { setLanguage } from "./locale";

afterEach(() => setLanguage("zh-CN"));

describe("localized JSX runtime", () => {
  it("translates intrinsic text and accessible properties", () => {
    setLanguage("en");
    expect(
      renderToStaticMarkup(<button aria-label="关闭窗口">总览</button>),
    ).toBe('<button aria-label="Close window">Overview</button>');
  });
});
