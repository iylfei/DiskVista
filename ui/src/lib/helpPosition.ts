export function helpPosition(
  anchor: { left: number; top: number; bottom: number; width: number },
  tip: { width: number; height: number },
  viewport: { width: number; height: number },
) {
  const margin = 12;
  const left = Math.max(
    margin,
    Math.min(
      anchor.left + anchor.width / 2 - tip.width / 2,
      viewport.width - tip.width - margin,
    ),
  );
  const below = anchor.bottom + 8;
  const preferredTop =
    below + tip.height <= viewport.height - margin
      ? below
      : anchor.top - tip.height - 8;
  const top = Math.max(
    margin,
    Math.min(preferredTop, viewport.height - tip.height - margin),
  );
  return { left, top };
}
