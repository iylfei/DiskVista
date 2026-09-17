import { useEffect, useId, useRef, useState } from "react";
import type { Scan } from "../lib/types";
import { History, ChevronDown, Check, X } from "lucide-react";
import { dateTime, statusText } from "../lib/api";
import HelpTip from "./HelpTip";
import Modal from "./Modal";
import ScanDeletionIndicator from "./ScanDeletionIndicator";
import { helpText } from "../lib/helpText";
import "./scan-picker.css";

export default function ScanPicker({
  scans,
  current,
  disabled,
  deleteDisabled,
  deletingScan = null,
  onSelect,
  onDelete,
}: {
  scans: Scan[];
  current: string;
  disabled: boolean;
  deleteDisabled: boolean;
  deletingScan?: Scan | null;
  onSelect: (scan: Scan) => void;
  onDelete: (scan: Scan) => void;
}) {
  const [open, setOpen] = useState(false);
  const [target, setTarget] = useState<Scan | null>(null);
  const wrapper = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const listId = useId();
  const selected = scans.find((s) => s.id === current);
  const deletingId = deletingScan?.id;
  useEffect(() => {
    if (target) return () => trigger.current?.focus();
  }, [target]);
  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => {
      if (!wrapper.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", outside);
    return () => document.removeEventListener("pointerdown", outside);
  }, [open]);
  function closeDialog() {
    setTarget(null);
  }
  function confirm() {
    if (!target || deleteDisabled || deletingId) return;
    const scan = target;
    closeDialog();
    onDelete(scan);
  }
  if (!scans.length) return null;
  return (
    <div
      className="scan-picker"
      ref={wrapper}
      onKeyDown={(event) => {
        if (event.key === "Escape" && open) {
          event.preventDefault();
          setOpen(false);
          trigger.current?.focus();
        }
      }}
    >
      <History size={17} aria-hidden="true" />
      <span className="scan-picker-label">
        扫描记录
        <HelpTip label="扫描记录" text={helpText.snapshot} />
      </span>
      <div className="scan-picker-control">
        <button
          type="button"
          className="scan-picker-trigger"
          ref={trigger}
          aria-label="扫描记录"
          aria-expanded={open}
          aria-controls={listId}
          disabled={disabled}
          onClick={() => setOpen(!open)}
        >
          <span>
            {selected
              ? `${dateTime(selected.started)} · ${selected.root} · ${statusText(selected.status)}`
              : "选择扫描记录"}
          </span>
          <ChevronDown size={14} aria-hidden="true" />
        </button>
        {open && (
          <ul
            className="scan-picker-list"
            id={listId}
            aria-label="扫描记录列表"
          >
            {scans.map((scan) => (
              <li key={scan.id} data-scan-id={scan.id}>
                <button
                  type="button"
                  className="scan-picker-option"
                  aria-pressed={scan.id === current}
                  disabled={disabled || scan.id === deletingId}
                  onClick={() => {
                    onSelect(scan);
                    setOpen(false);
                    trigger.current?.focus();
                  }}
                >
                  <Check
                    size={14}
                    aria-hidden="true"
                    style={{
                      visibility: scan.id === current ? "visible" : "hidden",
                    }}
                  />
                  <span>
                    <span className="scan-picker-path" title={scan.root}>
                      {scan.root}
                    </span>
                    <small>
                      {dateTime(scan.started)} ·{" "}
                      {scan.id === deletingId
                        ? "正在删除…"
                        : statusText(scan.status)}
                    </small>
                  </span>
                </button>
                <button
                  type="button"
                  className="icon-button scan-picker-delete"
                  aria-label={`删除扫描记录：${dateTime(scan.started)} · ${scan.root}`}
                  title={
                    deletingId
                      ? "已有扫描记录正在后台删除"
                      : deleteDisabled
                        ? "请先等待扫描、AI 分析或回收结束"
                        : "删除这条扫描记录"
                  }
                  disabled={
                    deleteDisabled ||
                    !!deletingId ||
                    ["queued", "scanning", "aggregating"].includes(scan.status)
                  }
                  onClick={() => {
                    setTarget(scan);
                    setOpen(false);
                  }}
                >
                  <X size={15} />
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
      {deletingScan && <ScanDeletionIndicator scan={deletingScan} />}
      {target && (
        <Modal title="删除扫描记录" onClose={closeDialog}>
          <div className="modal-body scan-delete-body">
            <p>确定删除这条扫描记录吗？</p>
            <p className="path-text">
              <strong>{target.root}</strong>
              <br />
              {dateTime(target.started)} · {statusText(target.status)}
            </p>
            <p>
              将删除本次扫描的文件索引和 AI
              分析结果，无法撤销。原文件和回收操作历史不会删除。
            </p>
            {deletingId ? (
              <p role="status">已有扫描记录正在后台删除</p>
            ) : (
              deleteDisabled && (
                <p role="status">请先等待扫描、AI 分析或回收结束。</p>
              )
            )}
          </div>
          <footer>
            <button type="button" onClick={closeDialog}>
              取消
            </button>
            <button
              type="button"
              className="scan-delete-confirm"
              disabled={!!deletingId || deleteDisabled}
              onClick={confirm}
            >
              确认删除
            </button>
          </footer>
        </Modal>
      )}
    </div>
  );
}
