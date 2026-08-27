import type { Scan } from "../lib/types";
import { date, statusText } from "../lib/api";
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
  if (scans.length < 2) return null;
  return (
    <label className="snapshot-picker" htmlFor="scan-snapshot">
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
            {date(s.started)} · {s.root} · {statusText(s.status)}
          </option>
        ))}
      </select>
    </label>
  );
}
