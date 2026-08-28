import { ArrowLeft, ArrowRight, FileSearch, FolderClock } from "lucide-react";
import type { SuggestionGroup } from "../lib/types";
import { bytes } from "../lib/api";
import HelpTip from "./HelpTip";

export function SuggestionListHeader({
  group,
  sort,
  onSort,
  onBack,
}: {
  group?: SuggestionGroup;
  sort: string;
  onSort: (value: string) => void;
  onBack?: () => void;
}) {
  return (
    <div className="suggestion-section-heading">
      {onBack && (
        <button onClick={onBack}>
          <ArrowLeft size={15} />
          返回分类
        </button>
      )}
      <strong>
        {group?.name ?? "文件列表"}
        {group && (
          <HelpTip
            label={`${group.name}的用途与清理影响`}
            text={`用途：${group.purpose} 清理影响：${group.consequence}`}
          />
        )}
      </strong>
      <select
        aria-label="建议排序"
        value={sort}
        onChange={(event) => onSort(event.target.value)}
      >
        <option value="size">按占用空间</option>
        <option value="name">按路径</option>
        <option value="activity_desc">最近变化：从新到旧</option>
        <option value="activity_asc">最近变化：从旧到新</option>
      </select>
    </div>
  );
}

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
