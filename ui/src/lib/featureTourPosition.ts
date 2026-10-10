type Placement = "right" | "left" | "bottom" | "top";
type Rect = { left: number; top: number; width: number; height: number };

function clamp(value: number, minimum: number, maximum: number) {
  return Math.max(minimum, Math.min(value, Math.max(minimum, maximum)));
}

export function featureTourPosition(
  anchor: Rect | null,
  card: Pick<Rect, "width" | "height">,
  viewport: { width: number; height: number },
  preferred: Placement,
) {
  const margin = 16;
  const gap = 44;
  const maxLeft = viewport.width - card.width - margin;
  const maxTop = viewport.height - card.height - margin;
  if (!anchor) {
    return {
      left: clamp((viewport.width - card.width) / 2, margin, maxLeft),
      top: clamp((viewport.height - card.height) / 2, margin, maxTop),
      spotlight: null,
      connector: null,
    };
  }

  const padding = 6;
  const left = Math.max(2, anchor.left - padding);
  const top = Math.max(2, anchor.top - padding);
  const right = Math.min(
    viewport.width - 2,
    anchor.left + anchor.width + padding,
  );
  const bottom = Math.min(
    viewport.height - 2,
    anchor.top + anchor.height + padding,
  );
  const spotlight = { left, top, width: right - left, height: bottom - top };
  const centerX = (left + right) / 2;
  const centerY = (top + bottom) / 2;
  const positions = {
    right: {
      left: right + gap,
      top: clamp(centerY - card.height / 2, margin, maxTop),
    },
    left: {
      left: left - gap - card.width,
      top: clamp(centerY - card.height / 2, margin, maxTop),
    },
    bottom: {
      left: clamp(centerX - card.width / 2, margin, maxLeft),
      top: bottom + gap,
    },
    top: {
      left: clamp(centerX - card.width / 2, margin, maxLeft),
      top: top - gap - card.height,
    },
  };
  const choices: Placement[] = [preferred, "right", "bottom", "left", "top"];
  const placement =
    choices.find((side) => {
      const position = positions[side];
      return (
        position.left >= margin &&
        position.left <= maxLeft &&
        position.top >= margin &&
        position.top <= maxTop
      );
    }) ?? preferred;
  const position = {
    left: clamp(positions[placement].left, margin, maxLeft),
    top: clamp(positions[placement].top, margin, maxTop),
  };
  const horizontal = placement === "left" || placement === "right";
  const start = horizontal
    ? {
        x: placement === "right" ? position.left : position.left + card.width,
        y: clamp(centerY, position.top + 24, position.top + card.height - 24),
      }
    : {
        x: clamp(centerX, position.left + 24, position.left + card.width - 24),
        y: placement === "bottom" ? position.top : position.top + card.height,
      };
  const end = horizontal
    ? { x: placement === "right" ? right : left, y: centerY }
    : { x: centerX, y: placement === "bottom" ? bottom : top };
  const middle = horizontal ? (start.x + end.x) / 2 : (start.y + end.y) / 2;
  const path = horizontal
    ? `M ${start.x} ${start.y} C ${middle} ${start.y}, ${middle} ${end.y}, ${end.x} ${end.y}`
    : `M ${start.x} ${start.y} C ${start.x} ${middle}, ${end.x} ${middle}, ${end.x} ${end.y}`;
  return { ...position, spotlight, connector: { path, end } };
}

export function spotlightCutout(rect: Rect): string {
  const { left, top, width, height } = rect;
  const right = left + width;
  const bottom = top + height;
  const radius = Math.min(10, width / 2, height / 2);
  return `M ${left + radius} ${top} H ${right - radius}
    Q ${right} ${top} ${right} ${top + radius} V ${bottom - radius}
    Q ${right} ${bottom} ${right - radius} ${bottom} H ${left + radius}
    Q ${left} ${bottom} ${left} ${bottom - radius} V ${top + radius}
    Q ${left} ${top} ${left + radius} ${top} Z`;
}
