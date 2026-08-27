import { describe, it, expect } from "vitest";
import { bytes, date, riskText } from "./api";
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
