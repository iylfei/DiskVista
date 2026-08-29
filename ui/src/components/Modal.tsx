import type { ReactNode } from "react";
import { useEffect, useId, useRef } from "react";
import { X } from "lucide-react";
export default function Modal({
  title,
  children,
  onClose,
  wide = false,
  compact = false,
  closeDisabled = false,
}: {
  title: string;
  children: ReactNode;
  onClose: () => void;
  wide?: boolean;
  compact?: boolean;
  closeDisabled?: boolean;
}) {
  const titleId = useId();
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    ref.current?.showModal();
    return () => ref.current?.close();
  }, []);
  return (
    <dialog
      className={`modal${wide ? " wide" : ""}${compact ? " compact" : ""}`}
      aria-labelledby={titleId}
      ref={ref}
      onCancel={(e) => {
        e.preventDefault();
        if (!closeDisabled) onClose();
      }}
    >
      <header>
        <h2 id={titleId}>{title}</h2>
        <button
          type="button"
          className="icon-button"
          aria-label="关闭对话框"
          disabled={closeDisabled}
          onClick={() => {
            if (!closeDisabled) onClose();
          }}
        >
          <X size={18} />
        </button>
      </header>
      {children}
    </dialog>
  );
}
