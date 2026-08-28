import type { Settings } from "./types";

export const DEFAULT_MAX_OUTPUT_TOKENS = 1800;
export const MAX_OUTPUT_TOKENS_LIMIT = 1_048_576;
export const OUTPUT_TOKENS_ERROR =
  "单次输出上限必须是 1 到 1,048,576 之间的整数。";

export function parseMaxOutputTokens(input: string): number | null {
  const value = input.trim();
  if (!/^\d+$/.test(value)) return null;
  const number = Number(value);
  return Number.isSafeInteger(number) &&
    number >= 1 &&
    number <= MAX_OUTPUT_TOKENS_LIMIT
    ? number
    : null;
}

export function settingsWithOutputLimit(
  settings: Settings,
  input: string,
): Settings {
  const value =
    settings.llm.tokenParameter === "none"
      ? String(settings.llm.maxOutputTokens ?? DEFAULT_MAX_OUTPUT_TOKENS)
      : input;
  const maxOutputTokens = parseMaxOutputTokens(value);
  if (maxOutputTokens === null) throw new Error(OUTPUT_TOKENS_ERROR);
  return { ...settings, llm: { ...settings.llm, maxOutputTokens } };
}
