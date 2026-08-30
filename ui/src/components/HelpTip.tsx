import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { CircleHelp } from "lucide-react";
import { helpPosition } from "../lib/helpPosition";
import { translateText } from "../i18n/translate";
import "./help.css";

/** An explanation, never the only place for a warning or a required instruction. */
export default function HelpTip({
  label,
  text,
}: {
  label: string;
  text: string;
}) {
  const id = useId();
  const trigger = useRef<HTMLButtonElement>(null);
  const tip = useRef<HTMLDivElement>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState<{
    left: number;
    top: number;
  } | null>(null);
  const localizedLabel = translateText(label);
  const localizedText = translateText(text);
  function cancelClose() {
    if (timer.current !== null) clearTimeout(timer.current);
    timer.current = null;
  }
  function show() {
    cancelClose();
    window.dispatchEvent(
      new CustomEvent("diskvista:help-open", { detail: id }),
    );
    setOpen(true);
  }
  function scheduleClose() {
    cancelClose();
    timer.current = setTimeout(() => {
      if (
        document.activeElement !== trigger.current &&
        !tip.current?.matches(":hover")
      )
        setOpen(false);
    }, 140);
  }
  useEffect(() => cancelClose, []);
  useEffect(() => {
    const closeOther = (event: Event) => {
      if ((event as CustomEvent<string>).detail !== id) setOpen(false);
    };
    window.addEventListener("diskvista:help-open", closeOther);
    return () => window.removeEventListener("diskvista:help-open", closeOther);
  }, [id]);
  useLayoutEffect(() => {
    if (!open) {
      setPosition(null);
      return;
    }
    const place = () => {
      if (!trigger.current || !tip.current) return;
      setPosition(
        helpPosition(
          trigger.current.getBoundingClientRect(),
          tip.current.getBoundingClientRect(),
          { width: window.innerWidth, height: window.innerHeight },
        ),
      );
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        // Dismiss help first, not the surrounding confirmation dialog.
        event.preventDefault();
        event.stopImmediatePropagation();
        setOpen(false);
      }
    };
    const outside = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!trigger.current?.contains(target) && !tip.current?.contains(target))
        setOpen(false);
    };
    place();
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    window.addEventListener("keydown", escape, true);
    window.addEventListener("pointerdown", outside, true);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
      window.removeEventListener("keydown", escape, true);
      window.removeEventListener("pointerdown", outside, true);
    };
  }, [open]);
  return (
    <>
      <sup className="help-anchor">
        {/* Keep the superscript with the preceding character when text wraps. */}
        {"\u2060"}
        <button
          ref={trigger}
          type="button"
          className="help-trigger"
          aria-label={translateText(`关于${localizedLabel}`)}
          aria-describedby={open ? id : undefined}
          onPointerEnter={show}
          onPointerLeave={scheduleClose}
          onFocus={show}
          onBlur={scheduleClose}
          onClick={(event) => {
            event.preventDefault();
            event.stopPropagation();
            show();
          }}
        >
          <CircleHelp size={12} aria-hidden="true" />
        </button>
      </sup>
      {open &&
        createPortal(
          <div
            id={id}
            ref={tip}
            role="tooltip"
            className="help-tooltip"
            style={{
              left: position?.left ?? 0,
              top: position?.top ?? 0,
              visibility: position ? "visible" : "hidden",
            }}
            onPointerEnter={cancelClose}
            onPointerLeave={scheduleClose}
          >
            <strong>{localizedLabel}</strong>
            <p>{localizedText}</p>
          </div>,
          // A modal makes the rest of the document inert. Keep its help inside it.
          trigger.current?.closest("dialog") ?? document.body,
        )}
    </>
  );
}
