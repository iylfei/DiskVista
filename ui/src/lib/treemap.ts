export interface Tile {
  index: number;
  x: number;
  y: number;
  width: number;
  height: number;
}
// Binary treemap: every tile retains its true area; tiny items are not artificially enlarged.
export function treemap(values: number[], width = 100, height = 100): Tile[] {
  const positive = values
    .map((value, index) => ({ value: Math.max(0, value), index }))
    .filter((x) => x.value > 0);
  const output: Tile[] = [];
  function split(
    items: typeof positive,
    x: number,
    y: number,
    w: number,
    h: number,
  ) {
    if (!items.length) return;
    if (items.length === 1) {
      output.push({ index: items[0].index, x, y, width: w, height: h });
      return;
    }
    const total = items.reduce((n, v) => n + v.value, 0);
    let sum = 0;
    let cut = 1;
    for (let i = 0; i < items.length - 1; i++) {
      sum += items[i].value;
      cut = i + 1;
      if (sum >= total / 2) break;
    }
    const ratio = sum / total;
    if (w >= h) {
      split(items.slice(0, cut), x, y, w * ratio, h);
      split(items.slice(cut), x + w * ratio, y, w * (1 - ratio), h);
    } else {
      split(items.slice(0, cut), x, y, w, h * ratio);
      split(items.slice(cut), x, y + h * ratio, w, h * (1 - ratio));
    }
  }
  split(positive, 0, 0, width, height);
  return output;
}
