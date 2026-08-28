import { Check, ListPlus } from "lucide-react";
import "./AddToBasketButton.css";

export default function AddToBasketButton({
  name,
  disabledReason,
  disabled = false,
  queued = false,
  adding = false,
  onClick,
}: {
  name: string;
  disabledReason?: string | null;
  disabled?: boolean;
  queued?: boolean;
  adding?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className="icon-button basket-add-button"
      aria-label={`加入待清理清单 ${name}`}
      aria-busy={adding || undefined}
      title={
        queued
          ? "已加入待清理清单"
          : adding
            ? "正在添加…"
            : disabledReason || "加入待清理清单"
      }
      disabled={disabled || queued || adding || !!disabledReason}
      onClick={onClick}
    >
      {queued ? <Check size={16} /> : <ListPlus size={16} />}
    </button>
  );
}
