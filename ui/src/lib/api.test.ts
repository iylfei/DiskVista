import { describe, it, expect } from "vitest";
import { bytes, date, riskText } from "./api";
import { analysisStatus } from "./analysisStatus";
import type { Settings } from "./types";
import { settingsWithOutputLimit } from "./settingsValidation";
describe("honest labels", () => {
  it("distinguishes unknown from zero", () => {
    expect(bytes(null)).toBe("未知");
    expect(bytes(0)).toBe("0 B");
    expect(bytes(1048576)).toBe("1 MiB");
  });
  it("never calls unknown activity unused", () => expect(date(0)).toBe("未知"));
  it("shows protected status", () =>
    expect(riskText("protected")).toBe("受保护"));
});

describe("AI status labels", () => {
  const configured = {
    enabled: true,
    automatic: true,
    metadataConsent: false,
    baseUrl: "http://localhost:8000/v1",
    model: "test-model",
  } as Settings["llm"];

  it("allows explicit analysis without background automation or its consent", () => {
    for (const automatic of [false, true]) {
      expect(
        analysisStatus({ ...configured, automatic }, false, {
          status: "complete",
        }),
      ).toMatchObject({
        label: "AI 分析当前扫描",
        action: "analyze",
        disabled: false,
      });
    }
  });

  it("distinguishes disabled and incomplete configuration", () => {
    expect(analysisStatus({ ...configured, enabled: false })).toMatchObject({
      label: "AI 未启用",
      action: "settings",
      disabled: false,
    });
    expect(analysisStatus({ ...configured, model: " " })).toMatchObject({
      label: "AI 待配置",
      action: "settings",
      disabled: false,
    });
  });

  it("waits for a selected completed scan", () => {
    expect(analysisStatus(configured).disabled).toBe(true);
    for (const status of [
      "queued",
      "scanning",
      "aggregating",
      "cancelled",
      "failed",
    ]) {
      expect(analysisStatus(configured, false, { status }).disabled).toBe(true);
    }
  });

  it("keeps an in-flight analysis visible even if settings change", () => {
    expect(
      analysisStatus({ ...configured, enabled: false }, true),
    ).toMatchObject({
      label: "AI 分析中",
      action: "analyze",
      disabled: true,
    });
  });

  it("describes the restricted batch and concise per-file result", () => {
    const status = analysisStatus(configured, false, { status: "complete" });
    expect(status.description).toContain("按目录分批");
    expect(status.description).toContain("来源未明确");
    expect(status.description).toContain("100 MiB");
    expect(status.description).toContain("删除建议与简短理由");
  });
});

describe("AI output limit save validation", () => {
  const settings = {
    llm: {
      tokenParameter: "max_tokens",
      maxOutputTokens: 1800,
      maxRequests: 12,
      model: "configured-model",
    },
    protectedPaths: ["D:\\protected"],
  } as Settings;

  it("preserves valid budgets exactly and leaves unrelated settings unchanged", () => {
    for (const value of [1, 1800, 32768, 1048576]) {
      expect(settingsWithOutputLimit(settings, String(value))).toEqual({
        ...settings,
        llm: { ...settings.llm, maxOutputTokens: value },
      });
    }
    expect(settings.llm.maxOutputTokens).toBe(1800);
  });

  it("rejects blank, fractional and out-of-range values instead of coercing or clamping", () => {
    for (const input of [
      "",
      " ",
      "0",
      "-1",
      "1.5",
      "1800.0000000000001",
      "1048577",
      "Infinity",
      "invalid",
    ]) {
      expect(() => settingsWithOutputLimit(settings, input)).toThrow(
        "单次输出上限必须是 1 到 1,048,576 之间的整数。",
      );
    }
  });

  it("validates the max_completion_tokens mode as well", () => {
    const completion = {
      ...settings,
      llm: { ...settings.llm, tokenParameter: "max_completion_tokens" },
    };
    expect(
      settingsWithOutputLimit(completion, "8192").llm.maxOutputTokens,
    ).toBe(8192);
    expect(() => settingsWithOutputLimit(completion, "")).toThrow();
  });

  it("keeps the last valid budget when none mode disables an unfinished input", () => {
    const valid = settingsWithOutputLimit(settings, "8192");
    const disabled = {
      ...valid,
      llm: { ...valid.llm, tokenParameter: "none" },
    };
    for (const input of ["", "invalid", "0", "1048577"]) {
      expect(settingsWithOutputLimit(disabled, input)).toEqual(disabled);
    }
    expect(() => settingsWithOutputLimit(valid, "")).toThrow();
  });

  it("uses 1800 for a legacy configuration when none mode is selected", () => {
    const legacy = {
      ...settings,
      llm: {
        ...settings.llm,
        tokenParameter: "none",
        maxOutputTokens: undefined,
      },
    } as unknown as Settings;
    expect(settingsWithOutputLimit(legacy, "").llm.maxOutputTokens).toBe(1800);
  });
});
