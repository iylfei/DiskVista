import assert from "node:assert/strict";
import { createHash } from "node:crypto";

export const sourceUrl = "https://models.dev/api.json";
export const providerIds = Object.freeze([
  "openai",
  "anthropic",
  "google",
  "deepseek",
  "zai",
  "zai-coding-plan",
  "zhipuai",
  "zhipuai-coding-plan",
  "moonshotai",
  "moonshotai-cn",
  "kimi-for-coding",
  "alibaba",
  "alibaba-cn",
  "alibaba-coding-plan",
  "alibaba-coding-plan-cn",
  "alibaba-token-plan",
  "alibaba-token-plan-cn",
  "minimax",
  "minimax-cn",
  "minimax-coding-plan",
  "minimax-cn-coding-plan",
  "xai",
  "opencode",
  "opencode-go",
]);

function requiredString(value, field) {
  assert.equal(typeof value, "string", `${field} must be a string`);
  assert(value.trim().length > 0, `${field} must not be empty`);
  return value;
}

function optionalString(value, field) {
  return value == null || value === "" ? null : requiredString(value, field);
}

function limit(value, field) {
  if (value == null || value === 0) return null;
  assert(
    Number.isSafeInteger(value) && value > 0,
    `${field} must be a positive integer, zero, or absent`,
  );
  return value;
}

function nullableBoolean(value, field) {
  if (value == null) return null;
  assert.equal(typeof value, "boolean", `${field} must be boolean or absent`);
  return value;
}

function textModel(id, model) {
  const nonGenerative =
    /(^|[\/_. -])(embeddings?|rerank(?:er)?)(?=$|[\/_. -])/i;
  // Some upstream embedding rows report vector dimensions as a text output limit.
  if (
    [model.type, model.family, id].some(
      (value) => typeof value === "string" && nonGenerative.test(value),
    )
  ) {
    return false;
  }
  return (
    Array.isArray(model.modalities?.input) &&
    model.modalities.input.includes("text") &&
    Array.isArray(model.modalities?.output) &&
    model.modalities.output.length === 1 &&
    model.modalities.output[0] === "text"
  );
}

export function buildCatalog(
  bytes,
  updatedAt,
  selectedProviders = providerIds,
) {
  assert(
    typeof updatedAt === "string" &&
      /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/.test(
        updatedAt,
      ),
    "The update timestamp must use ISO 8601 with an explicit time zone",
  );
  const timestamp = new Date(updatedAt);
  assert(
    Number.isFinite(timestamp.getTime()),
    "A valid update timestamp is required",
  );
  const source = JSON.parse(
    new TextDecoder("utf-8", { fatal: true }).decode(bytes),
  );
  assert(source && typeof source === "object" && !Array.isArray(source));
  assert.equal(new Set(selectedProviders).size, selectedProviders.length);
  const providers = selectedProviders.map((id) => {
    const provider = source[id];
    assert(provider && typeof provider === "object", `Missing provider: ${id}`);
    if (provider.id != null)
      assert.equal(provider.id, id, `Provider ID mismatch: ${id}`);
    const npm = optionalString(provider.npm, `${id}.npm`);
    assert(
      provider.models && typeof provider.models === "object",
      `Missing models: ${id}`,
    );
    const models = Object.keys(provider.models)
      .sort()
      .flatMap((modelId) => {
        const model = provider.models[modelId];
        if (!textModel(modelId, model)) return [];
        if (model.id != null)
          assert.equal(
            model.id,
            modelId,
            `Model ID mismatch: ${id}/${modelId}`,
          );
        return [
          {
            id: requiredString(modelId, `${id}.model.id`),
            name: requiredString(model.name, `${id}/${modelId}.name`),
            family: optionalString(model.family, `${id}/${modelId}.family`),
            context: limit(model.limit?.context, `${id}/${modelId}.context`),
            output: limit(model.limit?.output, `${id}/${modelId}.output`),
            reasoning: nullableBoolean(
              model.reasoning,
              `${id}/${modelId}.reasoning`,
            ),
            npm:
              optionalString(model.provider?.npm, `${id}/${modelId}.npm`) ??
              npm,
            status: optionalString(model.status, `${id}/${modelId}.status`),
          },
        ];
      });
    assert(models.length > 0, `No verified text models remain for ${id}`);
    return {
      id,
      name: requiredString(provider.name, `${id}.name`),
      api: optionalString(provider.api, `${id}.api`),
      npm,
      doc: optionalString(provider.doc, `${id}.doc`),
      models,
    };
  });
  return {
    schemaVersion: 1,
    sourceUrl,
    sourceSha256: createHash("sha256").update(bytes).digest("hex"),
    updatedAt: timestamp.toISOString(),
    providers,
  };
}
