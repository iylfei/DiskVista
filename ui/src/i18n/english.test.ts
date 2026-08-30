import { describe, expect, it } from "vitest";
import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { extname, join } from "node:path";
import ts from "typescript";
import { englishText } from "./english";

const chinese = /\p{Script=Han}/u;
const sourceRoot = fileURLToPath(new URL("../", import.meta.url));

function sourceFiles(folder: string): string[] {
  return readdirSync(folder, { withFileTypes: true }).flatMap((entry) => {
    const path = join(folder, entry.name);
    if (entry.isDirectory()) return sourceFiles(path);
    if (![".ts", ".tsx"].includes(extname(path))) return [];
    if (path.includes(".test.") || path.endsWith(join("i18n", "english.ts")))
      return [];
    return [path];
  });
}

function interfaceText(path: string): string[] {
  const contents = readFileSync(path, "utf8");
  const source = ts.createSourceFile(
    path,
    contents,
    ts.ScriptTarget.Latest,
    true,
    path.endsWith(".tsx") ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
  );
  const values: string[] = [];
  function visit(node: ts.Node): void {
    if (
      (ts.isStringLiteral(node) ||
        ts.isNoSubstitutionTemplateLiteral(node) ||
        ts.isJsxText(node)) &&
      chinese.test(node.text)
    ) {
      const value = node.text.replace(/\s+/g, " ").trim();
      if (value) values.push(value);
    }
    if (ts.isTemplateExpression(node) && chinese.test(node.getText(source))) {
      let value = node.head.text;
      node.templateSpans.forEach((span, index) => {
        value += `{${index + 1}}${span.literal.text}`;
      });
      values.push(value.replace(/\s+/g, " ").trim());
    }
    ts.forEachChild(node, visit);
  }
  visit(source);
  return values;
}

describe("English interface text", () => {
  it("translates whitespace-wrapped fragments and dynamic scan messages", () => {
    expect(englishText(" 可用 ")).toBe(" Available ");
    expect(englishText("正在分析本批 20 个文件…")).toBe(
      "Analyzing this batch of 20 files…",
    );
  });

  it("covers every Chinese user-facing string in production UI sources", () => {
    const missing = new Set<string>();
    for (const value of sourceFiles(sourceRoot).flatMap(interfaceText)) {
      const example = value.replace(/\{\d+\}/g, "42");
      if (chinese.test(englishText(example))) missing.add(value);
    }
    expect([...missing]).toEqual([]);
  });
});
