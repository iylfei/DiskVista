import { describe, expect, it, vi } from "vitest";
import {
  Children,
  isValidElement,
  type ReactElement,
  type ReactNode,
} from "react";
import { renderToStaticMarkup } from "react-dom/server";
import HelpTip from "./HelpTip";
import OverviewPage from "../pages/OverviewPage";
import BasketPage from "../pages/BasketPage";
import SettingsPage from "../pages/SettingsPage";
import ModelCapabilities from "./ModelCapabilities";
import AnalysisFilter from "./AnalysisFilter";
import { SuggestionListHeader } from "./SuggestionGroups";
import ScanPicker from "./ScanPicker";
import type { Scan, Settings, SuggestionGroup } from "../lib/types";

describe("plain-language help", () => {
  it("provides a named keyboard button without a native-title-only tooltip", () => {
    const html = renderToStaticMarkup(
      <HelpTip label="逻辑大小" text="文件内容的大小" />,
    );
    expect(html).toContain('type="button"');
    expect(html).toContain('aria-label="关于逻辑大小"');
    expect(html).not.toContain("title=");
    expect(html).not.toContain('role="tooltip"');
  });
  it("escapes help labels", () => {
    const html = renderToStaticMarkup(
      <HelpTip label="<script>" text="Explanation" />,
    );
    expect(html).not.toContain("<script>");
    expect(html).toContain("&lt;script&gt;");
  });
  it("keeps the help button in a non-breaking superscript", () => {
    const html = renderToStaticMarkup(
      <span>
        逻辑大小
        <HelpTip label="逻辑大小" text="文件内容的大小" />
      </span>,
    );
    expect(html).toContain('逻辑大小<sup class="help-anchor">\u2060<button');
    expect(html).toContain('width="12" height="12"');
    expect(html).toContain("</button></sup>");
  });
  it("uses task-focused overview copy instead of a slogan", () => {
    const html = renderToStaticMarkup(
      <OverviewPage
        volumes={[]}
        locations={[]}
        disabled={false}
        scan={null}
        onScan={() => {}}
        onBrowse={() => {}}
        onMap={() => {}}
      />,
    );
    expect(html).not.toContain("找出占用空间，选择不再需要的文件");
    expect(html).toContain("选择磁盘或文件夹，查看空间占用。");
    expect(html).not.toContain("每个文件，都值得");
    expect(html).toContain("清理前须知");
  });
  it("offers direct recycling without repeated confirmation copy", () => {
    const html = renderToStaticMarkup(
      <BasketPage
        items={[]}
        onFindFiles={() => {}}
        onClear={() => {}}
        scanId="s"
        onRemove={() => {}}
        onDone={() => {}}
        onError={() => {}}
      />,
    );
    expect(html).toContain("移入回收站");
    expect(html).not.toContain("检查并移入回收站");
    expect(html).not.toContain("先预览，再确认");
    expect(html).not.toContain("需要你确认");
    expect(html).toContain("去查看清理建议");
  });
  it("offers the supplied scan locations and disables scanning controls while busy", () => {
    const html = renderToStaticMarkup(
      <OverviewPage
        volumes={[
          {
            path: "D:\\",
            label: "数据",
            freeBytes: 10,
            totalBytes: 100,
            fileSystem: "NTFS",
            removable: false,
            identity: "fixture-volume",
          },
        ]}
        locations={[
          {
            id: "downloads",
            name: "查看下载中的大文件",
            path: "D:\\迁移后的下载",
            description: "扫描下载文件夹",
          },
        ]}
        disabled
        scan={null}
        onScan={() => {}}
        onBrowse={() => {}}
        onMap={() => {}}
      />,
    );
    expect(html).toContain("扫描 D 盘");
    expect(html).toContain('title="D:\\迁移后的下载"');
    const buttons = html.match(/<button[^>]*>/g) ?? [];
    expect(buttons.length).toBe(4);
    expect(buttons.every((button) => button.includes('disabled=""'))).toBe(
      true,
    );
  });
});

describe("scan history picker", () => {
  const scan: Scan = {
    id: "scan-one",
    root: "D:\\",
    started: 1724803200,
    finished: 1724803210,
    status: "complete",
    files: 10,
    directories: 2,
    logicalBytes: 1024,
    allocatedBytes: 4096,
    issues: 0,
    mode: "standard",
    message: "",
  };

  it("keeps a single scan visible and hides an empty history", () => {
    const render = (scans: Scan[]) =>
      renderToStaticMarkup(
        <ScanPicker
          scans={scans}
          current={scan.id}
          disabled={false}
          deleteDisabled={false}
          onDelete={async () => {}}
          onSelect={() => {}}
        />,
      );
    expect(render([])).toBe("");
    const html = render([scan]);
    expect(html).toContain('aria-label="扫描记录"');
    expect(html).toContain('aria-expanded="false"');
    expect(html).toContain("D:\\");
    expect(html).toContain("扫描完成");
  });

  it("preserves the selected scan and disables changing it when busy", () => {
    const html = renderToStaticMarkup(
      <ScanPicker
        scans={[scan, { ...scan, id: "scan-two", root: "E:\\" }]}
        current="scan-two"
        disabled
        deleteDisabled
        onDelete={async () => {}}
        onSelect={() => {}}
      />,
    );
    expect(html).toContain("E:\\");
    expect(html).not.toContain("D:\\");
    expect(html).toMatch(/<button[^>]*aria-label="扫描记录"[^>]*disabled=""/);
  });

  it("distinguishes scans of the same location on the same day", () => {
    const render = (current: string) =>
      renderToStaticMarkup(
        <ScanPicker
          scans={[
            scan,
            { ...scan, id: "scan-later", started: scan.started + 3600 },
          ]}
          current={current}
          disabled={false}
          deleteDisabled={false}
          onDelete={async () => {}}
          onSelect={() => {}}
        />,
      );
    const options = [render(scan.id), render("scan-later")];
    expect(options).toHaveLength(2);
    expect(options[0]).not.toEqual(options[1]);
    expect(options.every((option) => /\d{2}:\d{2}/.test(option))).toBe(true);
  });
});

describe("AI output limit settings", () => {
  const settings: Settings = {
    scanRetention: 0,
    enhancedScan: false,
    communityEnabled: false,
    protectedPaths: [],
    unprotectedPaths: [],
    ignoredPaths: [],
    excludedLlmPaths: [],
    labels: {},
    llm: {
      enabled: true,
      automatic: false,
      metadataConsent: false,
      historyReferenceEnabled: false,
      baseUrl: "http://localhost:8000/v1",
      model: "test-model",
      format: "auto",
      tokenParameter: "max_tokens",
      maxOutputTokens: 32768,
      minimumBytes: 1048576,
      maxRequests: 6,
      concurrency: 1,
      timeoutSeconds: 30,
    },
  };
  function render(llm: Partial<Settings["llm"]> = {}) {
    return renderToStaticMarkup(
      <SettingsPage
        settings={{ ...settings, llm: { ...settings.llm, ...llm } }}
        hasKey={false}
        onSave={() => {}}
        onError={() => {}}
      />,
    );
  }
  const outputInput = (html: string) =>
    html.match(/<input[^>]*id="ai-output-token-limit"[^>]*>/)?.[0] ?? "";

  it("shows a separate per-request output budget without changing the request count", () => {
    const html = render();
    const input = outputInput(html);
    expect(input).toContain('aria-label="单次输出上限（token）"');
    expect(input).toContain('value="32768"');
    expect(input).toContain('min="1" max="1048576" step="1"');
    expect(input).not.toContain("disabled");
    expect(html).toMatch(/<input[^>]*id="ai-request-limit"[^>]*value="6"/);
    expect(html).toContain("生成预算");
    expect(html).toContain("上下文不等于输出上限");
    expect(html).toContain("不会验证单次输出上限");
  });

  it("shows the legacy default when the saved configuration has no output limit", () => {
    expect(outputInput(render({ maxOutputTokens: undefined }))).toContain(
      'value="1800"',
    );
  });

  it("disables the budget field and explains it is not sent in none mode", () => {
    const html = render({ tokenParameter: "none" });
    const input = outputInput(html);
    expect(input).toContain('disabled=""');
    expect(input).toContain('value="32768"');
    expect(input).not.toContain("aria-invalid");
    expect(html).toContain("当前不发送此数值。");
  });

  it("keeps a manually saved budget unchanged when a model is recognized", () => {
    const html = render({
      baseUrl: "https://opencode.ai/zen/go/v1",
      model: "glm-5.3-flash",
      maxOutputTokens: 1800,
    });
    expect(outputInput(html)).toContain('value="1800"');
    expect(html).toContain("使用建议值");
    expect(html).toMatch(/<input[^>]*id="ai-request-limit"[^>]*value="6"/);
  });

  it("explains the batch floor and per-file history limit without rewriting an old threshold", () => {
    const html = render();
    expect(html).toMatch(
      /<input[^>]*id="ai-minimum-size"[^>]*min="100"[^>]*value="1"/,
    );
    expect(html).toContain("最低按 100 MiB 生效");
    expect(html).toContain("不包含目录或已明确所属应用的文件");
    expect(html).toContain("每个文件最多 6");
    expect(html).toContain("历史只作参考，不扩大分析范围");
    expect(html).not.toContain("也会分析与回收历史相关的项目");
  });

  it("places privacy and all three permissions after batch limits and before save/test actions", () => {
    const html = render();
    const positions = [
      'id="ai-minimum-size"',
      'id="ai-request-limit"',
      'id="ai-concurrency"',
      "重试也计入请求上限。",
      'class="privacy-note"',
      "扫描完成后，自动分析来源未明确的大文件",
      'id="ai-metadata-consent"',
      'id="ai-history-reference"',
      "历史只作参考，不扩大分析范围。",
      ">测试连接</button>",
      ">保存设置</button>",
    ].map((text) => html.indexOf(text));
    expect(positions.every((position) => position >= 0)).toBe(true);
    expect(positions).toEqual([...positions].sort((a, b) => a - b));
  });
});

describe("AI analysis filter", () => {
  it.each(["", "analyzed", "unanalyzed"])(
    "offers all three states and emits the %s query value",
    (value) => {
      const onChange = vi.fn();
      const element = AnalysisFilter({ value, onChange });
      const html = renderToStaticMarkup(element);
      expect(html).toContain('aria-label="AI 分析筛选"');
      expect(html).toContain("全部 AI 状态");
      expect(html).toContain("AI 已分析");
      expect(html).toContain("AI 未分析");
      expect(html).toContain("失败和过期结果归为未分析");
      expect(html).toContain(`value="${value}" selected=""`);
      element.props.onChange({ target: { value } });
      expect(onChange).toHaveBeenCalledExactlyOnceWith(value);
    },
  );
});

describe("local model capabilities", () => {
  const props = {
    baseUrl: "https://opencode.ai/zen/go/v1",
    modelId: "glm-5.3-flash",
    outputTokens: "1800",
    tokenParameter: "max_tokens",
    onUseSuggestion: () => {},
  };
  function render(overrides: Partial<typeof props> = {}) {
    return renderToStaticMarkup(
      <ModelCapabilities {...props} {...overrides} />,
    );
  }
  function findButton(
    node: ReactNode,
  ): ReactElement<{ onClick: () => void }> | null {
    if (!isValidElement<{ children?: ReactNode }>(node)) return null;
    if (node.type === "button")
      return node as ReactElement<{ onClick: () => void }>;
    for (const child of Children.toArray(node.props.children)) {
      const button = findButton(child);
      if (button) return button;
    }
    return null;
  }

  it("distinguishes context, reference output and the app's starting suggestion", () => {
    const html = render();
    expect(html).toContain("上下文：1,000,000 token");
    expect(html).toContain("参考最大输出：131,072 token");
    expect(html).toContain("起始建议 16,384 token");
    expect(html).toContain("models.dev");
    expect(html).toContain('title="https://models.dev/api.json"');
    expect(html).toMatch(/目录 \d{4}-\d{2}-\d{2}/);
  });

  it("keeps an unknown model editable and renders nothing for an empty model ID", () => {
    expect(render({ modelId: "not-in-the-catalog" })).toContain(
      "暂未识别此模型，可继续手动填写。",
    );
    expect(render({ modelId: "not-in-the-catalog" })).not.toContain(
      "使用建议值",
    );
    expect(render({ modelId: " " })).toBe("");
  });

  it("qualifies a custom gateway match without claiming its protocol is incompatible", () => {
    const html = render({ baseUrl: "https://gateway.example/v1" });
    expect(html).toContain("当前服务的实际限制可能不同");
    expect(html).not.toContain("当前需要兼容 chat/completions");
  });

  it("warns about a known non-chat protocol without guessing for an unknown protocol", () => {
    expect(render({ modelId: "gpt-5.6-luna" })).toContain(
      "当前需要兼容 chat/completions",
    );
    expect(
      render({
        baseUrl: "https://api.openai.com/v1",
        modelId: "gpt-5.6-luna",
      }),
    ).not.toContain("当前需要兼容 chat/completions");
  });

  it("warns above a reference allowance without blocking the suggestion button", () => {
    const html = render({ outputTokens: "262144" });
    expect(html).toContain("填写值超过参考最大输出");
    expect(html).toContain("仍可保留手填值");
    expect(html).not.toContain('disabled=""');
  });

  it("does not warn about a reference allowance when that budget is not sent", () => {
    const html = render({ outputTokens: "262144", tokenParameter: "none" });
    expect(html).not.toContain("填写值超过参考最大输出");
    expect(html).toContain("保持不发送长度参数");
  });

  it("keeps deprecated models recognizable but does not imply current availability", () => {
    expect(
      render({
        baseUrl: "https://opencode.ai/zen/v1",
        modelId: "glm-4.6",
      }),
    ).toContain("目录已将此型号标为弃用");
  });

  it.each([
    ["https://opencode.ai/zen/go/v1", "max_completion_tokens", "max_tokens"],
    ["https://opencode.ai/zen/go/v1", "none", null],
    ["https://gateway.example/v1", "max_completion_tokens", null],
  ])(
    "only applies a suggestion after a click and preserves unconfirmed/none parameters (%s, %s)",
    (baseUrl, tokenParameter, expectedParameter) => {
      const onUseSuggestion = vi.fn();
      const button = findButton(
        ModelCapabilities({
          ...props,
          baseUrl,
          tokenParameter,
          onUseSuggestion,
        }),
      );
      expect(onUseSuggestion).not.toHaveBeenCalled();
      expect(button).not.toBeNull();
      button!.props.onClick();
      expect(onUseSuggestion).toHaveBeenCalledExactlyOnceWith({
        maxOutputTokens: 16384,
        ...(expectedParameter ? { tokenParameter: expectedParameter } : {}),
      });
    },
  );
});

describe("compact suggestion list header", () => {
  const group: SuggestionGroup = {
    id: "temporary",
    name: "临时文件",
    purpose: "这里是分类用途的完整说明。",
    consequence: "清理后相关应用需要重新生成这些文件。",
    count: 200,
    occupiedBytes: 1048576,
    estimated: false,
    recognized: true,
  };

  it("puts the full purpose and impact in the title help instead of a separate text block", () => {
    const element = SuggestionListHeader({
      group,
      sort: "size",
      onSort: () => {},
      onBack: () => {},
    });
    const html = renderToStaticMarkup(element);
    expect(html).toContain('aria-label="关于临时文件的用途与清理影响"');
    expect(html).not.toContain(group.purpose);
    expect(html).not.toContain(group.consequence);
    const heading = Children.toArray(element.props.children).find(
      (child) => isValidElement(child) && child.type === "strong",
    ) as ReactElement<{ children: ReactNode }>;
    const help = Children.toArray(heading.props.children).find(
      (child) => isValidElement(child) && child.type === HelpTip,
    ) as ReactElement<{ text: string }>;
    expect(help.props.text).toBe(
      `用途：${group.purpose} 清理影响：${group.consequence}`,
    );
  });

  it.each(["activity_desc", "activity_asc"])(
    "shows explicit time directions and emits the %s query sort",
    (sort) => {
      const onSort = vi.fn();
      const element = SuggestionListHeader({ group, sort, onSort });
      const html = renderToStaticMarkup(element);
      expect(html).toContain("最近变化：从新到旧");
      expect(html).toContain("最近变化：从旧到新");
      expect(html).toContain(`value="${sort}" selected=""`);
      expect(html).not.toContain('value="activity"');
      const select = Children.toArray(element.props.children).find(
        (child) => isValidElement(child) && child.type === "select",
      ) as ReactElement<{
        onChange: (event: { target: { value: string } }) => void;
      }>;
      select.props.onChange({ target: { value: sort } });
      expect(onSort).toHaveBeenCalledWith(sort);
    },
  );

  it("also provides sorting for search results outside a category", () => {
    const html = renderToStaticMarkup(
      <SuggestionListHeader sort="size" onSort={() => {}} />,
    );
    expect(html).toContain("文件列表");
    expect(html).toContain('aria-label="建议排序"');
    expect(html).not.toContain("返回分类");
  });
});
