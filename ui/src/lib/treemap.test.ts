import { describe, it, expect } from "vitest";
import { treemap } from "./treemap";
describe("true-area treemap", () => {
  it("preserves area for strongly unequal values", () => {
    const tiles = treemap([999, 1, 0]);
    expect(tiles).toHaveLength(2);
    expect(tiles[0].width * tiles[0].height).toBeCloseTo(9990);
    expect(tiles[1].width * tiles[1].height).toBeCloseTo(10);
  });
  it("covers the canvas without leaving its bounds", () => {
    const tiles = treemap([9, 8, 7, 6, 5, 4, 3, 2, 1]);
    expect(tiles.reduce((a, t) => a + t.width * t.height, 0)).toBeCloseTo(
      10000,
    );
    for (const t of tiles) {
      expect(t.x + t.width).toBeLessThanOrEqual(100.0001);
      expect(t.y + t.height).toBeLessThanOrEqual(100.0001);
    }
  });
  it("handles empty and zero data", () => {
    expect(treemap([])).toEqual([]);
    expect(treemap([0, -1])).toEqual([]);
  });
});
