import {
  Boxes,
  ChevronDown,
  ChevronRight,
  FolderOpen,
  Info,
} from "lucide-react";
import { bytes } from "../lib/api";
import { canExpand } from "../lib/applicationTree";
import type { ApplicationUnit, UnitComponent } from "../lib/types";

const roleName: Record<string, string> = {
  installation: "安装文件",
  cache: "缓存与应用数据",
  user_data: "已标注的数据",
  unconfirmed: "应用文件",
  unclassified: "其他文件",
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
  return (
    <button
      className="unit-row"
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
        {unit.kind === "application" && unit.confidence === "high" && (
          <small>已识别应用</small>
        )}
      </span>
      <span className="unit-size">
        <strong>{bytes(unit.occupiedBytes)}</strong>
        {(unit.estimated || !unit.complete || unit.children.length > 0) && (
          <small>
            {[
              unit.estimated && "估算",
              !unit.complete && "未扫描完整",
              unit.children.length > 0 && "含子项",
            ]
              .filter(Boolean)
              .join(" · ")}
          </small>
        )}
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
    </button>
  );
}

export function ComponentSummary({
  component,
  onOpen,
  onDetail,
}: {
  component: UnitComponent;
  onOpen: () => void;
  onDetail: () => void;
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
        <span className="unit-size">
          <strong>{bytes(component.occupiedBytes)}</strong>
        </span>
        <span className="unit-count">
          {component.fileCount.toLocaleString()} 个文件
        </span>
        <span className="unit-action">查看文件</span>
      </button>
      <button
        className="icon-button unit-inspect"
        onClick={onDetail}
        aria-label={`查看详情 ${component.path}`}
      >
        <Info size={16} />
      </button>
    </div>
  );
}
