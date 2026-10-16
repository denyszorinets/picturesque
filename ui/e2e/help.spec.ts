import { expect, test } from "@playwright/test";

test("shows keyboard shortcuts and opens the guides", async ({ page }) => {
  await page.goto("/");
  // A first launch shows the setup wizard: close it.
  const wizard = page.getByTestId("setup-wizard");
  if (await wizard.isVisible()) await page.keyboard.press("Escape");

  await page.keyboard.press("?");
  const help = page.getByTestId("help-dialog");
  await expect(help).toBeVisible();
  await expect(help.getByTestId("shortcuts-Culling")).toContainText("Pick");
  await expect(help.getByTestId("shortcuts-Compare")).toContainText(
    "Show 2 or 4 photos",
  );

  // Guides open in a new browser page (the system browser in the app).
  const popup = page.waitForEvent("popup");
  await help.getByRole("link", { name: "Your first cull" }).click();
  expect((await popup).url()).toContain("/getting-started/first-cull.html");

  await page.keyboard.press("Escape");
  await expect(help).toBeHidden();
  await page.getByTestId("open-help").click();
  await expect(help).toBeVisible();
});
