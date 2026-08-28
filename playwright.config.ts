import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/e2e",
  testMatch: "**/*.spec.ts",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: 0,
  reporter: "list",
  use: {
    baseURL: "http://127.0.0.1:1421",
    browserName: "chromium",
    channel:
      process.env.PLAYWRIGHT_CHANNEL ||
      (process.platform === "win32" ? "msedge" : undefined),
    viewport: { width: 1120, height: 720 },
    trace: "retain-on-failure",
  },
  webServer: {
    command: "pnpm dev --port 1421",
    url: "http://127.0.0.1:1421",
    reuseExistingServer: !process.env.CI,
  },
});
