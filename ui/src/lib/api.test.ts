import { describe, it, expect } from "vitest";
import { bytes, date, riskText } from "./api";
import { analysisStatus } from "./analysisStatus";
import type { Settings } from "./types";
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
});
