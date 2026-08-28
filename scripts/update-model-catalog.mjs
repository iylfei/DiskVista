import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { buildCatalog, sourceUrl } from "./model-catalog/catalog.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const defaultOutput = path.join(root, "assets", "models", "common-models.json");
const maximumSourceBytes = 32 * 1024 * 1024;

async function download() {
  const response = await fetch(sourceUrl, {
    signal: AbortSignal.timeout(30_000),
    headers: { Accept: "application/json" },
    redirect: "error",
  });
  if (!response.ok)
    throw new Error(`Model catalog download failed: HTTP ${response.status}`);
  if (Number(response.headers.get("content-length")) > maximumSourceBytes) {
    throw new Error("Model catalog exceeds the download limit");
  }
  const chunks = [];
  let size = 0;
  for await (const chunk of response.body) {
    size += chunk.byteLength;
    if (size > maximumSourceBytes)
      throw new Error("Model catalog exceeds the download limit");
    chunks.push(chunk);
  }
  return Buffer.concat(chunks);
}

async function main() {
  const { values } = parseArgs({
    options: {
      input: { type: "string" },
      output: { type: "string" },
      "updated-at": { type: "string" },
      "save-source": { type: "string" },
      check: { type: "boolean", default: false },
      help: { type: "boolean", default: false },
    },
  });
  if (values.help) {
    console.log(`Update the bundled Models.dev metadata (manual command only).

Online update:
  node scripts/update-model-catalog.mjs --save-source output/model-catalog/models.dev-api.json
Reproduce offline with the same source and timestamp:
  node scripts/update-model-catalog.mjs --input <api.json> --updated-at <ISO timestamp>
Verify the committed catalog without network access:
  node scripts/update-model-catalog.mjs --input <api.json> --check

Options: --output <path>, --check (compare without writing), --help`);
    return;
  }
  const output = path.resolve(values.output ?? defaultOutput);
  const previous = values.check ? await fs.readFile(output, "utf8") : null;
  const updatedAt =
    values["updated-at"] ?? (previous ? JSON.parse(previous).updatedAt : null);
  if (values.input && !updatedAt) {
    throw new Error(
      "Offline generation requires --updated-at, or --check with an existing catalog",
    );
  }
  const bytes = values.input
    ? await fs.readFile(values.input)
    : await download();
  const catalog = buildCatalog(bytes, updatedAt ?? new Date().toISOString());
  const serialized = JSON.stringify(catalog, null, 2) + "\n";
  if (values.check) {
    if (previous !== serialized)
      throw new Error(
        "Model catalog differs from the selected source or timestamp",
      );
  } else {
    if (values["save-source"]) {
      const sourceFile = path.resolve(values["save-source"]);
      if (sourceFile === output)
        throw new Error("Source and catalog output must use different paths");
      await fs.mkdir(path.dirname(sourceFile), { recursive: true });
      await fs.writeFile(sourceFile, bytes);
    }
    await fs.mkdir(path.dirname(output), { recursive: true });
    await fs.writeFile(output, serialized, "utf8");
  }
  const models = catalog.providers.flatMap((provider) => provider.models);
  console.log(
    JSON.stringify(
      {
        action: values.check ? "verified" : "updated",
        output,
        sourceSha256: catalog.sourceSha256,
        updatedAt: catalog.updatedAt,
        providers: catalog.providers.length,
        models: models.length,
        missingApi: catalog.providers
          .filter((provider) => provider.api == null)
          .map((provider) => provider.id),
        missingOutput: models.filter((model) => model.output == null).length,
      },
      null,
      2,
    ),
  );
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  await main();
}
