import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import { createHash } from "node:crypto";
import {
  buildCatalog,
  providerIds,
  sourceUrl,
} from "./model-catalog/catalog.mjs";

const updatedAt = "2026-08-28T00:00:00.000Z";
const text = (name, extra = {}) => ({
  name,
  modalities: { input: ["text", "image"], output: ["text"] },
  limit: { context: 123_456, output: 7_890 },
  reasoning: true,
  ...extra,
});
const source = (models, extra = {}) =>
  Buffer.from(
    JSON.stringify({
      fixture: {
        id: "fixture",
        name: "Fixture",
        npm: "@ai-sdk/openai-compatible",
        models,
        ...extra,
      },
    }),
  );

test("text metadata excludes embeddings, media generation, audio-only input, and unknown modalities", () => {
  const bytes = source({
    chat: text("Chat"),
    embedding: text("Embedding", {
      modalities: { input: ["text"], output: ["embedding"] },
    }),
    image: text("Image", {
      modalities: { input: ["text"], output: ["text", "image"] },
    }),
    audio: text("Audio", {
      modalities: { input: ["text"], output: ["audio"] },
    }),
    video: text("Video", {
      modalities: { input: ["text"], output: ["video"] },
    }),
    transcription: text("Transcription", {
      modalities: { input: ["audio"], output: ["text"] },
    }),
    unknown: text("Unknown", { modalities: undefined }),
    mislabeledFamily: text("Vector model", { family: "text-embedding" }),
    "gemini-embedding-001": text("Gemini Embedding 001", { family: "gemini" }),
    mislabeledType: text("Rank model", { type: "reranker" }),
  });
  const catalog = buildCatalog(bytes, updatedAt, ["fixture"]);
  assert.deepEqual(
    catalog.providers[0].models.map((model) => model.id),
    ["chat"],
  );
  assert.equal(
    catalog.sourceSha256,
    createHash("sha256").update(bytes).digest("hex"),
  );
});

test("provider limits, SDK overrides, original IDs, and deprecated status are copied exactly", () => {
  const bytes = source({
    "model-v2": text("Model V2", {
      provider: { npm: "@ai-sdk/anthropic" },
      status: "deprecated",
      family: "fixture",
    }),
    "Model-V1": text("Model V1", { limit: { context: 99_000, output: 1_234 } }),
  });
  const provider = buildCatalog(bytes, updatedAt, ["fixture"]).providers[0];
  assert.equal(provider.api, null);
  assert.deepEqual(
    provider.models.map((model) => model.id),
    ["Model-V1", "model-v2"],
  );
  assert.equal(provider.models[0].output, 1_234);
  assert.equal(provider.models[0].npm, "@ai-sdk/openai-compatible");
  assert.deepEqual(provider.models[1], {
    id: "model-v2",
    name: "Model V2",
    family: "fixture",
    context: 123_456,
    output: 7_890,
    reasoning: true,
    npm: "@ai-sdk/anthropic",
    status: "deprecated",
  });
  assert.deepEqual(
    buildCatalog(bytes, updatedAt, ["fixture"]),
    buildCatalog(bytes, updatedAt, ["fixture"]),
  );
});

test("missing values remain unknown and malformed limits or provider IDs fail", () => {
  assert.throws(() =>
    buildCatalog(source({ chat: text("Chat") }), "2026-08-28", ["fixture"]),
  );
  const catalog = buildCatalog(
    source({
      chat: text("Chat", { limit: { context: 0 }, reasoning: undefined }),
    }),
    updatedAt,
    ["fixture"],
  );
  const model = catalog.providers[0].models[0];
  assert.equal(model.context, null);
  assert.equal(model.output, null);
  assert.equal(model.reasoning, null);
  assert.throws(() =>
    buildCatalog(
      source({ chat: text("Chat", { limit: { output: -1 } }) }),
      updatedAt,
      ["fixture"],
    ),
  );
  assert.throws(() =>
    buildCatalog(source({ chat: text("Chat") }), updatedAt, ["missing"]),
  );
  assert.throws(() =>
    buildCatalog(
      source({ chat: text("Chat", { id: "different" }) }),
      updatedAt,
      ["fixture"],
    ),
  );
});

test("bundled catalog retains selected providers and per-provider model identities", async () => {
  const catalog = JSON.parse(
    await fs.readFile(
      new URL("../assets/models/common-models.json", import.meta.url),
      "utf8",
    ),
  );
  assert.equal(catalog.schemaVersion, 1);
  assert.equal(catalog.sourceUrl, sourceUrl);
  assert.match(catalog.sourceSha256, /^[a-f0-9]{64}$/);
  assert.deepEqual(
    catalog.providers.map((provider) => provider.id),
    [...providerIds],
  );
  for (const provider of catalog.providers) {
    assert(provider.models.length > 0);
    assert.equal(
      new Set(provider.models.map((model) => model.id)).size,
      provider.models.length,
    );
    for (const model of provider.models) {
      assert.doesNotMatch(
        `${model.id} ${model.family ?? ""}`,
        /(^|[\/_. -])(embeddings?|rerank(?:er)?)(?=$|[\/_. -])/i,
      );
      for (const field of ["context", "output"])
        assert(
          model[field] === null ||
            (Number.isSafeInteger(model[field]) && model[field] > 0),
        );
      assert(model.reasoning === null || typeof model.reasoning === "boolean");
    }
  }
  const go = catalog.providers.find(
    (provider) => provider.id === "opencode-go",
  );
  const zen = catalog.providers.find((provider) => provider.id === "opencode");
  assert.equal(go.api, "https://opencode.ai/zen/go/v1");
  assert.equal(zen.api, "https://opencode.ai/zen/v1");
  assert(go.models.some((model) => model.npm === "@ai-sdk/anthropic"));
  assert(zen.models.some((model) => model.npm === "@ai-sdk/google"));
});
