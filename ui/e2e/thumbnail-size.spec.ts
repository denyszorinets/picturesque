import { expect, test } from "@playwright/test";
import { jpeg } from "./fixtures";
import { importFiles } from "./helpers";

test("resizes the virtualized grid, preserves selection and remembers the size", async ({
  page,
  request,
}) => {
  const day = "2095-06-07";
  await importFiles(
    request,
    Object.fromEntries(
      Array.from({ length: 120 }, (_, i) => [
        `resize/${i}.JPG`,
        { bytes: jpeg(93000 + i), time: new Date(`${day}T10:00:00`) },
      ]),
    ),
  );
  await page.goto("/");
  await page.getByTestId("filter-from").fill(day);
  await page.getByTestId("filter-to").fill(day);
  const timeline = page.getByTestId("timeline");
  const tiles = timeline.locator("button.tile");
  await expect(tiles.first()).toBeVisible();
  await tiles.first().click();
  const selected = await tiles.first().getAttribute("data-photo-id");
  const before = (await tiles.first().boundingBox())!.width;
  const slider = page.getByRole("slider", { name: "Thumbnail size" });
  await slider.focus();
  await page.keyboard.press("End");
  await expect(slider).toHaveValue("360");
  await expect
    .poll(async () => (await tiles.first().boundingBox())!.width)
    .toBeGreaterThan(before);
  await expect(
    timeline.locator(`button.tile[data-photo-id="${selected}"]`),
  ).toHaveAttribute("aria-pressed", "true");
  expect(await tiles.count()).toBeLessThan(120);
  await tiles.first().click();
  await page.keyboard.press("ArrowRight");
  await expect(tiles.nth(1)).toHaveAttribute("aria-pressed", "true");
  await page.screenshot({
    path: test.info().outputPath("large-thumbnails.png"),
  });
  await page.reload();
  await expect(slider).toHaveValue("360");
  await slider.focus();
  await page.keyboard.press("Home");
  await expect(slider).toHaveValue("120");
});
