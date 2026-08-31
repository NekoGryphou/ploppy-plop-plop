import { expect, test } from "@playwright/test";

test("captures Quick Access and Add PC states", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText("Gaming PC")).toBeVisible();
  await expect(page.getByText("Host port")).toBeVisible();
  await expect(page.getByRole("button", { name: "Back to PCs" })).toBeVisible();
  const nameInput = page.getByLabel("Name");
  const nameLabel = page.getByText("Name", { exact: true });
  const [inputBox, labelBox, panelBox] = await Promise.all([
    nameInput.boundingBox(),
    nameLabel.boundingBox(),
    page.locator(".settings").boundingBox()
  ]);
  expect(inputBox).not.toBeNull();
  expect(labelBox).not.toBeNull();
  expect(panelBox).not.toBeNull();
  if (!inputBox || !labelBox || !panelBox) {
    throw new Error("The Add PC form must have measurable input, label, and panel bounds.");
  }
  expect(inputBox.width).toBeGreaterThan(240);
  expect(Math.abs(inputBox.y - labelBox.y)).toBeLessThan(20);
  expect(inputBox.x + inputBox.width).toBeLessThanOrEqual(panelBox.x + panelBox.width);
  await page.screenshot({ path: "../out/tests/decky-ui.png", fullPage: true });
});
