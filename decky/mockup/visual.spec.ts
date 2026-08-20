import { expect, test } from "@playwright/test";

test("captures Quick Access and Add PC states", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText("Gaming PC")).toBeVisible();
  await expect(page.getByText("Host port")).toBeVisible();
  await page.screenshot({ path: "../out/ui/decky-ui.png", fullPage: true });
});
