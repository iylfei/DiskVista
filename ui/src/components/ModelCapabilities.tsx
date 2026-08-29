import { identifyModel } from "../lib/modelCatalog";
import { parseMaxOutputTokens } from "../lib/settingsValidation";
import "./model-capabilities.css";
import { localeName } from "../i18n/locale";

interface Props {
  baseUrl: string;
  modelId: string;
  outputTokens: string;
  tokenParameter: string;
  disabled?: boolean;
  onUseSuggestion: (value: {
    maxOutputTokens: number;
    tokenParameter?: string;
  }) => void;
}

const tokens = (value: number | null) =>
  value === null ? "未收录" : `${value.toLocaleString(localeName())} token`;

export default function ModelCapabilities({
  baseUrl,
  modelId,
  outputTokens,
  tokenParameter,
  disabled = false,
  onUseSuggestion,
}: Props) {
  if (!modelId.trim()) return null;
  const model = identifyModel(baseUrl, modelId);
  if (!model) {
    return (
      <p className="model-capabilities-unrecognized">
        暂未识别此模型，可继续手动填写。
      </p>
    );
  }
  const parameter =
    tokenParameter !== "none" && model.confidence === "endpoint"
      ? model.recommendedTokenParameter
      : null;
  const currentOutput = parseMaxOutputTokens(outputTokens);
  const exceedsReference =
    tokenParameter !== "none" &&
    currentOutput !== null &&
    model.output !== null &&
    currentOutput > model.output;
  const nativeProtocol =
    model.confidence === "endpoint" &&
    ["responses", "anthropic", "google"].includes(model.protocol);
  const source = model.sourceUrl.replace(/^https?:\/\//, "").split("/")[0];
  return (
    <section className="model-capabilities" aria-label="模型参考信息">
      <div className="model-capabilities-heading">
        <span>
          <strong>{model.name}</strong>
          <span className="model-capabilities-provider">
            {model.providerName}
          </span>
        </span>
        <small title={model.sourceUrl}>
          来源 {source} · 目录 {model.updatedAt.slice(0, 10)}
        </small>
      </div>
      <div className="model-capabilities-values">
        <span>上下文：{tokens(model.context)}</span>
        <span>参考最大输出：{tokens(model.output)}</span>
      </div>
      {model.confidence === "reference" && (
        <p className="model-capabilities-note">
          仅按模型名称匹配参考，当前服务的实际限制可能不同。
        </p>
      )}
      {nativeProtocol && (
        <p className="model-capabilities-warning">
          目录记录为 {model.protocol} 接口；当前需要兼容 chat/completions
          的服务地址。
        </p>
      )}
      {model.status === "deprecated" && (
        <p className="model-capabilities-warning">
          目录已将此型号标为弃用，请确认当前服务仍然提供。
        </p>
      )}
      {exceedsReference && (
        <p className="model-capabilities-warning">
          填写值超过参考最大输出，请确认服务实际限制；仍可保留手填值。
        </p>
      )}
      {model.suggestedOutputTokens !== null && (
        <div className="model-capabilities-suggestion">
          <small>
            起始建议 {tokens(model.suggestedOutputTokens)}
            {parameter && ` · ${parameter}`}
            {tokenParameter === "none" && " · 保持不发送长度参数"}
          </small>
          <button
            type="button"
            disabled={disabled}
            onClick={() =>
              onUseSuggestion({
                maxOutputTokens: model.suggestedOutputTokens!,
                ...(parameter ? { tokenParameter: parameter } : {}),
              })
            }
          >
            使用建议值
          </button>
        </div>
      )}
    </section>
  );
}
