export type Language = "zh-CN" | "en";

let currentLanguage: Language = "zh-CN";

export function normalizeLanguage(value: unknown): Language {
  return value === "en" ? "en" : "zh-CN";
}

export function getLanguage(): Language {
  if (typeof document !== "undefined" && document.documentElement.lang === "en")
    return "en";
  return currentLanguage;
}

export function setLanguage(value: unknown): void {
  const next = normalizeLanguage(value);
  if (next === currentLanguage) return;
  currentLanguage = next;
  if (typeof document !== "undefined") document.documentElement.lang = next;
}

export function localeName(): string {
  return currentLanguage === "en" ? "en-US" : "zh-CN";
}
