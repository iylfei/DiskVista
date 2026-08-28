import {
  Boxes,
  ChevronDown,
  ChevronRight,
  FolderOpen,
  Info,
} from "lucide-react";
import { bytes } from "../lib/api";
import { canExpand } from "../lib/applicationTree";
import { componentCleanupBlockReason } from "../lib/cleanupTarget";
import type { ApplicationUnit, UnitComponent } from "../lib/types";
import AddToBasketButton from "./AddToBasketButton";

const roleName: Record<string, string> = {
  installation: "安装文件",
  cache: "缓存",
  application_data: "应用数据",
  temporary: "临时文件",
  diagnostic: "日志与诊断",
  user_data: "已标注的数据",
  unconfirmed: "应用文件",
  unclassified: "未归属部分",
};

export function UnitSummary({
  unit,
  expanded = false,
  onClick,
}: {
  unit: ApplicationUnit;
  expanded?: boolean;
  onClick: () => void;
}) {
  const expandable = canExpand(unit);
  const note = [
    unit.kind === "application" && unit.confidence === "high" && "已识别应用",
    unit.kind === "application_data" && "应用数据",
    unit.kind === "possible_application" && "可能的应用目录",
    unit.kind === "unassigned" && "未归属部分 · 已扣除单独列出的应用",
    unit.estimated && "估算",
    !unit.complete && "未扫描完整",
    unit.children.length > 0 && "含子项",
  ]
    .filter(Boolean)
    .join(" · ");
  return (
    <button
      className={`unit-row${note ? " unit-row-with-note" : ""}`}
      onClick={onClick}
      aria-expanded={expandable ? expanded : undefined}
      aria-label={`${expandable ? (expanded ? "收起" : "展开") : "查看文件"} ${unit.name}`}
    >
      {expandable ? (
        expanded ? (
          <ChevronDown size={17} />
        ) : (
          <ChevronRight size={17} />
        )
      ) : (
        <Boxes size={19} />
      )}
      <span className="unit-name">
        <strong title={unit.name}>{unit.name}</strong>
      </span>
      <span className="unit-size">
        <strong>{bytes(unit.occupiedBytes)}</strong>
      </span>
      <span className="unit-count">
        {unit.fileCount.toLocaleString()} 个文件
      </span>
      <span className="unit-action">
        {expandable
          ? expanded
            ? "收起"
            : `展开 ${unit.children.length + (unit.components.length > 1 ? unit.components.length : 0)} 项`
          : "查看文件"}
      </span>
      {note && (
        <small className="unit-note" title={note}>
          {note}
        </small>
      )}
    </button>
  );
}

export function ComponentSummary({
  component,
  onOpen,
  onDetail,
  onAddToBasket,
  busy = false,
  queued = false,
  adding = false,
}: {
  component: UnitComponent;
  onOpen: () => void;
  onDetail: () => void;
  onAddToBasket: () => void;
  busy?: boolean;
  queued?: boolean;
  adding?: boolean;
}) {
  return (
    <div className="unit-component-row">
      <button
        className="unit-row"
        onClick={onOpen}
        aria-label={`查看文件 ${component.path}`}
      >
        <FolderOpen size={18} />
        <span className="unit-name">
          <strong>{roleName[component.role] ?? "文件位置"}</strong>
          <small title={component.path}>{component.path}</small>
        </span>
        <span className="unit-size" title={component.evidence}>
          <strong>{bytes(component.occupiedBytes)}</strong>
        </span>
        <span className="unit-count">
          {component.fileCount.toLocaleString()} 个文件
        </span>
        <span className="unit-action">查看文件</span>
      </button>
      <div className="unit-actions">
        <button
          className="icon-button unit-inspect"
          onClick={onDetail}
          aria-label={`查看详情 ${component.path}`}
          title="查看详情"
        >
          <Info size={16} />
        </button>
        <AddToBasketButton
          name={component.path}
          disabled={busy}
          queued={queued}
          adding={adding}
          disabledReason={componentCleanupBlockReason(component)}
          onClick={onAddToBasket}
        />
      </div>
    </div>
  );
}
