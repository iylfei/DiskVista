import { useEffect, useState } from "react";
import type { Rule, Settings } from "../lib/types";
import { api } from "../lib/api";
import HelpTip from "../components/HelpTip";
import { helpText } from "../lib/helpText";
export default function RulesPage({
  settings,
  onSave,
  onError,
}: {
  settings: Settings;
  onSave: (s: Settings) => void;
  onError: (e: unknown) => void;
}) {
  const [pack, setPack] = useState<{
    version: string;
    rules: Rule[];
    community: unknown;
  } | null>(null);
  useEffect(() => {
    api<typeof pack>("rules").then(setPack).catch(onError);
  }, [settings.communityEnabled, onError]);
  return (
    <>
      <section className="panel">
        <div className="section-heading">
          <div>
            <h2>重要内容始终受保护</h2>
            <p>规则和 AI 只能给出建议，不能解除系统保护或你设置的保护。</p>
          </div>
          <span className="badge protected">始终开启</span>
        </div>
        <label className="check-line" htmlFor="community-rules">
          <input
            id="community-rules"
            aria-label="启用社区规则（Winapp2）"
            type="checkbox"
            checked={settings.communityEnabled}
            onChange={async (e) => {
              const next = { ...settings, communityEnabled: e.target.checked };
              try {
                await api("save_settings", { settings: next, key: null });
                onSave(next);
              } catch (error) {
                onError(error);
              }
            }}
          />
          <span>
            启用社区规则（Winapp2）
            <HelpTip label="社区规则" text={helpText.community} />
          </span>
        </label>
        <p className="muted">
          默认关闭，可帮助识别更多应用文件。仅采用软件能安全理解的规则；切换后请重新扫描。
        </p>
        <details>
          <summary>社区来源、版本与转换报告</summary>
          <pre className="metadata-preview">
            {JSON.stringify(pack?.community ?? {}, null, 2)}
          </pre>
        </details>
      </section>
      <section className="panel">
        <h2>
          当前识别规则
          <HelpTip label="识别规则" text={helpText.rules} />
        </h2>
        <p className="muted">{pack?.version}</p>
        {pack?.rules.map((r) => (
          <details key={r.id} className="rule-item">
            <summary>
              <strong>{r.name}</strong>
              <span className="badge neutral">
                {r.community ? "社区规则" : "内置规则"}
              </span>
            </summary>
            <code className="path-text">{r.root}</code>
            <p>{r.purpose}</p>
            <p>影响：{r.consequence}</p>
            <p>恢复：{r.recovery}</p>
            {r.warning && <p className="warning-text">{r.warning}</p>}
            <small>
              {r.id} · 至少 {r.ageDays} 天没有变化时才考虑推荐清理
            </small>
          </details>
        ))}
      </section>
    </>
  );
}
