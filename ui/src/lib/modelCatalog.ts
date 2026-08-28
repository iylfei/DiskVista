import catalog from "../../../assets/models/common-models.json";
import { MAX_OUTPUT_TOKENS_LIMIT } from "./settingsValidation";
import {
  describeModelEndpoint,
  matchModelEndpoints,
  type ModelEndpoint,
  type ModelProtocol,
  type OutputTokenParameter,
} from "./modelEndpoints";

interface CatalogModel {
  id: string;
  name: string;
  context: number | null;
  output: number | null;
  reasoning: boolean | null;
  npm: string | null;
  status: string | null;
}

interface CatalogProvider {
  id: string;
  name: string;
  api: string | null;
  npm: string | null;
  models: CatalogModel[];
}

interface CatalogEntry {
  provider: CatalogProvider;
  model: CatalogModel;
}

export interface ModelMatch {
  providerId: string;
  providerName: string;
  id: string;
  name: string;
  context: number | null;
  output: number | null;
  reasoning: boolean | null;
  confidence: "endpoint" | "reference";
  protocol: ModelProtocol;
  recommendedTokenParameter: OutputTokenParameter | null;
  suggestedOutputTokens: number | null;
  sourceUrl: string;
  updatedAt: string;
  status: string | null;
}

const providers: CatalogProvider[] = catalog.providers;
const modelIndex = new Map<string, CatalogEntry[]>();
for (const provider of providers) {
  for (const model of provider.models) {
    const existing = modelIndex.get(model.id) ?? [];
    existing.push({ provider, model });
    modelIndex.set(model.id, existing);
  }
}

const referenceProviders = [
  "openai",
  "anthropic",
  "google",
  "deepseek",
  "zai",
  "zhipuai",
  "moonshotai",
  "moonshotai-cn",
  "alibaba",
  "alibaba-cn",
  "minimax",
  "minimax-cn",
  "xai",
];

const namespaces = new Map<string, readonly string[]>(
  Object.entries({
    openai: ["openai"],
    anthropic: ["anthropic"],
    google: ["google"],
    deepseek: ["deepseek"],
    zai: ["zai", "zhipuai"],
    "z-ai": ["zai", "zhipuai"],
    zhipuai: ["zhipuai", "zai"],
    moonshotai: ["moonshotai", "moonshotai-cn"],
    qwen: ["alibaba", "alibaba-cn"],
    alibaba: ["alibaba", "alibaba-cn"],
    minimax: ["minimax", "minimax-cn"],
    xai: ["xai"],
    "x-ai": ["xai"],
  }),
);

function referenceEntry(entries: readonly CatalogEntry[]): CatalogEntry | null {
  for (const providerId of referenceProviders) {
    const match = entries.find((entry) => entry.provider.id === providerId);
    if (match) return match;
  }
  return entries[0] ?? null;
}

function matchResult(
  entry: CatalogEntry,
  endpoint: ModelEndpoint | null,
): ModelMatch {
  const { provider, model } = entry;
  const description = endpoint
    ? describeModelEndpoint(endpoint, model.npm ?? provider.npm)
    : { protocol: "unknown" as const, parameter: null };
  const output = model.output && model.output > 0 ? model.output : null;
  return {
    providerId: provider.id,
    providerName: provider.name,
    id: model.id,
    name: model.name,
    context: model.context && model.context > 0 ? model.context : null,
    output,
    reasoning: model.reasoning,
    confidence: endpoint ? "endpoint" : "reference",
    protocol: description.protocol,
    recommendedTokenParameter: description.parameter,
    suggestedOutputTokens: output
      ? Math.min(
          output,
          model.reasoning ? 16_384 : 8_192,
          MAX_OUTPUT_TOKENS_LIMIT,
        )
      : null,
    sourceUrl: catalog.sourceUrl,
    updatedAt: catalog.updatedAt,
    status: model.status,
  };
}

export function identifyModel(
  baseUrl: string,
  modelId: string,
): ModelMatch | null {
  const id = modelId.trim();
  if (!id) return null;
  const exact = modelIndex.get(id) ?? [];
  for (const endpoint of matchModelEndpoints(baseUrl, providers)) {
    const entry = exact.find(
      (candidate) => candidate.provider.id === endpoint.providerId,
    );
    if (entry) return matchResult(entry, endpoint);
  }
  const reference = referenceEntry(exact);
  if (reference) return matchResult(reference, null);

  const slash = id.indexOf("/");
  if (slash < 1) return null;
  const providerIds = namespaces.get(id.slice(0, slash));
  if (!providerIds) return null;
  const canonical = modelIndex.get(id.slice(slash + 1)) ?? [];
  for (const providerId of providerIds) {
    const entry = canonical.find(
      (candidate) => candidate.provider.id === providerId,
    );
    if (entry) return matchResult(entry, null);
  }
  return null;
}
