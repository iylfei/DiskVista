import { ArrowRight, ListChecks } from "lucide-react";
import type { FileRecord } from "../lib/types";
import { selectedUsage } from "../lib/selection";
import { bytes } from "../lib/api";

export default function SelectionBar({
  items,
  onOpen,
}: {
  items: FileRecord[];
  onOpen: () => void;
}) {
  if (!items.length) return null;
  return (
    <div className="selection-bar" role="region" aria-label="已选清理项目">
      <span>
        <ListChecks size={17} />
        已选 <strong>{items.length}</strong> 项 · 占用约{" "}
        {bytes(selectedUsage(items))}
      </span>
      <button className="primary" onClick={onOpen}>
        检查已选内容
        <ArrowRight size={15} />
      </button>
    </div>
  );
}
