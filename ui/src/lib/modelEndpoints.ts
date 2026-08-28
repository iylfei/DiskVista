export type ModelProtocol =
  | "chat-completions"
  | "responses"
  | "anthropic"
  | "google"
  | "unknown";

export type OutputTokenParameter = "max_tokens" | "max_completion_tokens";

interface ProviderRoute {
  id: string;
  api: string | null;
}

interface EndpointRoute {
  providerId: string;
  api: string;
  protocol?: ModelProtocol;
  parameter?: OutputTokenParameter;
}

export interface ModelEndpoint {
  providerId: string;
  protocol: ModelProtocol | null;
  parameter: OutputTokenParameter | null;
}

const additionalRoutes: EndpointRoute[] = [
  {
    providerId: "openai",
    api: "https://api.openai.com/v1",
    protocol: "unknown",
    parameter: "max_completion_tokens",
  },
  {
    providerId: "anthropic",
    api: "https://api.anthropic.com/v1",
    protocol: "anthropic",
  },
  {
    providerId: "google",
    api: "https://generativelanguage.googleapis.com/v1beta",
    protocol: "google",
  },
  {
    providerId: "google",
    api: "https://generativelanguage.googleapis.com/v1beta/openai",
    protocol: "chat-completions",
  },
  {
    providerId: "deepseek",
    api: "https://api.deepseek.com/v1",
  },
  {
    providerId: "xai",
    api: "https://api.x.ai/v1",
    protocol: "unknown",
  },
];

function parseEndpoint(value: string): URL | null {
  try {
    const url = new URL(value.trim());
    if (
      url.protocol !== "https:" ||
      url.username ||
      url.password ||
      url.search ||
      url.hash
    ) {
      return null;
    }
    return url;
  } catch {
    return null;
  }
}

export function matchModelEndpoints(
  baseUrl: string,
  providers: readonly ProviderRoute[],
): ModelEndpoint[] {
  const url = parseEndpoint(baseUrl);
  if (!url) return [];
  const pathname = url.pathname.replace(/\/+$/, "");
  const routes: EndpointRoute[] = [
    ...providers.flatMap((provider) =>
      provider.api ? [{ providerId: provider.id, api: provider.api }] : [],
    ),
    ...additionalRoutes,
  ];
  const matches = routes.flatMap((route) => {
    const candidate = parseEndpoint(route.api);
    if (!candidate || candidate.origin !== url.origin) return [];
    const base = candidate.pathname.replace(/\/+$/, "");
    const suffix = pathname.slice(base.length);
    if (
      !pathname.startsWith(base) ||
      !["", "/chat/completions", "/responses", "/messages"].includes(suffix)
    ) {
      return [];
    }
    const explicitProtocol =
      suffix === "/responses"
        ? "responses"
        : suffix === "/messages"
          ? "anthropic"
          : null;
    return [
      {
        providerId: route.providerId,
        protocol: explicitProtocol ?? route.protocol ?? null,
        parameter: explicitProtocol ? null : (route.parameter ?? null),
        specificity: base.length,
      },
    ];
  });
  const longest = Math.max(-1, ...matches.map((match) => match.specificity));
  return matches.filter((match) => match.specificity === longest);
}

export function describeModelEndpoint(
  endpoint: ModelEndpoint,
  npm: string | null,
): { protocol: ModelProtocol; parameter: OutputTokenParameter | null } {
  if (endpoint.protocol) {
    return { protocol: endpoint.protocol, parameter: endpoint.parameter };
  }
  if (npm === "@ai-sdk/openai-compatible") {
    return { protocol: "chat-completions", parameter: "max_tokens" };
  }
  if (
    npm === "@ai-sdk/openai" &&
    ["opencode", "opencode-go"].includes(endpoint.providerId)
  ) {
    return { protocol: "responses", parameter: null };
  }
  if (npm === "@ai-sdk/anthropic") {
    return { protocol: "anthropic", parameter: null };
  }
  if (npm === "@ai-sdk/google") {
    return { protocol: "google", parameter: null };
  }
  return { protocol: "unknown", parameter: null };
}
