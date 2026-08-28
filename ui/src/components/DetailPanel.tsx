import { useEffect, useState } from "react";
import {
  X,
  FolderOpen,
  ShieldPlus,
  Sparkles,
  Info,
  Check,
  ListPlus,
} from "lucide-react";
import { api, bytes, date, riskText } from "../lib/api";
import type { FileRecord, AnalysisContext, AnalysisResult } from "../lib/types";
import Modal from "./Modal";
import HelpTip from "./HelpTip";
import { helpText } from "../lib/helpText";
import { startAnalysisPolling } from "../lib/analysisPolling";
import AnalysisResultCard from "./AnalysisResultCard";
import AnalysisHistory from "./AnalysisHistory";
import { fileCleanupBlockReason } from "../lib/cleanupTarget";
export default function DetailPanel({
  file,
  scanId,
  llmEnabled,
  analysisActive = false,
  revision = 0,
  selected = false,
  queued = false,
  addingToBasket = false,
  onClose,
  onSelect,
  onAddToBasket,
  onChanged,
  onError,
}: {
  file: FileRecord;
  scanId: string;
  llmEnabled: boolean;
  analysisActive?: boolean;
  revision?: number;
  selected?: boolean;
  queued?: boolean;
  addingToBasket?: boolean;
  onClose: () => void;
  onSelect: () => void;
  onAddToBasket?: () => void;
  onChanged: () => void;
  onError: (e: unknown) => void;
}) {
  const [context, setContext] = useState<{
    previewId: string;
    context: AnalysisContext;
  } | null>(null);
  const [ids, setIds] = useState<number[]>([]);
  const [samplePreview, setSamplePreview] = useState<{
    previewId: string;
    samples: { name: string; text: string }[];
  } | null>(null);
  const [results, setResults] = useState<AnalysisResult[]>([]);
  const [busy, setBusy] = useState(false);
  const [label, setLabel] = useState("");
  const [reloadResults, setReloadResults] = useState(0);
  const [annotating, setAnnotating] = useState(false);
  const [refreshingResults, setRefreshingResults] = useState(false);
  useEffect(() => {
    setResults([]);
    setContext(null);
    setSamplePreview(null);
  }, [file.id, scanId]);
  useEffect(
    () =>
      startAnalysisPolling({
        scanId,
        entryId: file.id,
        active: analysisActive,
        onResults: setResults,
        onError,
        onPending: setRefreshingResults,
      }),
    [file.id, scanId, analysisActive, revision, reloadResults, onError],
  );
  async function act(kind: string, value = "") {
    if (annotating) return;
    setAnnotating(true);
    try {
      await api("set_annotation", { scanId, entryId: file.id, kind, value });
      onChanged();
    } catch (e) {
      onError(e);
    } finally {
      setAnnotating(false);
    }
  }
  async function preview() {
    setBusy(true);
    try {
      setContext(await api("llm_context", { scanId, entryId: file.id }));
      setIds([]);
      setSamplePreview(null);
    } catch (e) {
      onError(e);
    } finally {
      setBusy(false);
    }
  }
  async function analyze() {
    if (!context) return;
    setBusy(true);
    try {
      await api("analyze", {
        previewId: context.previewId,
        samplePreviewId: samplePreview?.previewId ?? null,
      });
      setContext(null);
      onChanged();
    } catch (e) {
      onError(e);
    } finally {
      setBusy(false);
    }
  }
  const a = file.assessment;
  return (
    <aside className="detail-panel">
      <header>
        <span>文件详情</span>
        <button className="icon-button" aria-label="关闭详情" onClick={onClose}>
          <X size={18} />
        </button>
      </header>
      <div className="detail-content">
        <h2>{file.name || file.path}</h2>
        <p className="path-text">{file.path}</p>
        <p className="detail-size">
          占用 {bytes(file.allocatedBytes ?? file.logicalBytes)}
          {file.allocatedBytes === null ? "（估算）" : ""}
        </p>
        <span className={`badge ${a.risk}`}>{riskText(a.risk)}</span>

        <section>
          <h3>它可能是什么？</h3>
          <p>{a.purpose}</p>
        </section>
        <section>
          <h3>删除会发生什么？</h3>
          <p>{a.consequence}</p>
          <p className="muted">{a.recovery}</p>
        </section>
        {a.protectedReason && (
          <div className="warning">
            <ShieldPlus size={17} />
            <span>{a.protectedReason}</span>
          </div>
        )}
        {file.issue && (
          <div className="warning">
            <Info size={17} />
            <span>{file.issue}</span>
          </div>
        )}
        <details className="detail-metadata">
          <summary>大小、来源与时间</summary>
          <dl className="facts">
            <dt>
              逻辑大小
              <HelpTip label="逻辑大小" text={helpText.logicalSize} />
            </dt>
            <dd>{bytes(file.logicalBytes)}</dd>
            <dt>
              实际占用
              <HelpTip label="实际占用" text={helpText.diskUsage} />
            </dt>
            <dd>
              {bytes(file.allocatedBytes)}
              {file.allocatedBytes === null ? "（无法精确计算）" : ""}
            </dd>
            <dt>文件数量</dt>
            <dd>{file.fileCount.toLocaleString()}</dd>
            <dt>
              {["application_container", "system"].includes(a.category)
                ? "目录类型"
                : "关联应用"}
            </dt>
            <dd>
              {a.category === "application_container" ? (
                "多应用集合（非单个应用）"
              ) : a.category === "system" ? (
                "Windows 系统目录"
              ) : (
                <>
                  {a.owner
                    ? `${a.confidence === "low" ? "可能关联：" : ""}${a.owner}`
                    : "未知"}{" "}
                  ·{" "}
                  {{ high: "高", medium: "中", low: "低" }[a.confidence] ??
                    "未知"}
                  置信度
                  <HelpTip label="归属置信度" text={helpText.confidence} />
                </>
              )}
            </dd>
            <dt>修改时间</dt>
            <dd>{date(file.modified)}</dd>
            <dt>
              内容最新变化
              <HelpTip label="内容最新变化" text={helpText.latestChange} />
            </dt>
            <dd>{date(file.latestChange)}</dd>
            <dt>
              最后访问
              <HelpTip label="最后访问时间" text={helpText.activity} />
            </dt>
            <dd>{date(file.accessed)}（仅供参考）</dd>
          </dl>
        </details>
        <details>
          <summary>判断依据与注意事项</summary>
          {a.evidence.map((e, i) => (
            <div className="evidence" key={i}>
              <strong>{e.source}</strong>
              <p>{e.detail}</p>
            </div>
          ))}
          <p className="muted">
            来源是根据现有线索判断的，不一定是最初创建文件的程序。“很久没访问”也不代表可以删除。
          </p>
        </details>
        <div className="detail-actions">
          <button
            onClick={() =>
              api("open_location", { scanId, entryId: file.id }).catch(onError)
            }
          >
            <FolderOpen size={16} />
            在资源管理器中查看
          </button>
          <button disabled={annotating} onClick={() => act("protect")}>
            <ShieldPlus size={16} />
            保护此路径
          </button>
          <button disabled={annotating} onClick={() => act("ignore")}>
            忽略此项目
          </button>
          <button disabled={annotating} onClick={() => act("exclude_llm")}>
            禁止发送 AI
          </button>
        </div>
        <label>
          手动标注归属
          <div className="inline-form">
            <input
              value={label}
              onChange={(e) => setLabel(e.target.value)}
              placeholder="应用或用途名称"
              maxLength={100}
            />
            <button
              disabled={annotating || !label.trim()}
              onClick={() => act("label", label)}
            >
              保存
            </button>
          </div>
        </label>
        <section className="ai-section">
          <div className="section-heading">
            <h3>
              <Sparkles size={16} /> AI 辅助解释
            </h3>
            <span className="muted">仅建议</span>
          </div>
          <p>
            AI
            只提供建议，不会操作文件。发送前可查看信息；读取文件内容还需要你另外同意。
          </p>
          <button
            disabled={!llmEnabled || busy || a.risk === "protected"}
            onClick={preview}
          >
            {llmEnabled ? "预览将发送的信息" : "在设置中启用 AI"}
          </button>
          {results.map((result) => (
            <AnalysisResultCard result={result} key={result.id} />
          ))}
          <button
            disabled={analysisActive || refreshingResults}
            onClick={() => setReloadResults((v) => v + 1)}
          >
            {refreshingResults ? "正在刷新…" : "刷新分析记录"}
          </button>
        </section>
      </div>
      <footer>
        {onAddToBasket && (
          <button
            className="primary"
            disabled={
              queued ||
              addingToBasket ||
              !!fileCleanupBlockReason(file) ||
              annotating
            }
            aria-busy={addingToBasket || undefined}
            title={fileCleanupBlockReason(file) || "加入待清理清单"}
            onClick={onAddToBasket}
          >
            {queued ? <Check size={15} /> : <ListPlus size={15} />}
            {queued
              ? "已加入待清理清单"
              : addingToBasket
                ? "正在添加…"
                : "加入待清理清单"}
          </button>
        )}
        {(!onAddToBasket || !queued) && (
          <button
            className={onAddToBasket ? undefined : "primary"}
            disabled={a.risk === "protected" || queued || addingToBasket}
            aria-pressed={selected || queued}
            onClick={onSelect}
          >
            {queued ? "已在待清理清单中" : selected ? "取消选择" : "选择此项"}
          </button>
        )}
      </footer>
      {context && (
        <Modal
          title="AI 分析 · 本次发送预览"
          onClose={() => setContext(null)}
          wide
        >
          <div className="modal-body">
            <p>
              以下信息将发送到你设置的 AI
              服务。请检查文件名是否包含隐私；不会发送全盘文件记录或完整应用列表。
            </p>
            <h3>
              文件基本信息（元数据）
              <HelpTip label="元数据" text={helpText.metadata} />
            </h3>
            <pre className="metadata-preview">
              {JSON.stringify(context.context, null, 2)}
            </pre>
            {(context.context.historyReferences?.length ?? 0) > 0 && (
              <section>
                <h3>同时发送的回收历史</h3>
                <AnalysisHistory
                  references={context.context.historyReferences}
                />
              </section>
            )}
            <h3>
              可选：读取少量文件内容
              <HelpTip label="正文采样" text={helpText.sampling} />
            </h3>
            <p className="muted">
              最多选择 4 个文本文件，每个读取不超过 4
              KiB。先同意读取并查看片段，再决定是否发送。
            </p>
            {context.context.files
              .filter((f) => f.sampleAllowed)
              .map((f) => (
                <label className="check-line" key={f.entryId}>
                  <input
                    type="checkbox"
                    checked={ids.includes(f.entryId)}
                    disabled={!ids.includes(f.entryId) && ids.length >= 4}
                    onChange={(e) => {
                      setIds(
                        e.target.checked
                          ? [...ids, f.entryId]
                          : ids.filter((id) => id !== f.entryId),
                      );
                      setSamplePreview(null);
                    }}
                  />
                  <span className="path-text">
                    {f.name} · {bytes(f.bytes)}
                  </span>
                </label>
              ))}
            {ids.length > 0 && (
              <button
                onClick={async () => {
                  try {
                    setSamplePreview(
                      await api("preview_samples", {
                        previewId: context.previewId,
                        entryIds: ids,
                      }),
                    );
                  } catch (e) {
                    onError(e);
                  }
                }}
              >
                授权读取所选文件并预览片段
              </button>
            )}
            {samplePreview?.samples.map((s) => (
              <details key={s.name} open>
                <summary>{s.name}</summary>
                <pre className="metadata-preview">{s.text}</pre>
              </details>
            ))}
            <p className="warning-text">
              AI
              分析可能出错。服务商可能按其隐私政策保存收到的信息，请确认后再发送。
            </p>
          </div>
          <footer>
            <button onClick={() => setContext(null)}>不发送</button>
            <button
              className="primary"
              disabled={busy || (ids.length > 0 && !samplePreview)}
              onClick={analyze}
            >
              确认发送并分析
            </button>
          </footer>
        </Modal>
      )}
    </aside>
  );
}
