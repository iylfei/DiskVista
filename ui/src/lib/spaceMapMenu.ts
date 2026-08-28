import { fileCleanupBlockReason } from "./cleanupTarget";
import type { FileRecord } from "./types";

export interface SpaceMapMenuContext {
  scope: string;
  cacheable: boolean;
  queued: ReadonlySet<number>;
  adding: ReadonlySet<number>;
  disabledReason?: string | null;
  onAddToBasket: (file: FileRecord) => void;
  onShowFiles: () => void;
}

export interface SpaceMapMenuTarget {
  file: FileRecord | null;
  label: string;
  point: { x: number; y: number };
  trigger: Pick<HTMLElement, "isConnected" | "focus">;
  scope: string;
  sequence: number;
}

export function spaceMapAddBlockReason(
  file: FileRecord | null,
  context: SpaceMapMenuContext,
): string | null {
  if (!file) return "这是其余项目的合计，请在文件列表中选择具体项目。";
  if (context.queued.has(file.id)) return "已加入待清理清单";
  if (context.adding.has(file.id)) return "正在添加…";
  if (context.disabledReason) return context.disabledReason;
  if (!context.cacheable) return "扫描完成后才能加入待清理清单";
  return fileCleanupBlockReason(file);
}

export function isSpaceMapMenuKey(key: string, shiftKey: boolean): boolean {
  return key === "ContextMenu" || (key === "F10" && shiftKey);
}

export function spaceMapMenuPosition(
  point: { x: number; y: number },
  menu: { width: number; height: number },
  viewport: {
    width: number;
    height: number;
    offsetLeft?: number;
    offsetTop?: number;
  },
) {
  const left = (viewport.offsetLeft ?? 0) + 8;
  const top = (viewport.offsetTop ?? 0) + 8;
  return {
    left: Math.max(
      left,
      Math.min(point.x, left + viewport.width - menu.width - 16),
    ),
    top: Math.max(
      top,
      Math.min(point.y, top + viewport.height - menu.height - 16),
    ),
  };
}

export function listenForSpaceMapMenuEvents(
  element: HTMLElement,
  onClose: (restoreFocus: boolean) => void,
  host: Window = window,
) {
  const outside = (event: Event) => {
    if (!element.contains(event.target as Node)) onClose(false);
  };
  const close = () => onClose(true);
  const keydown = (event: KeyboardEvent) => {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopImmediatePropagation();
      close();
    } else if (event.key === "Tab") {
      close();
    } else if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
      event.preventDefault();
      const buttons = Array.from(
        element.querySelectorAll<HTMLButtonElement>(
          '[role="menuitem"]:not(:disabled)',
        ),
      );
      if (!buttons.length) return;
      const index = buttons.findIndex(
        (button) => button === host.document.activeElement,
      );
      const next =
        event.key === "Home"
          ? 0
          : event.key === "End"
            ? buttons.length - 1
            : event.key === "ArrowDown"
              ? (index + 1) % buttons.length
              : (index < 0 ? buttons.length - 1 : index - 1 + buttons.length) %
                buttons.length;
      buttons[next].focus({ preventScroll: true });
    }
  };
  const viewport = host.visualViewport;
  const capture = { capture: true };
  host.addEventListener("pointerdown", outside, capture);
  host.addEventListener("focusin", outside, capture);
  host.addEventListener("keydown", keydown, capture);
  host.addEventListener("scroll", close, capture);
  host.addEventListener("resize", close);
  viewport?.addEventListener("resize", close);
  viewport?.addEventListener("scroll", close);
  return () => {
    host.removeEventListener("pointerdown", outside, capture);
    host.removeEventListener("focusin", outside, capture);
    host.removeEventListener("keydown", keydown, capture);
    host.removeEventListener("scroll", close, capture);
    host.removeEventListener("resize", close);
    viewport?.removeEventListener("resize", close);
    viewport?.removeEventListener("scroll", close);
  };
}

export function createSpaceMapMenu(
  context: () => SpaceMapMenuContext,
  onChange: (target: SpaceMapMenuTarget | null) => void,
) {
  let current: SpaceMapMenuTarget | null = null;
  let sequence = 0;
  function matches(target: SpaceMapMenuTarget) {
    return current === target && target.scope === context().scope;
  }
  function close(target: SpaceMapMenuTarget, restoreFocus = false) {
    if (current !== target) return false;
    current = null;
    onChange(null);
    if (
      restoreFocus &&
      target.scope === context().scope &&
      target.trigger.isConnected
    )
      target.trigger.focus({ preventScroll: true });
    return true;
  }
  return {
    open(target: Omit<SpaceMapMenuTarget, "scope" | "sequence">) {
      current = { ...target, scope: context().scope, sequence: ++sequence };
      onChange(current);
      return current;
    },
    close,
    syncScope() {
      if (current && current.scope !== context().scope) close(current);
    },
    invalidate() {
      current = null;
    },
    add(target: SpaceMapMenuTarget) {
      const latest = context();
      if (!matches(target) || spaceMapAddBlockReason(target.file, latest))
        return false;
      close(target, true);
      latest.onAddToBasket(target.file!);
      return true;
    },
    showFiles(target: SpaceMapMenuTarget) {
      if (!matches(target)) return false;
      const latest = context();
      close(target, true);
      latest.onShowFiles();
      return true;
    },
  };
}
