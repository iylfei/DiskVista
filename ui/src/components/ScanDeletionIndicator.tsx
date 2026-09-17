import { LoaderCircle } from "lucide-react";
import { dateTime } from "../lib/api";
import type { Scan } from "../lib/types";
import HelpTip from "./HelpTip";
import "./scan-deletion-indicator.css";

export default function ScanDeletionIndicator({ scan }: { scan: Scan }) {
  return (
    <span className="scan-deletion-indicator">
      <HelpTip
        variant="status"
        label="正在删除扫描记录…"
        text={`${dateTime(scan.started)}\n${scan.root}`}
        icon={<LoaderCircle size={14} strokeWidth={1.6} aria-hidden="true" />}
      />
    </span>
  );
}
