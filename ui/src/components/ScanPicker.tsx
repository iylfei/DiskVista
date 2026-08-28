import type { Scan } from "../lib/types";
import { History } from "lucide-react";
import { dateTime, statusText } from "../lib/api";
import HelpTip from "./HelpTip";
import { helpText } from "../lib/helpText";
export default function ScanPicker({
  scans,
  current,
  disabled,
  onSelect,
}: {
  scans: Scan[];
  current: string;
  disabled: boolean;
  onSelect: (scan: Scan) => void;
}) {
  if (!scans.length) return null;
  return (
    <label className="snapshot-picker" htmlFor="scan-snapshot">
      <History size={17} aria-hidden="true" />
      <span>
        扫描记录
        <HelpTip label="扫描记录" text={helpText.snapshot} />
      </span>
      <select
        id="scan-snapshot"
        aria-label="扫描记录"
        value={current}
        disabled={disabled}
        onChange={(e) => {
          const scan = scans.find((s) => s.id === e.target.value);
          if (scan) onSelect(scan);
        }}
      >
        {scans.map((s) => (
          <option key={s.id} value={s.id}>
            {dateTime(s.started)} · {s.root} · {statusText(s.status)}
          </option>
        ))}
      </select>
    </label>
  );
}
