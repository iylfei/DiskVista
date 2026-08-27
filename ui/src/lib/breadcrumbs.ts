export interface Breadcrumb {
  path: string;
  label: string;
}

function clean(path: string) {
  const value = path.replaceAll("/", "\\").replace(/\\+$/, "");
  return /^[a-z]:$/i.test(value) ? `${value}\\` : value;
}

export function breadcrumbs(root: string, current: string): Breadcrumb[] {
  const start = clean(root);
  const path = clean(current);
  const parts: Breadcrumb[] = [{ path: start, label: start }];
  const prefix = start.endsWith("\\") ? start : `${start}\\`;
  if (!path.toLowerCase().startsWith(prefix.toLowerCase())) return parts;
  let parent = start;
  for (const label of path.slice(prefix.length).split("\\").filter(Boolean)) {
    parent = `${parent.endsWith("\\") ? parent : `${parent}\\`}${label}`;
    parts.push({ path: parent, label });
  }
  return parts;
}
