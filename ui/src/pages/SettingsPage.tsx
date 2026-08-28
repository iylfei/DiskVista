import { useState } from "react";
import type { Settings } from "../lib/types";
import { api } from "../lib/api";
import HelpTip from "../components/HelpTip";
import ModelCapabilities from "../components/ModelCapabilities";
import { helpText } from "../lib/helpText";
import {
  DEFAULT_MAX_OUTPUT_TOKENS,
  MAX_OUTPUT_TOKENS_LIMIT,
  OUTPUT_TOKENS_ERROR,
  parseMaxOutputTokens,
  settingsWithOutputLimit,
} from "../lib/settingsValidation";
export default function SettingsPage({
  settings,
  hasKey,
  onSave,
  onError,
}: {
  settings: Settings;
  hasKey: boolean;
  onSave: (s: Settings) => void;
  onError: (e: unknown) => void;
}) {
  const [draft, setDraft] = useState<Settings>(() => {
    const value = structuredClone(settings);
    value.llm.maxOutputTokens ??= DEFAULT_MAX_OUTPUT_TOKENS;
    return value;
  });
  const [outputTokens, setOutputTokens] = useState(
    String(settings.llm.maxOutputTokens ?? DEFAULT_MAX_OUTPUT_TOKENS),
  );
  const [key, setKey] = useState("");
  const [pending, setPending] = useState<"save" | "test" | null>(null);
  const busy = pending !== null;
  const [note, setNote] = useState("");
  const [testError, setTestError] = useState("");
  const outputLimitDisabled = draft.llm.tokenParameter === "none";
  const outputLimitInvalid =
    !outputLimitDisabled && parseMaxOutputTokens(outputTokens) === null;
  const llm = (change: Partial<Settings["llm"]>) =>
    setDraft((s) => ({ ...s, llm: { ...s.llm, ...change } }));
  function acceptSaved(saved: Settings) {
    setDraft(saved);
    setOutputTokens(
      String(saved.llm.maxOutputTokens ?? DEFAULT_MAX_OUTPUT_TOKENS),
    );
    onSave(saved);
  }
  async function save() {
    setPending("save");
    setNote("");
    setTestError("");
    try {
      const saved = await api<Settings>("save_settings", {
        settings: settingsWithOutputLimit(draft, outputTokens),
        key: key || null,
      });
      setKey("");
      acceptSaved(saved);
      setNote(
        (draft.llm.metadataConsent && !saved.llm.metadataConsent) ||
          (draft.llm.historyReferenceEnabled &&
            !saved.llm.historyReferenceEnabled)
          ? "设置已保存。服务地址已更改，自动发送与历史参考授权已重置，请重新检查并授权。"
          : "设置已保存；密钥只存入 Windows 凭据管理器。",
      );
    } catch (e) {
      onError(e);
    } finally {
      setPending(null);
    }
  }
  async function test() {
    setPending("test");
    setNote("");
    setTestError("");
    try {
      const result = await api<string>("test_connection", {
        settings: draft.llm,
        key: key || null,
      });
      setNote(result);
    } catch (e) {
      setTestError(e instanceof Error ? e.message : String(e));
    } finally {
      setPending(null);
    }
  }
  return (
    <div className="settings-content">
      <section className="panel">
        <h2>扫描设置</h2>
        <label className="check-line" htmlFor="enhanced-scan">
          <input
            id="enhanced-scan"
            aria-label="启用增强扫描（USN）"
            type="checkbox"
            checked={draft.enhancedScan}
            onChange={(e) =>
              setDraft({ ...draft, enhancedScan: e.target.checked })
            }
          />
          <span>
            启用增强扫描（USN）
            <HelpTip label="增强扫描" text={helpText.enhancedScan} />
          </span>
        </label>
        <p className="muted">
          默认关闭，可能需要管理员许可。无法使用时会自动改为完整扫描，不修改系统设置。
        </p>
        <label htmlFor="scan-retention">
          <span>扫描记录保留数量（每个扫描位置）</span>
          <select
            id="scan-retention"
            value={draft.scanRetention ?? 0}
            onChange={(e) =>
              setDraft({ ...draft, scanRetention: Number(e.target.value) })
            }
          >
            <option value={0}>保留全部</option>
            <option value={5}>最近 5 次</option>
            <option value={10}>最近 10 次</option>
            <option value={30}>最近 30 次</option>
            <option value={100}>最近 100 次</option>
          </select>
        </label>
        <p className="muted">
          保存后，下次启动时移除较早的扫描记录；增量扫描需要的记录会额外保留。不删除原文件，也不移除回收操作历史。
        </p>
      </section>
      <section className="panel">
        <div className="section-heading">
          <div>
            <h2>可选 AI 分析</h2>
            <p>
              让 AI 提供删除建议与简短理由。是否启用由你决定，它不会操作文件。
            </p>
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={draft.llm.enabled}
              onChange={(e) => llm({ enabled: e.target.checked })}
            />
            启用 AI
          </label>
        </div>
        {draft.llm.enabled &&
          draft.llm.automatic &&
          !draft.llm.metadataConsent && (
            <p className="notice ai-setup-notice" role="status">
              自动分析待授权：允许发送基本信息并保存后，将在下次扫描完成时分析符合条件的项目。
            </p>
          )}
        <div className="form-grid">
          <label htmlFor="ai-api-url">
            <span>
              兼容 API 地址
              <HelpTip label="API 地址" text={helpText.apiAddress} />
            </span>
            <input
              id="ai-api-url"
              aria-label="兼容 API 地址"
              placeholder="https://your-provider.example/v1"
              value={draft.llm.baseUrl}
              onChange={(e) => llm({ baseUrl: e.target.value })}
            />
            <small>使用服务商提供的接口地址，或本机运行的 AI 服务地址。</small>
          </label>
          <label htmlFor="ai-model">
            <span>
              模型 ID
              <HelpTip label="模型 ID" text={helpText.model} />
            </span>
            <input
              id="ai-model"
              aria-label="模型 ID"
              placeholder="手动填写服务提供的模型名"
              value={draft.llm.model}
              onChange={(e) => llm({ model: e.target.value })}
            />
          </label>
          <ModelCapabilities
            baseUrl={draft.llm.baseUrl}
            modelId={draft.llm.model}
            outputTokens={outputTokens}
            tokenParameter={draft.llm.tokenParameter}
            disabled={busy}
            onUseSuggestion={(value) => {
              setOutputTokens(String(value.maxOutputTokens));
              llm(value);
            }}
          />
          <label htmlFor="ai-key">
            <span>
              API 密钥
              <HelpTip label="API 密钥" text={helpText.apiKey} />
            </span>
            <input
              id="ai-key"
              aria-label="API 密钥"
              type="password"
              autoComplete="new-password"
              value={key}
              placeholder={
                hasKey ? "已安全保存，留空保持不变" : "本机免密服务可留空"
              }
              onChange={(e) => setKey(e.target.value)}
            />
            <small>更改地址会清除旧密钥，请重新填写。</small>
          </label>
          <label htmlFor="ai-format">
            <span>
              输出兼容模式
              <HelpTip label="输出兼容模式" text={helpText.outputFormat} />
            </span>
            <select
              id="ai-format"
              aria-label="输出兼容模式"
              value={draft.llm.format}
              onChange={(e) => llm({ format: e.target.value })}
            >
              <option value="auto">自动（推荐）</option>
              <option value="schema">严格 JSON Schema</option>
              <option value="json">JSON 模式</option>
              <option value="text">JSON 文本</option>
            </select>
          </label>
          <label htmlFor="ai-token-parameter">
            <span>
              输出长度参数
              <HelpTip label="输出长度参数" text={helpText.tokenParameter} />
            </span>
            <select
              id="ai-token-parameter"
              aria-label="输出长度参数"
              value={draft.llm.tokenParameter}
              onChange={(e) => llm({ tokenParameter: e.target.value })}
            >
              <option value="max_tokens">max_tokens</option>
              <option value="max_completion_tokens">
                max_completion_tokens
              </option>
              <option value="none">不发送长度参数</option>
            </select>
          </label>
          <label htmlFor="ai-output-token-limit">
            <span>
              单次输出上限（token）
              <HelpTip label="单次输出上限" text={helpText.maxOutputTokens} />
            </span>
            <input
              id="ai-output-token-limit"
              aria-label="单次输出上限（token）"
              aria-describedby="ai-output-token-help"
              aria-invalid={outputLimitInvalid || undefined}
              type="number"
              inputMode="numeric"
              min={1}
              max={MAX_OUTPUT_TOKENS_LIMIT}
              step={1}
              disabled={outputLimitDisabled}
              value={outputTokens}
              onChange={(e) => {
                setOutputTokens(e.target.value);
                const value = parseMaxOutputTokens(e.target.value);
                if (value !== null) llm({ maxOutputTokens: value });
              }}
            />
            <small id="ai-output-token-help">
              {outputLimitDisabled && "当前不发送此数值。"}
              这是单次请求的生成预算；服务支持的最大输出需单独确认，1M
              上下文不等于输出上限。
            </small>
            {outputLimitInvalid && (
              <small className="warning-text" role="alert">
                {OUTPUT_TOKENS_ERROR}
              </small>
            )}
          </label>
          <label htmlFor="ai-timeout">
            <span>
              请求超时（秒）
              <HelpTip label="请求超时" text={helpText.timeout} />
            </span>
            <input
              id="ai-timeout"
              aria-label="请求超时（秒）"
              type="number"
              min={5}
              max={300}
              value={draft.llm.timeoutSeconds}
              onChange={(e) => llm({ timeoutSeconds: Number(e.target.value) })}
            />
          </label>
          <label htmlFor="ai-minimum-size">
            <span>
              自动分析门槛（MiB）
              <HelpTip label="自动分析门槛" text={helpText.minimumSize} />
            </span>
            <input
              id="ai-minimum-size"
              aria-label="自动分析门槛（MiB）"
              type="number"
              min={100}
              value={draft.llm.minimumBytes / 1048576}
              onChange={(e) =>
                llm({ minimumBytes: Number(e.target.value) * 1048576 })
              }
            />
            <small>最低按 100 MiB 生效，只分析严格大于门槛的文件。</small>
          </label>
          <label htmlFor="ai-request-limit">
            <span>
              每次扫描请求上限
              <HelpTip label="请求上限" text={helpText.requestLimit} />
            </span>
            <input
              id="ai-request-limit"
              aria-label="每次扫描请求上限"
              type="number"
              min={1}
              max={100}
              value={draft.llm.maxRequests}
              onChange={(e) => llm({ maxRequests: Number(e.target.value) })}
            />
          </label>
          <label htmlFor="ai-concurrency">
            <span>
              最大并发
              <HelpTip label="最大并发" text={helpText.concurrency} />
            </span>
            <input
              id="ai-concurrency"
              aria-label="最大并发"
              type="number"
              min={1}
              max={2}
              value={draft.llm.concurrency}
              onChange={(e) => llm({ concurrency: Number(e.target.value) })}
            />
          </label>
        </div>
        <p className="muted">
          重试也计入请求上限。没有服务商提供的用量或价格时，不估算费用。
        </p>
        <div className="privacy-note">
          <strong>哪些信息会发送给 AI</strong>
          <p>
            默认只发送部分文件名、文件夹结构、大小、时间和判断依据，不包含文件正文。用户名会被替换，但文件名仍可能透露隐私。读取正文需要你另行同意，密码、钱包等敏感文件不允许读取正文。
          </p>
        </div>
        <label className="check-line">
          <input
            type="checkbox"
            checked={draft.llm.automatic}
            onChange={(e) => llm({ automatic: e.target.checked })}
          />
          扫描完成后，自动分析来源未明确的大文件
        </label>
        <p className="muted">
          只分析超过门槛的文件，不包含目录或已明确所属应用的文件。按目录分批分析，每个文件给出删除建议与简短理由。
        </p>
        <label className="check-line" htmlFor="ai-metadata-consent">
          <input
            id="ai-metadata-consent"
            aria-label="允许自动发送上述脱敏元数据（不含文件正文）"
            type="checkbox"
            checked={draft.llm.metadataConsent}
            onChange={(e) => llm({ metadataConsent: e.target.checked })}
          />
          <span>
            我允许自动模式发送上述基本信息（脱敏元数据，不含文件正文）
            <HelpTip label="脱敏元数据" text={helpText.metadata} />
          </span>
        </label>
        <label className="check-line" htmlFor="ai-history-reference">
          <input
            id="ai-history-reference"
            type="checkbox"
            checked={draft.llm.historyReferenceEnabled ?? false}
            onChange={(e) => llm({ historyReferenceEnabled: e.target.checked })}
          />
          <span>
            允许分析时参考并发送相关回收历史
            <HelpTip label="回收历史参考" text={helpText.historyReference} />
          </span>
        </label>
        <p className="muted">
          仅从本软件近 180 天成功回收的记录中挑选，每个文件最多 6
          条。历史只作参考，不扩大分析范围。会发送脱敏路径、大小、回收时间及已有的应用归属，不包含文件正文；失败或取消的操作不参与。关闭后不再发送，更改服务地址后需重新开启。
        </p>
        <div className="actions">
          <button
            disabled={busy}
            onClick={test}
            title="仅发送固定轻量测试消息（128 token 输出预算），不发送文件信息；不会验证单次输出上限"
          >
            {pending === "test" ? "正在测试…" : "测试连接"}
          </button>
          <button disabled={busy} className="primary" onClick={save}>
            保存设置
          </button>
        </div>
        {note && (
          <p className="success-text" role="status">
            {note}
          </p>
        )}
        {testError && (
          <p className="connection-error" role="alert">
            {testError}
          </p>
        )}
      </section>
      <section className="panel">
        <h2>不想清理或发送的内容</h2>
        <p>
          系统保护始终开启。你可以在文件详情中保护某个位置、忽略项目或禁止发送给
          AI，在这里取消这些设置。
        </p>
        {(["protectedPaths", "ignoredPaths", "excludedLlmPaths"] as const).map(
          (field, i) => (
            <div key={field}>
              <h3>{["用户保护路径", "忽略的项目", "禁止发送 AI 的路径"][i]}</h3>
              {draft[field].length === 0 ? (
                <p className="muted">暂无</p>
              ) : (
                draft[field].map((path) => (
                  <div className="path-setting" key={path}>
                    <code>{path}</code>
                    <button
                      onClick={() =>
                        setDraft({
                          ...draft,
                          [field]: draft[field].filter((p) => p !== path),
                        })
                      }
                    >
                      移除
                    </button>
                  </div>
                ))
              )}
            </div>
          ),
        )}
        <div className="actions">
          <button
            disabled={busy}
            onClick={async () => {
              setPending("save");
              setNote("");
              setTestError("");
              try {
                const saved = await api<Settings>("save_settings", {
                  settings: {
                    ...settings,
                    protectedPaths: draft.protectedPaths,
                    ignoredPaths: draft.ignoredPaths,
                    excludedLlmPaths: draft.excludedLlmPaths,
                  },
                  key: null,
                });
                onSave(saved);
                setNote("保护设置已保存");
              } catch (e) {
                onError(e);
              } finally {
                setPending(null);
              }
            }}
          >
            保存保护设置
          </button>
          <button
            disabled={busy}
            onClick={async () => {
              setPending("save");
              setNote("");
              setTestError("");
              try {
                const saved = await api<Settings>("save_settings", {
                  settings,
                  key: "",
                });
                setKey("");
                onSave(saved);
                setNote("密钥已从 Windows 凭据存储移除");
              } catch (e) {
                onError(e);
              } finally {
                setPending(null);
              }
            }}
          >
            移除已保存密钥
          </button>
        </div>
      </section>
    </div>
  );
}
