import { describe, expect, it } from "vitest";
import catalog from "../../../assets/models/common-models.json";
import { identifyModel } from "./modelCatalog";

describe("local model recognition", () => {
  it("distinguishes the actual Go model's context from its output allowance", () => {
    expect(
      identifyModel("https://opencode.ai/zen/go/v1", "glm-5.3-flash"),
    ).toMatchObject({
      providerId: "opencode-go",
      context: 1_000_000,
      output: 131_072,
      reasoning: true,
      confidence: "endpoint",
      protocol: "chat-completions",
      recommendedTokenParameter: "max_tokens",
      suggestedOutputTokens: 16_384,
    });
  });

  it("accepts a complete chat endpoint and trailing slashes without altering the model ID", () => {
    expect(
      identifyModel(
        " https://OPENCODE.AI/zen/go/v1/chat/completions/ ",
        " glm-5.3-flash ",
      ),
    ).toMatchObject({
      id: "glm-5.3-flash",
      providerId: "opencode-go",
      confidence: "endpoint",
    });
  });

  it("uses endpoint-specific metadata instead of treating the same model ID as the same API", () => {
    const id = "gpt-5.6-luna";
    expect(identifyModel("https://opencode.ai/zen/go/v1", id)).toMatchObject({
      providerId: "opencode-go",
      protocol: "responses",
      recommendedTokenParameter: null,
    });
    expect(identifyModel("https://opencode.ai/zen/v1", id)).toMatchObject({
      providerId: "opencode",
      protocol: "responses",
    });
    expect(identifyModel("https://api.openai.com/v1", id)).toMatchObject({
      providerId: "openai",
      protocol: "unknown",
      recommendedTokenParameter: "max_completion_tokens",
    });
  });

  it.each([
    ["https://api.deepseek.com/v1", "deepseek-v4-flash", "deepseek"],
    ["https://api.z.ai/api/paas/v4", "glm-5.3-flash", "zai"],
    ["https://open.bigmodel.cn/api/paas/v4", "glm-5.3-flash", "zhipuai"],
    ["https://api.moonshot.ai/v1", "kimi-k2.5", "moonshotai"],
    [
      "https://dashscope.aliyuncs.com/compatible-mode/v1",
      "qwen-plus",
      "alibaba-cn",
    ],
    ["https://api.minimax.io/anthropic/v1", "MiniMax-M2.5", "minimax"],
    ["https://api.anthropic.com/v1", "claude-sonnet-4-6", "anthropic"],
    ["https://api.x.ai/v1", "grok-4.6", "xai"],
  ])("recognizes an exact catalog entry for %s", (url, id, providerId) => {
    expect(identifyModel(url, id)).toMatchObject({
      providerId,
      id,
      confidence: "endpoint",
    });
  });

  it("keeps the native Google and Anthropic protocols distinct from chat compatibility", () => {
    expect(
      identifyModel(
        "https://generativelanguage.googleapis.com/v1beta/openai",
        "gemini-2.5-pro",
      ),
    ).toMatchObject({ protocol: "chat-completions" });
    expect(
      identifyModel(
        "https://generativelanguage.googleapis.com/v1beta",
        "gemini-2.5-pro",
      ),
    ).toMatchObject({ protocol: "google", recommendedTokenParameter: null });
    expect(
      identifyModel("https://api.anthropic.com/v1", "claude-sonnet-4-6"),
    ).toMatchObject({ protocol: "anthropic", recommendedTokenParameter: null });
    expect(
      identifyModel("https://api.openai.com/v1/responses", "gpt-4.1"),
    ).toMatchObject({ protocol: "responses", recommendedTokenParameter: null });
  });

  it("uses a reference only on a custom gateway and never assumes its token parameter", () => {
    expect(
      identifyModel("https://gateway.example/v1", "glm-5.3-flash"),
    ).toMatchObject({
      providerId: "zai",
      confidence: "reference",
      output: 131_072,
      protocol: "unknown",
      recommendedTokenParameter: null,
    });
    expect(identifyModel("http://localhost:8000/v1", "grok-4.6")).toMatchObject(
      {
        providerId: "xai",
        confidence: "reference",
        recommendedTokenParameter: null,
      },
    );
  });

  it("recognizes only known provider namespaces, without removing arbitrary versions or aliases", () => {
    expect(identifyModel("", "openai/gpt-4.1")).toMatchObject({
      id: "gpt-4.1",
      providerId: "openai",
      confidence: "reference",
    });
    expect(identifyModel("", "z-ai/glm-5.3-flash")).toMatchObject({
      id: "glm-5.3-flash",
      providerId: "zai",
      confidence: "reference",
    });
    for (const id of [
      "",
      " ",
      "test-model",
      "gpt-4.1-custom",
      "glm-5.3-flash-future",
      "arbitrary/glm-5.3-flash",
      "anthropic/gpt-4.1",
      "__proto__/gpt-4.1",
      "constructor/gpt-4.1",
    ]) {
      expect(identifyModel("https://opencode.ai/zen/go/v1", id)).toBeNull();
    }
  });

  it("does not mistake malformed, credential-bearing or lookalike addresses for a known provider", () => {
    for (const url of [
      "not a URL",
      "http://opencode.ai/zen/go/v1",
      "https://opencode.ai.example/zen/go/v1",
      "https://opencode.ai/zen/go/v10",
      "https://opencode.ai/zen/go/v1/custom",
      "https://opencode.ai/zen/go/v1?key=private",
      "https://opencode.ai/zen/go/v1#fragment",
      "https://private@opencode.ai/zen/go/v1",
    ]) {
      expect(identifyModel(url, "glm-5.3-flash")).toMatchObject({
        confidence: "reference",
        protocol: "unknown",
        recommendedTokenParameter: null,
      });
    }
  });

  it("does not recommend chat generation budgets for embedding models", () => {
    for (const id of [
      "text-embedding-3-large",
      "text-embedding-3-small",
      "text-embedding-ada-002",
      "gemini-embedding-001",
      "gemini-embedding-2",
    ]) {
      expect(identifyModel("", id)).toBeNull();
    }
  });

  it("bounds every suggestion by the model output rather than the context window", () => {
    for (const provider of catalog.providers) {
      for (const model of provider.models) {
        const match = identifyModel(provider.api ?? "", model.id);
        expect(match).not.toBeNull();
        if (match?.suggestedOutputTokens && match.output) {
          expect(match.suggestedOutputTokens).toBeLessThanOrEqual(match.output);
          expect(match.suggestedOutputTokens).toBeLessThanOrEqual(16_384);
        }
      }
    }
  });
});
