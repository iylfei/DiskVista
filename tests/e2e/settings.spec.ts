import { expect, test } from "@playwright/test";
import { openApp } from "./fixtures/backend";

test("partial saves preserve other unfinished inputs", async ({ page }) => {
  await openApp(page);
  await page.getByRole("button", { name: "设置", exact: true }).click();
  const model = page.getByRole("textbox", { name: "模型 ID", exact: true });
  const output = page.getByRole("spinbutton", {
    name: "单次输出上限（token）",
    exact: true,
  });
  await model.fill("unsaved-model");
  await output.fill("");
  await page
    .getByRole("checkbox", { name: "启用增强扫描（USN）", exact: true })
    .check();
  await page.getByRole("button", { name: "移除", exact: true }).click();
  await page.getByRole("button", { name: "保存保护设置", exact: true }).click();
  await expect(page.getByRole("status")).toHaveText("保护设置已保存");
  await expect(model).toHaveValue("unsaved-model");
  await expect(output).toHaveValue("");
  await expect(
    page.getByRole("checkbox", { name: "启用增强扫描（USN）", exact: true }),
  ).toBeChecked();
  await page
    .getByRole("button", { name: "移除已保存密钥", exact: true })
    .click();
  await expect(page.getByRole("status")).toHaveText(
    "密钥已从 Windows 凭据存储移除",
  );
  await expect(model).toHaveValue("unsaved-model");
  await expect(output).toHaveValue("");
  await output.fill("8192");
  await page.getByRole("button", { name: "保存设置", exact: true }).click();
  await expect(page.getByRole("status")).toContainText("设置已保存");
  await page.getByRole("button", { name: "总览", exact: true }).click();
  await page.getByRole("button", { name: "设置", exact: true }).click();
  await expect(model).toHaveValue("unsaved-model");
  await expect(output).toHaveValue("8192");
});

test("switches the complete interface to English and saves the choice", async ({
  page,
}) => {
  await openApp(page, { active: true });
  await page.getByRole("button", { name: "设置", exact: true }).click();
  await page
    .getByRole("combobox", { name: "语言", exact: true })
    .selectOption("en");

  await expect(page.locator("html")).toHaveAttribute("lang", "en");
  await expect(page.locator(".app-shell")).toHaveAttribute(
    "data-language",
    "en",
  );
  await expect(
    page.getByRole("button", { name: "Overview", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Interface language", exact: true }),
  ).toBeVisible();

  const savedLanguage = await page.evaluate(() => {
    const calls = (
      window as unknown as {
        __testCalls: { command: string; args: Record<string, any> }[];
      }
    ).__testCalls;
    return calls.filter((call) => call.command === "save_settings").at(-1)?.args
      .settings.language;
  });
  expect(savedLanguage).toBe("en");

  await page.getByRole("button", { name: "Overview", exact: true }).click();
  await expect(
    page.getByText("Analyzing this batch of 20 files…"),
  ).toBeVisible();
  await expect(page.locator(".scan-picker-trigger")).toContainText(
    "Scan complete",
  );
  await expect(page.getByText("Find large files in Downloads")).toBeVisible();
  await expect(page.getByText("Check temporary files")).toBeVisible();
  await expect(page.getByText(/^Available 59\.3 GiB$/)).toBeVisible();
  await expect(
    page.getByRole("button", { name: "View results" }),
  ).toBeVisible();

  const sidebarLayout = await page.locator(".sidebar").evaluate((sidebar) => {
    const icons = [...sidebar.querySelectorAll("nav button > svg")].map(
      (icon) => icon.getBoundingClientRect().width,
    );
    const cleanupLabel = [
      ...sidebar.querySelectorAll("nav button > span"),
    ].find((label) => label.textContent === "Cleanup Suggestions");
    const subtitle = sidebar.querySelector(".brand span:not(.brand-icon)");
    return {
      clientWidth: sidebar.clientWidth,
      scrollWidth: sidebar.scrollWidth,
      icons,
      cleanupLabelHeight: cleanupLabel?.getBoundingClientRect().height ?? 0,
      subtitleHeight: subtitle?.getBoundingClientRect().height ?? 0,
    };
  });
  expect(sidebarLayout.scrollWidth).toBeLessThanOrEqual(
    sidebarLayout.clientWidth,
  );
  expect(Math.min(...sidebarLayout.icons)).toBeGreaterThanOrEqual(17.5);
  expect(sidebarLayout.cleanupLabelHeight).toBeGreaterThan(20);
  expect(sidebarLayout.subtitleHeight).toBeGreaterThan(20);

  await page
    .getByRole("button", { name: "Application Space", exact: true })
    .click();
  await expect(page.locator(".scan-summary")).toContainText("Total file size");
  await expect(page.locator(".scan-summary")).toContainText(
    "2 locations could not be scanned",
  );
  await expect(page.getByText("Estimated · Incomplete scan")).toBeVisible();
  await expect(page.getByText("Total 458 items")).toBeVisible();
});
