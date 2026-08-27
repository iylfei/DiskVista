import { describe, expect, it } from "vitest";
import { helpPosition } from "./helpPosition";

describe("help placement", () => {
  it("opens below the label when there is room", () => {
    expect(
      helpPosition(
        { left: 200, top: 50, bottom: 72, width: 22 },
        { width: 300, height: 100 },
        { width: 1000, height: 700 },
      ),
    ).toEqual({ left: 61, top: 80 });
  });
  it("stays in the viewport at the bottom right", () => {
    expect(
      helpPosition(
        { left: 970, top: 670, bottom: 692, width: 22 },
        { width: 300, height: 100 },
        { width: 1000, height: 700 },
      ),
    ).toEqual({ left: 688, top: 562 });
  });
  it("stays readable in a small viewport", () => {
    expect(
      helpPosition(
        { left: 0, top: 12, bottom: 34, width: 22 },
        { width: 296, height: 210 },
        { width: 320, height: 240 },
      ),
    ).toEqual({ left: 12, top: 12 });
  });
});
