import {
  Children,
  isValidElement,
  type ReactElement,
  type ReactNode,
} from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import BasketPage from "../pages/BasketPage";
import {
  initialRecycleState,
  type RecycleDialogState,
} from "../lib/recycleSession";
import type { FileRecord } from "../lib/types";
import RecycleDialog, { RecycleDialogView } from "./RecycleDialog";

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
const noop = () => {};

function view(state: RecycleDialogState = initialRecycleState, files = [file]) {
  return RecycleDialogView({
    files,
    state,
    onClose: noop,
    onRetry: noop,
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

describe("one-click basket recycle dialog", () => {
  it("shows stage, target and examined-entry counts without estimating an elapsed-time percentage", () => {
    const html = renderToStaticMarkup(
      view({
        ...initialRecycleState,
        checkProgress: {
          stage: "filesystem",
          targetsDone: 1,
          targetsTotal: 3,
          currentPath: "D:\\Fixture\\<target>",
          checkedEntries: 640,
        },
      }),
    );
    expect(html).toContain("核对当前文件");
    expect(html).toContain("1 / 3");
    expect(html).toContain("640");
    expect(html).toContain("&lt;target&gt;");
    expect(html).toContain('value="1"');
    expect(html).toContain('max="3"');
    expect(html).toContain("取消并关闭");
  });

  it("offers one direct recycle action without mounting the dialog early", () => {
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

    expect(html).toContain("移入回收站");
    expect(html).not.toContain("检查并移入回收站");
    expect(html).not.toContain("<dialog");
  });

  it("waits for pending additions before enabling recycle", () => {
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
      /<button[^>]*disabled=""[^>]*>移入回收站<\/button>/,
    );
    for (const label of ["移出清单", "清空清单"]) {
      const button = pending.match(
        new RegExp(`<button[^>]*>${label}</button>`),
      )?.[0];
      expect(button).toBeDefined();
      expect(button).not.toContain("disabled");
    }
    expect(render()).not.toMatch(
      /<button[^>]*disabled=""[^>]*>移入回收站<\/button>/,
    );
  });

  it("shows a concise status while checking", () => {
    const html = renderToStaticMarkup(
      <RecycleDialog
        scanId="scan-one"
        files={[file]}
        onClose={noop}
        onDone={noop}
        onBusyChange={noop}
      />,
    );

    expect(html).toContain("正在准备…");
    expect(html).not.toContain("实际回收预览");
    expect(html).not.toContain("本次回收大小");
    expect(html).not.toContain('type="checkbox"');
    expect(html).not.toContain("确认移入回收站");
  });

  it("locks closing during execution and supports cancellation", () => {
    const cancel = vi.fn();
    const component = RecycleDialogView({
      files: [file],
      state: { ...initialRecycleState, phase: "executing" },
      onClose: noop,
      onRetry: noop,
      onCancelRemaining: cancel,
    });
    const html = renderToStaticMarkup(component);

    expect(component.props.closeDisabled).toBe(true);
    expect(html).toContain("正在移入回收站…");
    expect(html).not.toContain("重试");
    expect(html).not.toContain("确认移入回收站");
    const cancelButton = buttons(component).find(
      (button) => button.props.children === "取消剩余项",
    );
    expect(cancelButton?.props.disabled).toBe(false);
    (cancelButton?.props.onClick as () => void)();
    expect(cancel).toHaveBeenCalledOnce();
    expect(
      buttons(component).find((button) => button.props.children === "关闭")
        ?.props.disabled,
    ).toBe(true);
  });

  it("shows cancellation state without exposing a new confirmation", () => {
    const component = view({
      ...initialRecycleState,
      phase: "executing",
      cancelRequested: true,
      cancelError: "请求失败 <script>",
    });
    const html = renderToStaticMarkup(component);

    expect(html).toContain("正在取消剩余项…");
    expect(html).toContain("取消失败：请求失败 &lt;script&gt;");
    expect(html).not.toContain("确认移入回收站");
    expect(
      buttons(component).find(
        (button) => button.props.children === "已请求取消剩余项",
      )?.props.disabled,
    ).toBe(true);
  });

  it("shows a local escaped error and retries the whole flow", () => {
    const retry = vi.fn();
    const component = RecycleDialogView({
      files: [file],
      state: {
        ...initialRecycleState,
        phase: "error",
        error: "安全检查失败 <script>",
      },
      onClose: noop,
      onRetry: retry,
      onCancelRemaining: noop,
    });
    const html = renderToStaticMarkup(component);

    expect(html).toContain('role="alert"');
    expect(html).toContain("安全检查失败 &lt;script&gt;");
    expect(html).not.toContain("确认移入回收站");
    const retryButton = buttons(component).find(
      (button) => button.props.children === "重试",
    );
    (retryButton?.props.onClick as () => void)();
    expect(retry).toHaveBeenCalledOnce();
    expect(component.props.closeDisabled).toBe(false);
  });
});
