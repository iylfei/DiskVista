// Build-time license inventory. Generated output preserves the original upstream texts.
import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
const root = path.resolve(import.meta.dirname, "..");
const out = path.join(root, "third-party", "dependencies");
fs.mkdirSync(out, { recursive: true });
const meta = JSON.parse(
  execFileSync(
    "cargo",
    [
      "metadata",
      "--locked",
      "--offline",
      "--format-version",
      "1",
      "--filter-platform",
      "x86_64-pc-windows-msvc",
    ],
    { cwd: root, encoding: "utf8", maxBuffer: 30 * 1024 * 1024 },
  ),
);
const inventory = [];
function collect(kind, name, version, license, directory, repository) {
  const id = `${kind}-${name.replaceAll("/", "-").replaceAll("@", "")}-${version}`;
  const dest = path.join(out, id);
  fs.mkdirSync(dest, { recursive: true });
  const files = fs
    .readdirSync(directory, { withFileTypes: true })
    .filter(
      (f) =>
        f.isFile() &&
        /^(licen[cs]e|copying|copyright|notice)([._-]|$)/i.test(f.name),
    );
  for (const f of files)
    fs.copyFileSync(path.join(directory, f.name), path.join(dest, f.name));
  // Some crates place the full license in a dedicated licenses directory.
  const nested = path.join(directory, "licenses");
  if (fs.existsSync(nested))
    fs.cpSync(nested, path.join(dest, "licenses"), { recursive: true });
  inventory.push({
    kind,
    name,
    version,
    license: license ?? "UNSPECIFIED",
    repository: repository ?? null,
    texts: files.map((f) => `${id}/${f.name}`),
    hasLicenseDirectory: fs.existsSync(nested),
  });
}
for (const p of meta.packages.filter((p) => p.source))
  collect(
    "rust",
    p.name,
    p.version,
    p.license,
    path.dirname(p.manifest_path),
    p.repository,
  );
const pnpm = path.join(root, "node_modules", ".pnpm");
for (const entry of fs
  .readdirSync(pnpm, { withFileTypes: true })
  .filter((e) => e.isDirectory())) {
  const modules = path.join(pnpm, entry.name, "node_modules");
  if (!fs.existsSync(modules)) continue;
  for (const child of fs
    .readdirSync(modules, { withFileTypes: true })
    .filter((e) => e.isDirectory())) {
    const dirs = child.name.startsWith("@")
      ? fs
          .readdirSync(path.join(modules, child.name), { withFileTypes: true })
          .filter((e) => e.isDirectory())
          .map((e) => path.join(modules, child.name, e.name))
      : [path.join(modules, child.name)];
    for (const directory of dirs) {
      const file = path.join(directory, "package.json");
      if (!fs.existsSync(file)) continue;
      const p = JSON.parse(fs.readFileSync(file, "utf8"));
      if (
        inventory.some(
          (i) =>
            i.kind === "npm" && i.name === p.name && i.version === p.version,
        )
      )
        continue;
      collect(
        "npm",
        p.name,
        p.version,
        p.license,
        directory,
        typeof p.repository === "string" ? p.repository : p.repository?.url,
      );
    }
  }
}
const repoKey = (repo) =>
  (repo ?? "")
    .replace(/^git\+|^git:\/\//g, (match) =>
      match === "git://" ? "https://" : "",
    )
    .replace(/\.git\/?$|\/$/g, "");
const licenseKey = (license) =>
  String(license)
    .replaceAll("/", " OR ")
    .split(/\s+OR\s+/)
    .sort()
    .join(" OR ");
for (const row of inventory.filter(
  (p) => !p.texts.length && !p.hasLicenseDirectory,
)) {
  const sibling = inventory.find(
    (p) =>
      p.texts.length &&
      repoKey(p.repository) === repoKey(row.repository) &&
      licenseKey(p.license) === licenseKey(row.license),
  );
  const id = `${row.kind}-${row.name.replaceAll("/", "-").replaceAll("@", "")}-${row.version}`;
  if (sibling) {
    for (const file of sibling.texts) {
      const dest = `${id}/${path.basename(file)}`;
      fs.copyFileSync(path.join(out, file), path.join(out, dest));
      row.texts.push(dest);
    }
    row.licenseSource = `Same upstream repository: ${sibling.name} ${sibling.version}`;
    continue;
  }
  if (row.kind !== "rust") continue;
  const p = meta.packages.find(
    (p) => p.name === row.name && p.version === row.version,
  );
  const dir = path.dirname(p.manifest_path);
  const vcs = path.join(dir, ".cargo_vcs_info.json");
  if (!fs.existsSync(vcs)) continue;
  const commit = JSON.parse(fs.readFileSync(vcs, "utf8")).git?.sha1;
  const repository = repoKey(p.repository).replace("https://github.com/", "");
  if (!commit || repository.includes("://")) continue;
  const cache = path.join(
    root,
    "third-party",
    "upstream-licenses",
    repository.replaceAll("/", "-"),
    commit,
  );
  fs.mkdirSync(cache, { recursive: true });
  for (const filename of row.license === "MPL-2.0"
    ? ["LICENSE-MPL-2.0", "LICENSE", "COPYING"]
    : ["LICENSE", "LICENSE-MIT", "LICENSE-APACHE", "COPYING"]) {
    const cached = path.join(cache, filename);
    const url = `https://raw.githubusercontent.com/${repository}/${commit}/${filename}`;
    if (!fs.existsSync(cached)) {
      const response = await fetch(url);
      if (!response.ok) continue;
      fs.writeFileSync(cached, await response.text(), "utf8");
    }
    const dest = `${id}/${filename}`;
    fs.copyFileSync(cached, path.join(out, dest));
    row.texts.push(dest);
    row.licenseSource = `${repository}@${commit}`;
  }
  if (row.license === "MPL-2.0" && !row.texts.length) {
    const common = inventory.find(
      (p) =>
        p.name === "cssparser" && p.license === "MPL-2.0" && p.texts.length,
    );
    if (common) {
      for (const file of common.texts) {
        const dest = `${id}/${path.basename(file)}`;
        fs.copyFileSync(path.join(out, file), path.join(out, dest));
        row.texts.push(dest);
      }
      row.licenseSource =
        "Standard MPL-2.0 text; exact unmodified source with original attribution is in source-dist";
    }
  }
}
// MPL source availability: retain the exact, unmodified crate source alongside notices.
for (const p of meta.packages.filter(
  (p) => p.source && p.license?.includes("MPL-2.0"),
)) {
  const dest = path.join(
    root,
    "third-party",
    "source-dist",
    `${p.name}-${p.version}`,
  );
  fs.cpSync(path.dirname(p.manifest_path), dest, {
    recursive: true,
    filter: (source) => !path.basename(source).startsWith(".cargo"),
  });
}
inventory.sort((a, b) =>
  (a.kind + a.name + a.version).localeCompare(b.kind + b.name + b.version),
);
fs.writeFileSync(
  path.join(out, "inventory.json"),
  JSON.stringify(inventory, null, 2) + "\n",
  "utf8",
);
fs.writeFileSync(
  path.join(out, "README.md"),
  "# Third-party dependency licenses\n\nGenerated from Cargo.lock and pnpm-lock.yaml, including build/test dependencies. Original license texts are retained in the package subdirectories. This inventory does not change their terms.\n\n" +
    inventory
      .map((p) => `- ${p.kind} ${p.name} ${p.version}: ${p.license}`)
      .join("\n") +
    "\n",
  "utf8",
);
console.log(
  `Collected ${inventory.length} dependency notices; ${inventory.filter((p) => !p.texts.length && !p.hasLicenseDirectory).length} packages without root license texts (inspect inventory).`,
);
