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
  await openApp(page);
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
});
