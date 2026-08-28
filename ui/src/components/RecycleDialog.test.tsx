import {
  Children,
  isValidElement,
  type ReactElement,
  type ReactNode,
} from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import RecycleDialog, { RecycleDialogView } from "./RecycleDialog";
import BasketPage from "../pages/BasketPage";
import {
  initialRecycleState,
  type RecycleDialogState,
} from "../lib/recycleSession";
import type { CleanupPreview, FileRecord } from "../lib/types";

const file = {
  id: 7,
  path: "D:\\Fixture\\Target",
  isDir: true,
  fileCount: 23,
  allocatedBytes: 4096,
  logicalBytes: 2048,
  complete: true,
  assessment: { consequence: "可能需要重新生成文件。" },
} as FileRecord;
const preview: CleanupPreview = {
  id: "preview-one",
  scanId: "scan-one",
  created: 1,
  policyFingerprint: "policy",
  pendingBytes: 8192,
  requiresExtraConfirmation: false,
  items: [
    {
      entryId: 7,
      path: 'D:\\Actual\\<img src="x">',
      bytes: 8192,
      fingerprint: "file",
      risk: "low",
      allowed: true,
      reason: "已检查实际目标",
    },
  ],
};
const ready: RecycleDialogState = {
  ...initialRecycleState,
  phase: "ready",
  preview,
  acknowledged: false,
  error: "",
};
const noop = () => {};
function view(state = ready, files = [file]) {
  return RecycleDialogView({
    files,
    state,
    onClose: noop,
    onRetry: noop,
    onAcknowledge: noop,
    onConfirm: noop,
    onCancelRemaining: noop,
  });
}
function buttons(node: ReactNode): ReactElement<Record<string, unknown>>[] {
  const found: ReactElement<Record<string, unknown>>[] = [];
  Children.forEach(node, (child) => {
    if (!isValidElement<{ children?: ReactNode }>(child)) return;
    if (child.type === "button")
      found.push(child as ReactElement<Record<string, unknown>>);
    found.push(...buttons(child.props.children));
  });
  return found;
}

describe("basket recycle confirmation dialog", () => {
  it("does not mount confirmation just by opening the basket", () => {
    const html = renderToStaticMarkup(
      <BasketPage
        items={[file]}
        scanId="scan-one"
        onRemove={noop}
        onDone={noop}
        onError={noop}
        onFindFiles={noop}
        onClear={noop}
      />,
    );
    expect(html).toContain("检查并移入回收站");
    expect(html).not.toContain("<dialog");
    expect(html).not.toContain("正在检查回收条件");
  });

  it("waits for pending additions without disabling remove or clear", () => {
    const render = (addingCount?: number) =>
      renderToStaticMarkup(
        <BasketPage
          items={[file]}
          scanId="scan-one"
          addingCount={addingCount}
          onRemove={noop}
          onDone={noop}
          onError={noop}
          onFindFiles={noop}
          onClear={noop}
        />,
      );
    const pending = render(2);
    expect(pending).toContain("正在添加 2 项…");
    expect(pending).toMatch(
      /<button[^>]*disabled=""[^>]*>检查并移入回收站<\/button>/,
    );
    for (const label of ["移出清单", "清空清单"]) {
      const button = pending.match(
        new RegExp(`<button[^>]*>${label}</button>`),
      )?.[0];
      expect(button).toBeDefined();
      expect(button).not.toContain("disabled");
    }
    const idle = render();
    expect(idle).not.toContain("正在添加");
    expect(idle).not.toMatch(
      /<button[^>]*disabled=""[^>]*>检查并移入回收站<\/button>/,
    );
  });

  it("starts in preview mode without exposing an enabled execute button", () => {
    const html = renderToStaticMarkup(
      <RecycleDialog
        scanId="scan-one"
        files={[file]}
        onClose={noop}
        onDone={noop}
        onBusyChange={noop}
      />,
    );
    expect(html).toContain("正在检查回收条件");
    expect(html).toContain("确认移入回收站");
    expect(
      buttons(view(initialRecycleState)).find(
        (button) => button.props.className === "primary",
      )?.props.disabled,
    ).toBe(true);
  });

  it("shows the actual preview separately from inclusive directory scan metadata and escapes it", () => {
    const html = renderToStaticMarkup(view());
    expect(html).toContain("将回收整个目录及其全部内容，包括单独列出的子目录");
    expect(html).toContain("目录扫描记录：23 个文件 · 占用 4 KiB");
    expect(html).toContain("本次回收大小：8 KiB");
    expect(html).toContain("已检查实际目标");
    expect(html).toContain("检查通过");
    expect(html).toContain("较低风险");
    expect(html).toContain("清理影响：可能需要重新生成文件。");
    expect(html).toContain("D:\\Actual\\&lt;img");
    expect(html).not.toContain('<img src="x">');
    expect(html).toContain("回收后可重新扫描更新空间占用");
    expect(html).toContain("不会永久删除");
    expect(html).toContain("清空回收站前通常不会释放空间");
    expect(html).toContain('type="checkbox"');
    expect(
      buttons(view()).find((button) => button.props.className === "primary")
        ?.props.disabled,
    ).toBe(true);
    expect(
      buttons(view({ ...ready, acknowledged: true })).find(
        (button) => button.props.className === "primary",
      )?.props.disabled,
    ).toBe(false);
  });

  it("does not add directory confirmation copy to ordinary files unless the preview requires extra acknowledgement", () => {
    const regular = { ...file, isDir: false };
    const html = renderToStaticMarkup(view(ready, [regular]));
    expect(html).not.toContain("整个目录");
    expect(html).not.toContain('type="checkbox"');
    expect(
      buttons(view(ready, [regular])).find(
        (button) => button.props.className === "primary",
      )?.props.disabled,
    ).toBe(false);
    const extra = {
      ...ready,
      preview: { ...preview, requiresExtraConfirmation: true },
    };
    expect(renderToStaticMarkup(view(extra, [regular]))).toContain(
      'type="checkbox"',
    );
    expect(
      buttons(view(extra, [regular])).find(
        (button) => button.props.className === "primary",
      )?.props.disabled,
    ).toBe(true);
  });

  it("shows refusal reasons and keeps confirmation disabled even if the box was checked", () => {
    const denied = {
      ...ready,
      acknowledged: true,
      preview: {
        ...preview,
        pendingBytes: 0,
        items: preview.items.map((item) => ({
          ...item,
          allowed: false,
          reason: "受保护目录，不能回收",
        })),
      },
    };
    const html = renderToStaticMarkup(view(denied));
    expect(html).toContain("不可回收");
    expect(html).toContain("受保护目录，不能回收");
    expect(html).toContain("没有可回收的项目");
    expect(
      buttons(view(denied)).find(
        (button) => button.props.className === "primary",
      )?.props.disabled,
    ).toBe(true);
  });

  it("shows the actual mixed preview count and deduplicated parent scope", () => {
    const child = {
      ...file,
      id: 8,
      path: `${file.path}\\child.txt`,
      isDir: false,
    };
    const denied = {
      ...file,
      id: 9,
      path: "D:\\Fixture\\denied.txt",
      isDir: false,
    };
    const state = {
      ...ready,
      preview: {
        ...preview,
        items: [
          preview.items[0],
          {
            ...preview.items[0],
            entryId: 9,
            path: denied.path,
            bytes: 1024,
            allowed: false,
            risk: "protected",
            reason: "文件已受保护 <script>",
          },
        ],
      },
    };
    const html = renderToStaticMarkup(view(state, [file, child, denied]));
    expect(html).toContain("待清理清单：3 项");
    expect(html).toMatch(/实际预览 2 项 · 检查通过 1 项 ·\s*不可回收 1 项/);
    expect(html).toContain("预览会合并重复范围");
    expect(html).toContain("目录扫描记录：23 个文件 · 占用 4 KiB");
    expect(html).toContain("本次回收大小：8 KiB");
    expect(html).not.toContain("child.txt");
    expect(html).toContain(denied.path);
    expect(html).toContain("文件已受保护 &lt;script&gt;");
    expect(html.match(/<li\b/g)).toHaveLength(2);
    expect(html).toContain(
      "我确认回收检查通过的项目，包括其中目录的全部内容。",
    );
  });

  it("locks modal close, cancel and confirm while executing", () => {
    const component = view({
      ...ready,
      acknowledged: true,
      phase: "executing",
    });
    expect(component.props.closeDisabled).toBe(true);
    const html = renderToStaticMarkup(component);
    expect(html).toContain("正在移入回收站，请等待操作完成");
    const tags = html.match(/<button[^>]*>/g) ?? [];
    expect(tags).toHaveLength(4);
    expect(tags.filter((tag) => tag.includes('disabled=""'))).toHaveLength(3);
    expect(
      buttons(component).find(
        (button) => button.props.children === "取消剩余项",
      )?.props.disabled,
    ).toBe(false);
    expect(html).toMatch(/<dialog[^>]*aria-labelledby=/);
  });

  it("keeps cancellation inside the running dialog without offering a new preview", () => {
    const cancel = vi.fn();
    const component = RecycleDialogView({
      files: [file],
      state: {
        ...ready,
        phase: "executing",
        acknowledged: true,
        cancelError: "请求失败 <script>",
      },
      onClose: noop,
      onRetry: noop,
      onAcknowledge: noop,
      onConfirm: noop,
      onCancelRemaining: cancel,
    });
    const html = renderToStaticMarkup(component);
    expect(html).toContain("取消请求失败：请求失败 &lt;script&gt;");
    expect(html).not.toContain("重新检查");
    const button = buttons(component).find(
      (button) => button.props.children === "取消剩余项",
    );
    (button?.props.onClick as () => void)();
    expect(cancel).toHaveBeenCalledOnce();
    const requested = view({
      ...ready,
      phase: "executing",
      acknowledged: true,
      cancelRequested: true,
    });
    const requestedHtml = renderToStaticMarkup(requested);
    expect(requestedHtml).toContain(
      "已请求取消尚未开始的项目，正在等待操作结果。",
    );
    expect(
      buttons(requested).find(
        (button) => button.props.children === "已请求取消剩余项",
      )?.props.disabled,
    ).toBe(true);
    expect(requested.props.closeDisabled).toBe(true);
  });

  it("offers only a fresh preview after errors and keeps errors local and escaped", () => {
    const retry = vi.fn();
    const component = RecycleDialogView({
      files: [file],
      state: {
        ...initialRecycleState,
        phase: "error",
        error: "检查失败 <script>",
      },
      onClose: noop,
      onRetry: retry,
      onAcknowledge: noop,
      onConfirm: noop,
      onCancelRemaining: noop,
    });
    const html = renderToStaticMarkup(component);
    expect(html).toContain('role="alert"');
    expect(html).toContain("检查失败 &lt;script&gt;");
    expect(html).toContain("重新检查");
    const button = buttons(component).find(
      (button) => button.props.children === "重新检查",
    );
    (button?.props.onClick as () => void)();
    expect(retry).toHaveBeenCalledOnce();
    expect(
      buttons(component).find((button) => button.props.className === "primary")
        ?.props.disabled,
    ).toBe(true);
  });
});
