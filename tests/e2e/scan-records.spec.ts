import { expect, test } from "@playwright/test";
import { deletionCalls, openApp } from "./fixtures/backend";

test("each record has a delete action; cancel is safe and current deletion selects the remaining scan", async ({
  page,
}) => {
  await openApp(page);
  const picker = page.getByRole("button", { name: "扫描记录", exact: true });
  await picker.click();
  await expect(
    page.getByRole("button", { name: /^删除扫描记录：/ }),
  ).toHaveCount(2);
  await page
    .locator('[data-scan-id="current"]')
    .getByRole("button", { name: /^删除扫描记录：/ })
    .click();
  const dialog = page.getByRole("dialog", {
    name: "删除扫描记录",
    exact: true,
  });
  await expect(dialog).toContainText("原文件和回收操作历史不会删除");
  await expect(dialog).toContainText("D:\\测试资料");
  await expect.poll(() => deletionCalls(page)).toHaveLength(0);
  await dialog.getByRole("button", { name: "取消", exact: true }).click();
  await expect(dialog).not.toBeVisible();
  await expect(picker).toBeFocused();
  await picker.click();
  await page
    .locator('[data-scan-id="current"]')
    .getByRole("button", { name: /^删除扫描记录：/ })
    .click();
  await dialog.getByRole("button", { name: "确认删除", exact: true }).click();
  await expect(dialog).not.toBeVisible();
  await expect(picker).toContainText("D:\\旧扫描");
  await picker.click();
  await expect(page.locator('[data-scan-id="current"]')).toHaveCount(0);
  await page
    .locator('[data-scan-id="older"]')
    .getByRole("button", { name: /^删除扫描记录：/ })
    .click();
  await dialog.getByRole("button", { name: "确认删除", exact: true }).click();
  await expect(picker).toHaveCount(0);
  await expect.poll(() => deletionCalls(page)).toHaveLength(2);
});

test("deleting another record does not switch the selected scan", async ({
  page,
}) => {
  await page.clock.install();
  await openApp(page, { staleStatus: true });
  const picker = page.getByRole("button", { name: "扫描记录", exact: true });
  await picker.click();
  await page
    .locator('[data-scan-id="older"]')
    .getByRole("button", { name: /^删除扫描记录：/ })
    .click();
  await page.getByRole("button", { name: "确认删除", exact: true }).click();
  await expect(page.getByRole("dialog")).not.toBeVisible();
  await expect(picker).toContainText("D:\\测试资料");
  await page.clock.fastForward(5000);
  await picker.click();
  await expect(page.locator('[data-scan-id="older"]')).toHaveCount(0);
});

test("pending deletion submits once and cannot close midway", async ({
  page,
}) => {
  await openApp(page, { deleteDelay: 800 });
  await page.getByRole("button", { name: "扫描记录", exact: true }).click();
  await page
    .locator('[data-scan-id="older"]')
    .getByRole("button", { name: /^删除扫描记录：/ })
    .click();
  const dialog = page.getByRole("dialog");
  await dialog
    .getByRole("button", { name: "确认删除", exact: true })
    .click({ clickCount: 2 });
  await expect(
    dialog.getByRole("button", { name: "正在删除…", exact: true }),
  ).toBeDisabled();
  await expect(
    dialog.getByRole("button", { name: "取消", exact: true }),
  ).toBeDisabled();
  await page.keyboard.press("Escape");
  await expect(dialog).toBeVisible();
  await expect(dialog).not.toBeVisible();
  expect(await deletionCalls(page)).toHaveLength(1);
});

test("failed deletion leaves the record and displays an actionable error", async ({
  page,
}) => {
  await openApp(page, { deleteFails: true });
  await page.getByRole("button", { name: "扫描记录", exact: true }).click();
  await page
    .locator('[data-scan-id="current"]')
    .getByRole("button", { name: /^删除扫描记录：/ })
    .click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("button", { name: "确认删除", exact: true }).click();
  await expect(dialog.getByRole("alert")).toHaveText("模拟数据库写入失败");
  await dialog.getByRole("button", { name: "取消", exact: true }).click();
  await page.getByRole("button", { name: "扫描记录", exact: true }).click();
  await expect(page.locator('[data-scan-id="current"]')).toBeVisible();
});

test("active analysis disables every delete action", async ({ page }) => {
  await openApp(page, { active: true });
  await page.getByRole("button", { name: "扫描记录", exact: true }).click();
  for (const button of await page
    .getByRole("button", { name: /^删除扫描记录：/ })
    .all())
    await expect(button).toBeDisabled();
  expect(await deletionCalls(page)).toHaveLength(0);
});
