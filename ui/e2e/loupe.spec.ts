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

test("culls in the loupe with buttons, keys and auto-advance", async ({
  page,
  request,
}) => {
  await importFiles(request, {
    "cull/a.JPG": { bytes: jpeg(9101), time: new Date("2003-04-04T10:00:00") },
    "cull/b.JPG": { bytes: jpeg(9102), time: new Date("2003-04-04T10:00:01") },
    "cull/c.JPG": { bytes: jpeg(9103), time: new Date("2003-04-04T10:00:02") },
  });
  await page.goto("/");
  await page.locator("[data-photo-id]").first().dblclick();
  const position = page.getByTestId("loupe-position");
  await expect(position).toHaveText(/^1 \/ \d+$/);
  const pick = page.getByTestId("loupe-pick");
  const reject = page.getByTestId("loupe-reject");

  // Buttons show and change the photo's decision.
  await pick.click();
  await expect(pick).toHaveAttribute("aria-pressed", "true");
  await page.getByRole("button", { name: "3 stars" }).click();
  await expect(page.getByRole("button", { name: "3 stars" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  // Keys still work after clicking a button.
  await page.keyboard.press("x");
  await expect(reject).toHaveAttribute("aria-pressed", "true");
  await expect(position).toHaveText(/^1 \/ \d+$/);

  // Shift moves on after the decision.
  await page.keyboard.press("Shift+P");
  await expect(position).toHaveText(/^2 \/ \d+$/);
  await page.keyboard.press("ArrowLeft");
  await expect(pick).toHaveAttribute("aria-pressed", "true");

  // Auto-advance: every decision moves on.
  await page.getByTestId("loupe-auto-advance").check();
  await page.keyboard.press("x");
  await expect(position).toHaveText(/^2 \/ \d+$/);
  await page.keyboard.press("ArrowLeft");
  await expect(reject).toHaveAttribute("aria-pressed", "true");
  await page.getByTestId("loupe-unflag").click();
  await expect(position).toHaveText(/^2 \/ \d+$/);
  await page.keyboard.press("ArrowLeft");
  await expect(pick).toHaveAttribute("aria-pressed", "false");
  await expect(reject).toHaveAttribute("aria-pressed", "false");

  // Leave the shared library as it was.
  await page.getByTestId("loupe-auto-advance").uncheck();
  await page.keyboard.press("0");
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("u");
});
