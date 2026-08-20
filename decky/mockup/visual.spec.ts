import { expect, test } from "@playwright/test";

test("captures Quick Access and Add PC states", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText("Gaming PC")).toBeVisible();
  await expect(page.getByText("Host port")).toBeVisible();
  await expect(page.getByRole("button", { name: "Back to Remote PCs" })).toBeVisible();
  const nameWidth = await page.getByLabel("Name").evaluate((element) => element.getBoundingClientRect().width);
  expect(nameWidth).toBeGreaterThan(300);
  await page.screenshot({ path: "../out/tests/decky-ui.png", fullPage: true });
});
