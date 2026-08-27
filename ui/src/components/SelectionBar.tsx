import { ListChecks, Plus } from "lucide-react";
import type { FileRecord } from "../lib/types";
import { selectedUsage } from "../lib/selection";
import { bytes } from "../lib/api";

export default function SelectionBar({
  items,
  message,
  onAdd,
}: {
  items: FileRecord[];
  message: string;
  onAdd: () => void;
}) {
  if (!items.length && !message) return null;
  return (
    <div className="selection-bar" role="region" aria-label="已选清理项目">
      <span role="status">
        <ListChecks size={17} />
        {items.length ? (
          <>
            已选 <strong>{items.length}</strong> 项 · 占用约{" "}
            {bytes(selectedUsage(items))}
          </>
        ) : (
          message
        )}
      </span>
      {items.length > 0 && (
        <button className="primary" onClick={onAdd}>
          <Plus size={15} />
          添加到待清理清单
        </button>
      )}
    </div>
  );
}
