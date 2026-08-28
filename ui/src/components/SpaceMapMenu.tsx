import {
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
} from "react";
import { createPortal } from "react-dom";
import { List, ListPlus } from "lucide-react";
import {
  createSpaceMapMenu,
  isSpaceMapMenuKey,
  listenForSpaceMapMenuEvents,
  spaceMapAddBlockReason,
  spaceMapMenuPosition,
  type SpaceMapMenuContext,
  type SpaceMapMenuTarget,
} from "../lib/spaceMapMenu";
import type { FileRecord } from "../lib/types";
import "./space-map-menu.css";

export function useSpaceMapMenu(context: SpaceMapMenuContext) {
  const id = useId();
  const latest = useRef(context);
  latest.current = context;
  const [target, setTarget] = useState<SpaceMapMenuTarget | null>(null);
  const [menu] = useState(() =>
    createSpaceMapMenu(() => latest.current, setTarget),
  );
  useEffect(() => menu.syncScope(), [menu, context.scope]);
  useEffect(() => () => menu.invalidate(), [menu]);
  const active = target?.scope === context.scope ? target : null;
  function open(
    trigger: HTMLButtonElement,
    file: FileRecord | null,
    label: string,
    point?: { x: number; y: number },
  ) {
    const rect = trigger.getBoundingClientRect();
    menu.open({
      trigger,
      file,
      label,
      point: point ?? {
        x: rect.left + Math.min(12, rect.width / 2),
        y: rect.top + Math.min(28, rect.height),
      },
    });
  }
  return {
    id,
    target: active,
    close: () => active && menu.close(active),
    onContextMenu(
      event: ReactMouseEvent<HTMLButtonElement>,
      file: FileRecord | null,
      label: string,
    ) {
      event.preventDefault();
      event.stopPropagation();
      open(
        event.currentTarget,
        file,
        label,
        event.clientX || event.clientY
          ? { x: event.clientX, y: event.clientY }
          : undefined,
      );
    },
    onKeyDown(
      event: ReactKeyboardEvent<HTMLButtonElement>,
      file: FileRecord | null,
      label: string,
    ) {
      if (!isSpaceMapMenuKey(event.key, event.shiftKey)) return;
      event.preventDefault();
      event.stopPropagation();
      open(event.currentTarget, file, label);
    },
    element: active ? (
      <SpaceMapMenu
        key={active.sequence}
        id={id}
        target={active}
        reason={spaceMapAddBlockReason(active.file, context)}
        adding={!!active.file && context.adding.has(active.file.id)}
        menu={menu}
      />
    ) : null,
  };
}

export function SpaceMapMenuItems({
  target,
  reason,
  reasonId,
  adding,
  onAdd,
  onShowFiles,
}: {
  target: Pick<SpaceMapMenuTarget, "file" | "label">;
  reason: string | null;
  reasonId: string;
  adding: boolean;
  onAdd: () => void;
  onShowFiles: () => void;
}) {
  return (
    <>
      <div
        className="space-map-menu-target"
        title={target.file?.path ?? target.label}
      >
        {target.label}
      </div>
      <button
        type="button"
        role="menuitem"
        disabled={!!reason}
        aria-busy={adding || undefined}
        aria-describedby={reason ? reasonId : undefined}
        onClick={onAdd}
      >
        <ListPlus size={16} aria-hidden="true" />
        加入待清理清单
      </button>
      {reason && (
        <p id={reasonId} className="space-map-menu-reason">
          {reason}
        </p>
      )}
      {!target.file && (
        <button type="button" role="menuitem" onClick={onShowFiles}>
          <List size={16} aria-hidden="true" />
          查看文件列表
        </button>
      )}
    </>
  );
}

function SpaceMapMenu({
  id,
  target,
  reason,
  adding,
  menu,
}: {
  id: string;
  target: SpaceMapMenuTarget;
  reason: string | null;
  adding: boolean;
  menu: ReturnType<typeof createSpaceMapMenu>;
}) {
  const container = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState<{
    left: number;
    top: number;
  } | null>(null);
  const positioned = position !== null;
  const viewport = {
    width: window.visualViewport?.width ?? window.innerWidth,
    height: window.visualViewport?.height ?? window.innerHeight,
    offsetLeft: window.visualViewport?.offsetLeft ?? 0,
    offsetTop: window.visualViewport?.offsetTop ?? 0,
  };
  useLayoutEffect(() => {
    if (container.current)
      setPosition(
        spaceMapMenuPosition(
          target.point,
          container.current.getBoundingClientRect(),
          viewport,
        ),
      );
  }, [target, reason]);
  useLayoutEffect(() => {
    const element = container.current;
    if (!element || !positioned) return;
    (
      element.querySelector<HTMLButtonElement>(
        '[role="menuitem"]:not(:disabled)',
      ) ?? element
    ).focus({ preventScroll: true });
    return listenForSpaceMapMenuEvents(element, (restoreFocus) =>
      menu.close(target, restoreFocus),
    );
  }, [menu, target, positioned]);
  return createPortal(
    <div
      id={id}
      ref={container}
      role="menu"
      aria-label={`${target.label}的操作`}
      aria-describedby={reason ? `${id}-reason` : undefined}
      tabIndex={-1}
      className="space-map-menu"
      style={{
        left: position?.left ?? 0,
        top: position?.top ?? 0,
        maxWidth: Math.max(0, viewport.width - 16),
        maxHeight: Math.max(0, viewport.height - 16),
        visibility: position ? "visible" : "hidden",
      }}
      onContextMenu={(event) => event.preventDefault()}
    >
      <SpaceMapMenuItems
        target={target}
        reason={reason}
        reasonId={`${id}-reason`}
        adding={adding}
        onAdd={() => menu.add(target)}
        onShowFiles={() => menu.showFiles(target)}
      />
    </div>,
    document.body,
  );
}
