import { expect, test } from "@playwright/test";
import { jpeg } from "./fixtures";
import { importFiles } from "./helpers";

test("opens photos in the loupe and works from the keyboard", async ({
  page,
  request,
}) => {
  await importFiles(request, {
    "loupe/a.JPG": { bytes: jpeg(9001), time: new Date("2002-03-03T10:00:00") },
    "loupe/b.JPG": { bytes: jpeg(9002), time: new Date("2002-03-03T10:00:01") },
  });
  await page.goto("/");
  const tile = page.locator("[data-photo-id]").first();
  await tile.click();

  await page.keyboard.press("Enter");
  const loupe = page.getByTestId("loupe");
  await expect(loupe).toBeVisible();
  await expect(page.getByTestId("loupe-position")).toHaveText(/^1 \/ \d+$/);
  await expect(page.getByTestId("loupe-info")).toContainText("Files");
  const fitted = page.getByTestId("loupe-image");
  await expect
    .poll(() => fitted.evaluate((img: HTMLImageElement) => img.naturalWidth))
    .toBeGreaterThan(0);

  await page.keyboard.press("ArrowRight");
  await expect(page.getByTestId("loupe-position")).toHaveText(/^2 \/ \d+$/);
  await page.keyboard.press("ArrowLeft");
  await expect(page.getByTestId("loupe-position")).toHaveText(/^1 \/ \d+$/);

  await page.keyboard.press("i");
  await expect(page.getByTestId("loupe-info")).toBeHidden();

  // 100% zoom loads the full-size image.
  await page.keyboard.press("z");
  const full = page.getByTestId("loupe-zoomed").locator("img");
  await expect
    .poll(() => full.evaluate((img: HTMLImageElement) => img.naturalWidth))
    .toBeGreaterThan(0);
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("loupe-zoomed")).toBeHidden();

  await page.keyboard.press("Escape");
  await expect(loupe).toBeHidden();
  await expect(tile).toHaveAttribute("aria-pressed", "true");
});
