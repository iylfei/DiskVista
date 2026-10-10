import { useId, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { ArrowLeft, ArrowRight, Check } from "lucide-react";
import { featureTourSteps } from "../lib/featureTour";
import { translateText } from "../i18n/translate";
import {
  featureTourPosition,
  spotlightCutout,
} from "../lib/featureTourPosition";
import "./feature-tour.css";

type Layout = ReturnType<typeof featureTourPosition> & {
  viewport: { width: number; height: number };
};

export default function FeatureTour({ onFinish }: { onFinish: () => void }) {
  const [index, setIndex] = useState(0);
  const [layout, setLayout] = useState<Layout | null>(null);
  const dialog = useRef<HTMLDialogElement>(null);
  const card = useRef<HTMLElement>(null);
  const next = useRef<HTMLButtonElement>(null);
  const titleId = useId();
  const descriptionId = useId();
  const step = featureTourSteps[index];
  const last = index === featureTourSteps.length - 1;

  useLayoutEffect(() => {
    const element = dialog.current;
    element?.showModal();
    return () => element?.close();
  }, []);

  useLayoutEffect(() => {
    const target = document.querySelector<HTMLElement>(
      `[data-tour="${step.target}"]`,
    );
    target?.setAttribute("data-tour-current", "");
    target?.scrollIntoView({ block: "nearest", inline: "nearest" });
    let frame = 0;
    const place = () => {
      if (!card.current) return;
      const bounds = target?.getBoundingClientRect();
      const viewport = { width: window.innerWidth, height: window.innerHeight };
      const visible =
        bounds &&
        bounds.width > 0 &&
        bounds.height > 0 &&
        bounds.right > 0 &&
        bounds.bottom > 0 &&
        bounds.left < viewport.width &&
        bounds.top < viewport.height;
      setLayout({
        ...featureTourPosition(
          visible ? bounds : null,
          card.current.getBoundingClientRect(),
          viewport,
          step.placement,
        ),
        viewport,
      });
    };
    const schedule = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(place);
    };
    place();
    const observer = new ResizeObserver(schedule);
    if (target) {
      observer.observe(target);
      const container = target.closest("aside, header");
      if (container) observer.observe(container);
    }
    if (card.current) observer.observe(card.current);
    window.addEventListener("resize", schedule);
    window.addEventListener("scroll", schedule, true);
    return () => {
      target?.removeAttribute("data-tour-current");
      observer.disconnect();
      cancelAnimationFrame(frame);
      window.removeEventListener("resize", schedule);
      window.removeEventListener("scroll", schedule, true);
    };
  }, [step]);

  const positioned = layout !== null;
  useLayoutEffect(() => {
    if (positioned) next.current?.focus({ preventScroll: true });
  }, [index, positioned]);

  const spotlight = layout?.spotlight;
  const viewport = layout?.viewport;
  return createPortal(
    <dialog
      ref={dialog}
      className="feature-tour"
      aria-labelledby={titleId}
      aria-describedby={descriptionId}
      onCancel={(event) => {
        event.preventDefault();
        onFinish();
      }}
    >
      {viewport && (
        <svg
          className="feature-tour-overlay"
          aria-hidden="true"
          width={viewport.width}
          height={viewport.height}
          viewBox={`0 0 ${viewport.width} ${viewport.height}`}
        >
          <path
            className="feature-tour-shade"
            fillRule="evenodd"
            d={`M 0 0 H ${viewport.width} V ${viewport.height} H 0 Z ${spotlight ? spotlightCutout(spotlight) : ""}`}
          />
          {spotlight && (
            <rect
              className="feature-tour-spotlight"
              x={spotlight.left}
              y={spotlight.top}
              width={spotlight.width}
              height={spotlight.height}
              rx={10}
            />
          )}
          {layout.connector && (
            <g className="feature-tour-connector">
              <path d={layout.connector.path} />
              <circle
                cx={layout.connector.end.x}
                cy={layout.connector.end.y}
                r={4}
              />
            </g>
          )}
        </svg>
      )}
      <section
        ref={card}
        className="feature-tour-card"
        style={{
          left: layout?.left ?? 16,
          top: layout?.top ?? 16,
          visibility: layout ? "visible" : "hidden",
        }}
      >
        <header className="feature-tour-header">
          <span>功能介绍</span>
          <button type="button" className="text-button" onClick={onFinish}>
            跳过引导
          </button>
        </header>
        <div className="feature-tour-heading">
          <span className="feature-tour-number" aria-hidden="true">
            {index + 1}
          </span>
          <h2 id={titleId}>{step.title}</h2>
        </div>
        <p id={descriptionId}>{step.description}</p>
        <nav className="feature-tour-steps" aria-label="功能介绍进度">
          {featureTourSteps.map((item, position) => (
            <button
              key={item.target}
              type="button"
              aria-label={`查看第 ${position + 1} 步：${translateText(item.title)}`}
              aria-current={position === index ? "step" : undefined}
              className={position < index ? "visited" : ""}
              onClick={() => setIndex(position)}
            >
              {position + 1}
            </button>
          ))}
        </nav>
        <footer className="feature-tour-actions">
          <button
            type="button"
            disabled={index === 0}
            onClick={() => setIndex((current) => Math.max(0, current - 1))}
          >
            <ArrowLeft size={15} aria-hidden="true" />
            上一步
          </button>
          <button
            ref={next}
            type="button"
            className="primary"
            onClick={() =>
              last
                ? onFinish()
                : setIndex((current) =>
                    Math.min(featureTourSteps.length - 1, current + 1),
                  )
            }
          >
            {last ? "开始使用" : "下一步"}
            {last ? (
              <Check size={15} aria-hidden="true" />
            ) : (
              <ArrowRight size={15} aria-hidden="true" />
            )}
          </button>
        </footer>
      </section>
    </dialog>,
    document.body,
  );
}
