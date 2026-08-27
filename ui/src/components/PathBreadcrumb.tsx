import { useEffect, useRef } from "react";
import { ArrowUp, ChevronRight } from "lucide-react";
import { breadcrumbs } from "../lib/breadcrumbs";

export default function PathBreadcrumb({
  root,
  path,
  onNavigate,
}: {
  root: string;
  path: string;
  onNavigate: (path: string) => void;
}) {
  const parts = breadcrumbs(root, path);
  const trail = useRef<HTMLOListElement>(null);
  useEffect(() => {
    if (trail.current) trail.current.scrollLeft = trail.current.scrollWidth;
  }, [path]);
  return (
    <nav className="breadcrumb" aria-label="当前文件夹路径">
      <button
        className="icon-button"
        disabled={parts.length < 2}
        aria-label="上一级"
        onClick={() => onNavigate(parts[parts.length - 2].path)}
      >
        <ArrowUp size={17} />
      </button>
      <ol ref={trail}>
        {parts.map((part, index) => (
          <li key={part.path}>
            {index > 0 && <ChevronRight size={14} aria-hidden="true" />}
            <button
              title={part.path}
              aria-current={index === parts.length - 1 ? "location" : undefined}
              onClick={() => onNavigate(part.path)}
            >
              {part.label}
            </button>
          </li>
        ))}
      </ol>
    </nav>
  );
}
