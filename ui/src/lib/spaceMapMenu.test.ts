import { describe, expect, it, vi } from "vitest";
import {
  createSpaceMapMenu,
  isSpaceMapMenuKey,
  listenForSpaceMapMenuEvents,
  spaceMapAddBlockReason,
  spaceMapMenuPosition,
  type SpaceMapMenuContext,
} from "./spaceMapMenu";
import type { FileRecord } from "./types";

const file = {
  id: 7,
  name: "目标目录",
  path: "D:\\Fixture\\目标目录",
  isDir: true,
  complete: true,
  hasBlockedChildren: false,
  assessment: { risk: "review", protectedReason: null },
} as FileRecord;

function setup() {
  const context: SpaceMapMenuContext = {
    scope: JSON.stringify(["scan-one", "D:\\Fixture", "revision-one", true]),
    cacheable: true,
    queued: new Set(),
    adding: new Set(),
    onAddToBasket: vi.fn(),
    onShowFiles: vi.fn(),
  };
  const onChange = vi.fn();
  const trigger = { isConnected: true, focus: vi.fn() };
  const menu = createSpaceMapMenu(() => context, onChange);
  const open = (target: FileRecord | null = file) =>
    menu.open({
      file: target,
      label: target?.name ?? "其余 4 项",
      point: { x: 200, y: 180 },
      trigger,
    });
  return { context, menu, open, trigger, onChange };
}

describe("space map menu target lifecycle", () => {
  it.each([true, false])(
    "only adds the exact tile after a menu action (directory=%s)",
    (isDir) => {
      const { menu, open, context, trigger } = setup();
      const exactFile = { ...file, isDir };
      const target = open(exactFile);
      expect(context.onAddToBasket).not.toHaveBeenCalled();
      expect(context.onShowFiles).not.toHaveBeenCalled();
      expect(menu.add(target)).toBe(true);
      expect(context.onAddToBasket).toHaveBeenCalledWith(exactFile);
      expect(trigger.focus).toHaveBeenCalledWith({ preventScroll: true });
      expect(menu.add(target)).toBe(false);
      expect(context.onAddToBasket).toHaveBeenCalledTimes(1);
    },
  );

  it("does not let a dismissed or replaced menu act on the current target", () => {
    const { menu, open, context } = setup();
    const first = open();
    menu.close(first);
    expect(menu.add(first)).toBe(false);
    const secondFile = { ...file, id: 8, name: "另一个目录" };
    const second = open(secondFile);
    expect(menu.close(first, true)).toBe(false);
    expect(menu.add(first)).toBe(false);
    expect(menu.add(second)).toBe(true);
    expect(context.onAddToBasket).toHaveBeenCalledExactlyOnceWith(secondFile);
  });

  it.each([
    ["scan-two", "D:\\Fixture", "revision-one", true],
    ["scan-one", "D:\\Elsewhere", "revision-one", true],
    ["scan-one", "D:\\Fixture", "revision-two", true],
    ["scan-one", "D:\\Fixture", "revision-one", false],
  ])(
    "rejects old actions before effects run when scope becomes %s / %s / %s / %s",
    (...scope) => {
      const { menu, open, context, onChange, trigger } = setup();
      const old = open();
      context.scope = JSON.stringify(scope);
      expect(menu.add(old)).toBe(false);
      expect(menu.showFiles(old)).toBe(false);
      menu.syncScope();
      expect(onChange).toHaveBeenLastCalledWith(null);
      expect(trigger.focus).not.toHaveBeenCalled();
      const replacement = { ...file, path: "E:\\AnotherScan\\same-id" };
      expect(menu.add(open(replacement))).toBe(true);
      expect(context.onAddToBasket).toHaveBeenCalledExactlyOnceWith(
        replacement,
      );
    },
  );

  it("ignores late actions after unmount invalidation", () => {
    const { menu, open, context, onChange } = setup();
    const target = open();
    onChange.mockClear();
    menu.invalidate();
    expect(menu.add(target)).toBe(false);
    expect(menu.showFiles(target)).toBe(false);
    expect(onChange).not.toHaveBeenCalled();
    expect(context.onAddToBasket).not.toHaveBeenCalled();
  });

  it("does not move focus on outside dismissal or to a detached tile", () => {
    const { menu, open, trigger } = setup();
    menu.close(open());
    expect(trigger.focus).not.toHaveBeenCalled();
    const target = open();
    trigger.isConnected = false;
    menu.close(target, true);
    expect(trigger.focus).not.toHaveBeenCalled();
  });

  it("treats remaining items as an aggregate, with only file-list navigation", () => {
    const { menu, open, context } = setup();
    const target = open(null);
    expect(spaceMapAddBlockReason(null, context)).toContain("合计");
    expect(menu.add(target)).toBe(false);
    expect(menu.showFiles(target)).toBe(true);
    expect(menu.showFiles(target)).toBe(false);
    expect(context.onAddToBasket).not.toHaveBeenCalled();
    expect(context.onShowFiles).toHaveBeenCalledTimes(1);
  });

  it.each(["queued", "adding", "unfinished", "cleaning"])(
    "checks live %s state again on selection",
    (state) => {
      const { menu, open, context } = setup();
      const target = open();
      if (state === "queued") context.queued = new Set([file.id]);
      if (state === "adding") context.adding = new Set([file.id]);
      if (state === "unfinished") context.cacheable = false;
      if (state === "cleaning") context.disabledReason = "正在执行回收，请稍候";
      expect(spaceMapAddBlockReason(file, context)).toBeTruthy();
      expect(menu.add(target)).toBe(false);
      expect(context.onAddToBasket).not.toHaveBeenCalled();
    },
  );

  it.each([
    { ...file, complete: false },
    { ...file, hasBlockedChildren: true },
    {
      ...file,
      assessment: {
        ...file.assessment,
        risk: "protected",
        protectedReason: "系统文件",
      },
    },
  ])(
    "blocks unsafe targets through the existing cleanup policy",
    (blockedFile) => {
      const { menu, open, context } = setup();
      expect(spaceMapAddBlockReason(blockedFile, context)).toBeTruthy();
      expect(menu.add(open(blockedFile))).toBe(false);
      expect(context.onAddToBasket).not.toHaveBeenCalled();
    },
  );
});

describe("space map menu keyboard and viewport", () => {
  it("recognizes only the two menu keyboard shortcuts", () => {
    expect(isSpaceMapMenuKey("ContextMenu", false)).toBe(true);
    expect(isSpaceMapMenuKey("F10", true)).toBe(true);
    expect(isSpaceMapMenuKey("F10", false)).toBe(false);
    expect(isSpaceMapMenuKey("Enter", false)).toBe(false);
    expect(isSpaceMapMenuKey(" ", false)).toBe(false);
  });

  it("keeps a menu at the pointer when it fits", () => {
    expect(
      spaceMapMenuPosition(
        { x: 200, y: 180 },
        { width: 244, height: 90 },
        { width: 1036, height: 678 },
      ),
    ).toEqual({ left: 200, top: 180 });
  });

  it("clamps bottom-right and top-left placement inside the viewport", () => {
    expect(
      spaceMapMenuPosition(
        { x: 1030, y: 675 },
        { width: 244, height: 130 },
        { width: 1036, height: 678 },
      ),
    ).toEqual({ left: 784, top: 540 });
    expect(
      spaceMapMenuPosition(
        { x: -10, y: 0 },
        { width: 244, height: 130 },
        { width: 860, height: 678 },
      ),
    ).toEqual({ left: 8, top: 8 });
  });

  it("respects a zoomed visual viewport and keeps an oversized menu anchored", () => {
    expect(
      spaceMapMenuPosition(
        { x: 700, y: 650 },
        { width: 244, height: 130 },
        {
          width: 430,
          height: 339,
          offsetLeft: 200,
          offsetTop: 100,
        },
      ),
    ).toEqual({ left: 378, top: 301 });
    expect(
      spaceMapMenuPosition(
        { x: 50, y: 40 },
        { width: 244, height: 130 },
        { width: 200, height: 100 },
      ),
    ).toEqual({ left: 8, top: 8 });
  });
});

describe("space map menu event cleanup", () => {
  function events() {
    const host = Object.assign(new EventTarget(), {
      visualViewport: new EventTarget(),
      document: { activeElement: null },
    });
    const element = {
      contains: vi.fn(() => false),
      querySelectorAll: () => [],
    };
    const onClose = vi.fn();
    const release = listenForSpaceMapMenuEvents(
      element as unknown as HTMLElement,
      onClose,
      host as unknown as Window,
    );
    return { host, element, onClose, release };
  }

  it("closes on scroll, resize and visual viewport changes, and releases listeners", () => {
    const { host, onClose, release } = events();
    for (const source of [host, host.visualViewport]) {
      source.dispatchEvent(new Event("scroll"));
      source.dispatchEvent(new Event("resize"));
    }
    expect(onClose.mock.calls).toEqual([[true], [true], [true], [true]]);
    release();
    onClose.mockClear();
    for (const source of [host, host.visualViewport]) {
      source.dispatchEvent(new Event("scroll"));
      source.dispatchEvent(new Event("resize"));
    }
    host.dispatchEvent(new Event("pointerdown"));
    expect(onClose).not.toHaveBeenCalled();
  });

  it("closes for outside pointer/focus without stealing focus, not for an inside pointer", () => {
    const { host, element, onClose, release } = events();
    host.dispatchEvent(new Event("pointerdown"));
    host.dispatchEvent(new Event("focusin"));
    expect(onClose.mock.calls).toEqual([[false], [false]]);
    element.contains.mockReturnValue(true);
    host.dispatchEvent(new Event("pointerdown"));
    expect(onClose).toHaveBeenCalledTimes(2);
    release();
  });

  it("consumes Escape but lets Tab advance after restoring tile focus", () => {
    const { host, onClose, release } = events();
    const key = (value: string) =>
      Object.assign(new Event("keydown", { cancelable: true }), { key: value });
    const escape = key("Escape");
    host.dispatchEvent(escape);
    expect(escape.defaultPrevented).toBe(true);
    const tab = key("Tab");
    host.dispatchEvent(tab);
    expect(tab.defaultPrevented).toBe(false);
    expect(onClose.mock.calls).toEqual([[true], [true]]);
    release();
    host.dispatchEvent(key("Escape"));
    expect(onClose).toHaveBeenCalledTimes(2);
  });
});
