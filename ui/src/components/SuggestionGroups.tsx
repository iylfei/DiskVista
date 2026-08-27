import { ArrowRight, FileSearch, FolderClock } from "lucide-react";
import type { SuggestionGroup } from "../lib/types";
import { bytes } from "../lib/api";

export default function SuggestionGroups({
  groups,
  onOpen,
}: {
  groups: SuggestionGroup[];
  onOpen: (id: string) => void;
}) {
  return (
    <div className="suggestion-groups">
      {groups.map((group) => (
        <article className="suggestion-group" key={group.id}>
          {group.recognized ? (
            <FolderClock size={22} />
          ) : (
            <FileSearch size={22} />
          )}
          <div>
            <h2>{group.name}</h2>
            <p>{group.purpose}</p>
            <p className="group-impact">清理影响：{group.consequence}</p>
          </div>
          <div className="suggestion-group-size">
            <strong>
              {bytes(group.occupiedBytes)}
              {group.estimated ? "（估算）" : ""}
            </strong>
            <small>{group.count.toLocaleString()} 个文件或文件夹</small>
            <button onClick={() => onOpen(group.id)}>
              查看文件
              <ArrowRight size={14} />
            </button>
          </div>
        </article>
      ))}
    </div>
  );
}
