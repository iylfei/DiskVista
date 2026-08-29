import { getLanguage } from "./locale";
import { englishText } from "./english";

export function translateText(value: string): string {
  return getLanguage() === "en" ? englishText(value) : value;
}

const localizedProperties = new Set([
  "aria-label",
  "aria-description",
  "alt",
  "placeholder",
  "title",
]);

function translateChildren(value: unknown): unknown {
  if (typeof value === "string") return translateText(value);
  if (Array.isArray(value)) return value.map(translateChildren);
  return value;
}

export function localizedProps(
  type: unknown,
  props: Record<string, unknown> | null,
): Record<string, unknown> | null {
  if (typeof type !== "string" || props === null) return props;
  let changed = false;
  const next = { ...props };
  if ("children" in next) {
    const children = translateChildren(next.children);
    if (children !== next.children) {
      next.children = children;
      changed = true;
    }
  }
  for (const property of localizedProperties) {
    const value = next[property];
    if (typeof value !== "string") continue;
    const translated = translateText(value);
    if (translated !== value) {
      next[property] = translated;
      changed = true;
    }
  }
  return changed ? next : props;
}
