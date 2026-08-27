import fs from "node:fs/promises";
import path from "node:path";
import crypto from "node:crypto";
import { fileURLToPath } from "node:url";
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
export const commit = "b8fa0bd9cc5e59f17a34fe71c5464c7450180a37";
const base = `https://raw.githubusercontent.com/MoscaDotTo/Winapp2/${commit}`;
export function convert(text) {
  const entries = [];
  let current = null;
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    if (!line || line.startsWith(";")) continue;
    if (line.startsWith("[") && line.endsWith("]")) {
      current = { name: line.slice(1, -1), values: [] };
      entries.push(current);
    } else if (current) {
      const i = line.indexOf("=");
      if (i < 1) {
        current.values.push(["!invalid", line]);
        continue;
      }
      current.values.push([line.slice(0, i), line.slice(i + 1)]);
    }
  }
  const rules = [],
    skipped = [];
  for (const [index, entry] of entries.entries()) {
    const unsupported = entry.values.filter(
      ([key]) =>
        !["LangSecRef", "Section", "DetectFile", "Default", "Warning"].includes(
          key,
        ) && !/^FileKey\d+$/.test(key),
    );
    const detects = entry.values
      .filter(([key]) => key === "DetectFile")
      .map(([, v]) => v);
    const files = entry.values.filter(([key]) => /^FileKey\d+$/.test(key));
    let reason = "";
    if (unsupported.length)
      reason = `Unsupported semantics: ${unsupported.map(([k]) => k).join(", ")}`;
    else if (detects.length !== 1 || /[?*|]/.test(detects[0]))
      reason = "Requires exactly one literal DetectFile";
    else if (!files.length) reason = "No file target";
    const targets = [];
    for (const [, value] of files) {
      const [directory, pattern, flags, ...extra] = value.split("|");
      if (pattern !== "*" || flags !== "REMOVESELF" || extra.length) {
        reason ||= "Only whole-directory *|REMOVESELF is supported";
        break;
      }
      if (
        !directory.toLowerCase().startsWith(detects[0]?.toLowerCase() + "\\")
      ) {
        reason ||= "Detection must be a literal ancestor of every target";
        break;
      }
      if (
        directory.includes("..") ||
        directory.startsWith("\\\\") ||
        !/^%(localappdata|appdata|userprofile)%\\/i.test(directory)
      ) {
        reason ||= "Target outside allowed per-user roots";
        break;
      }
      if (!/\\([^\\]*(cache|temp|logs?)[^\\]*)$/i.test(directory)) {
        reason ||= "Target is not an explicitly named cache/temp/log directory";
        break;
      }
      targets.push(directory);
    }
    if (reason) {
      skipped.push({ name: entry.name, reason });
      continue;
    }
    for (const [n, target] of targets.entries())
      rules.push({
        id: `winapp2-${index}-${n}`,
        name: entry.name,
        root: target,
        detectFiles: detects,
        category: "cache",
        owner: entry.name
          .replace(/ (Caches?|Logs?|Temporary Files).*$/i, "")
          .replace(/ \*$/, ""),
        ageDays: 30,
        purpose: "社区规则识别的缓存、临时或日志目录；用途仍需人工复核",
        consequence:
          "可能丢失诊断记录、离线缓存或需要重新下载；不能保证可再生成",
        recovery: "优先从 Windows 回收站恢复，不保证应用能重新生成",
        warning: entry.values
          .filter(([k]) => k === "Warning")
          .map(([, v]) => v)
          .join("\n"),
        community: true,
        excludes: [],
      });
  }
  return { rules, skipped };
}
if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const fetchText = async (name) => {
    const r = await fetch(`${base}/${name}`);
    if (!r.ok) throw new Error(`Download ${name}: ${r.status}`);
    return r.text();
  };
  const [source, license] = await Promise.all([
    fetchText("Winapp2.ini"),
    fetchText("License.md"),
  ]);
  const report = convert(source);
  const dest = path.join(root, "assets", "rules");
  const upstream = path.join(root, "third-party", "winapp2");
  await fs.mkdir(upstream, { recursive: true });
  await fs.writeFile(path.join(upstream, "Winapp2.ini"), source, "utf8");
  await fs.writeFile(path.join(upstream, "License.md"), license, "utf8");
  const pack = {
    version: `winapp2-${commit.slice(0, 12)}`,
    source: `https://github.com/MoscaDotTo/Winapp2/tree/${commit}`,
    commit,
    sourceSha256: crypto.createHash("sha256").update(source).digest("hex"),
    license: "CC-BY-SA-4.0",
    attribution: "Winapp2 project and contributors (MoscaDotTo/Winapp2)",
    modifications:
      "Conservative directory-only conversion by DiskVista. Unsupported sections skipped in full. No registry writes, scripts, or Winapp3. All converted suggestions require review.",
    rules: report.rules,
    skipped: report.skipped,
  };
  await fs.writeFile(
    path.join(dest, "community.json"),
    JSON.stringify(pack, null, 2) + "\n",
    "utf8",
  );
  console.log(
    `Pinned ${commit}: ${report.rules.length} targets, ${report.skipped.length} entire entries skipped.`,
  );
}
