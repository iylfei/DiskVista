import type { Page } from "@playwright/test";
import type { Bootstrap } from "../../../ui/src/lib/types";

export function installBackend(
  options: {
    deleteFails?: boolean;
    deleteDelay?: number;
    staleStatus?: boolean;
    active?: boolean;
  } = {},
) {
  const scan = {
    id: "current",
    root: "D:\\测试资料",
    started: 1787800000,
    finished: 1787800020,
    status: "complete",
    files: 3,
    directories: 1,
    logicalBytes: 4096,
    allocatedBytes: 4096,
    issues: 2,
    mode: "完整扫描",
    message: "",
  };
  const boot: Bootstrap = {
    volumes: [
      {
        path: "C:\\",
        label: "本地磁盘",
        fileSystem: "NTFS",
        totalBytes: 322122547200,
        freeBytes: 63672852480,
        removable: false,
      },
    ],
    scanLocations: [
      {
        id: "downloads",
        name: "查看下载中的大文件",
        path: "C:\\Users\\tester\\Downloads",
        description: "扫描下载文件夹，查找安装包、视频等大文件",
      },
      {
        id: "temp",
        name: "检查临时文件",
        path: "C:\\Users\\tester\\AppData\\Local\\Temp",
        description: "只扫描当前用户的临时文件夹",
      },
    ],
    scans: [
      scan,
      {
        ...scan,
        id: "older",
        root: "D:\\旧扫描",
        started: scan.started - 86400,
      },
    ],
    hasKey: true,
    accessPolicy: "",
    settings: {
      language: "zh-CN",
      scanRetention: 0,
      enhancedScan: false,
      communityEnabled: false,
      protectedPaths: ["D:\\保护目录"],
      unprotectedPaths: [],
      ignoredPaths: [],
      excludedLlmPaths: [],
      labels: {},
      llm: {
        enabled: false,
        automatic: false,
        metadataConsent: false,
        historyReferenceEnabled: false,
        baseUrl: "https://provider.example/v1",
        model: "original-model",
        format: "auto",
        tokenParameter: "max_tokens",
        maxOutputTokens: 1800,
        minimumBytes: 104857600,
        maxRequests: 10,
        concurrency: 2,
        timeoutSeconds: 120,
      },
    },
    analysisProgress: {
      scanId: "current",
      active: !!options.active,
      queued: options.active ? 99 : 0,
      finished: options.active ? 40 : 0,
      requests: options.active ? 9 : 0,
      maxRequests: 10,
      message: options.active ? "正在分析本批 20 个文件…" : "",
    },
  };
  const original = structuredClone(boot);
  const calls: { command: string; args: Record<string, unknown> }[] = [];
  const host = window as unknown as {
    __TAURI_INTERNALS__: {
      invoke: (command: string, args?: Record<string, any>) => Promise<unknown>;
    };
    __testCalls: typeof calls;
  };
  host.__testCalls = calls;
  host.__TAURI_INTERNALS__ = {
    invoke: async (command, args = {}) => {
      calls.push({ command, args: structuredClone(args) });
      switch (command) {
        case "bootstrap":
          return structuredClone(boot);
        case "runtime_status": {
          const snapshot = options.staleStatus ? original : boot;
          const current = snapshot.scans.find(
            (scan) => scan.id === args.scanId,
          );
          if (!current) throw new Error("记录不存在");
          return structuredClone({
            scan: current,
            scans: snapshot.scans,
            analysisProgress: snapshot.analysisProgress,
          });
        }
        case "delete_scan":
          if (options.deleteDelay)
            await new Promise((resolve) =>
              setTimeout(resolve, options.deleteDelay),
            );
          if (options.deleteFails) throw new Error("模拟数据库写入失败");
          boot.scans = boot.scans.filter((scan) => scan.id !== args.scanId);
          if (boot.analysisProgress.scanId === args.scanId)
            boot.analysisProgress = {
              ...boot.analysisProgress,
              scanId: null,
              message: "",
            };
          return structuredClone(boot.scans);
        case "save_settings":
          boot.settings = structuredClone(args.settings);
          if (args.key === "") boot.hasKey = false;
          return structuredClone(boot.settings);
        case "history_page":
          return { items: [], total: 0 };
        case "query_entries":
          return { items: [], total: 0 };
        case "cleanup_suggestions":
          return { groups: [], items: [], total: 0 };
        case "analysis_summaries":
          return [];
        case "application_units":
          return {
            items: [
              {
                id: "application-verifier",
                name: "Application Verifier (X64)",
                kind: "application",
                confidence: "medium",
                logicalBytes: 11703785882,
                occupiedBytes: 11703785882,
                fileCount: 109,
                estimated: true,
                complete: false,
                components: [
                  {
                    entryId: 1,
                    path: "C:\\Program Files\\Application Verifier",
                    role: "installation",
                    evidence: "Windows 卸载清单",
                    logicalBytes: 11703785882,
                    occupiedBytes: 11703785882,
                    fileCount: 109,
                    protected: true,
                  },
                ],
                children: [],
              },
            ],
            total: 458,
            applications: 1,
            uncertain: 827,
            occupiedBytes: 248786740019,
            estimated: true,
          };
        default:
          throw new Error(`Unexpected test command: ${command}`);
      }
    },
  };
}

export async function openApp(
  page: Page,
  options: Parameters<typeof installBackend>[0] = {},
) {
  await page.addInitScript(installBackend, options);
  await page.goto("/");
}

export async function deletionCalls(page: Page) {
  return page.evaluate(() =>
    (
      window as unknown as { __testCalls: { command: string }[] }
    ).__testCalls.filter((call) => call.command === "delete_scan"),
  );
}
