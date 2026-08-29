import type { Scan } from "../lib/types";
import { bytes } from "../lib/api";
import HelpTip from "./HelpTip";
import { helpText } from "../lib/helpText";
import { useState } from "react";
import ScanIssues from "./ScanIssues";
import { localeName } from "../i18n/locale";

export default function ScanSummary({ scan }: { scan: Scan }) {
  const [showIssues, setShowIssues] = useState(false);
  return (
    <div className="scan-summary">
      <span>
        文件总大小 <strong>{bytes(scan.logicalBytes)}</strong>
        <HelpTip label="扫描结果中的文件大小" text={helpText.logicalSize} />
      </span>
      <span>{scan.files.toLocaleString(localeName())} 个文件</span>
      {scan.issues > 0 && (
        <span className="scan-issues">
          {scan.issues} 处未能扫描
          <HelpTip label="未能扫描的原因" text={helpText.incomplete} />
          <button className="text-button" onClick={() => setShowIssues(true)}>
            查看原因
          </button>
        </span>
      )}
      {showIssues && (
        <ScanIssues scanId={scan.id} onClose={() => setShowIssues(false)} />
      )}
    </div>
  );
}
