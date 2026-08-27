import type { ApplicationUnit, UnitComponent } from "./types";

export type UnitRow =
  | { key: string; depth: number; unit: ApplicationUnit; component?: never }
  | { key: string; depth: number; component: UnitComponent; unit?: never };

export function canExpand(unit: ApplicationUnit) {
  return unit.children.length > 0 || unit.components.length > 1;
}

export function applicationRows(
  units: ApplicationUnit[],
  expanded: Set<string>,
): UnitRow[] {
  const rows: UnitRow[] = [];
  function visit(unit: ApplicationUnit, depth: number) {
    rows.push({ key: unit.id, depth, unit });
    if (!expanded.has(unit.id)) return;
    for (const child of unit.children) visit(child, depth + 1);
    if (unit.components.length > 1) {
      for (const component of unit.components) {
        rows.push({
          key: `${unit.id}:${component.entryId}`,
          depth: depth + 1,
          component,
        });
      }
    }
  }
  for (const unit of units) visit(unit, 0);
  return rows;
}

export function expandedSearchResults(units: ApplicationUnit[]): Set<string> {
  const expanded = new Set<string>();
  function visit(unit: ApplicationUnit) {
    if (canExpand(unit)) expanded.add(unit.id);
    unit.children.forEach(visit);
  }
  units.forEach(visit);
  return expanded;
}
