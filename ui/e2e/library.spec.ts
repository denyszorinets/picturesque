import { expect, test, type APIRequestContext } from "@playwright/test";
import { jpeg } from "./fixtures";
import { call, importFiles } from "./helpers";

const DAYS = ["2001-02-01", "2001-02-02", "2001-02-03", "2001-02-04"];
const PER_DAY = 60;

/** Imports PER_DAY tiny JPEGs for each day (dated by file time; no EXIF). */
async function importPhotos(request: APIRequestContext) {
  const files: Record<string, { bytes: Buffer; time: Date }> = {};
  DAYS.forEach((day, d) => {
    for (let i = 0; i < PER_DAY; i++) {
      files[`d${d}/IMG_${d}_${i}.JPG`] = {
        bytes: jpeg(1000 + d * PER_DAY + i),
        time: new Date(`${day}T12:00:${String(i % 60).padStart(2, "0")}`),
      };
    }
  });
  await importFiles(request, files);
}

test("shows the library as a virtualized timeline grouped by day", async ({
  page,
  request,
}) => {
  await importPhotos(request);
  const facets = await call<{ total: number }>(request, "photo_facets", null);

  await page.goto("/");

  await expect(page.getByTestId("library-count")).toHaveText(
    `${facets.total} photos`,
  );
  const headers = page.getByTestId("day-header");
  await expect(headers.first()).toBeVisible();
  // Thumbnails load from the dev server.
  const firstImage = page.locator("[data-photo-id] img").first();
  await expect
    .poll(() =>
      firstImage.evaluate((img: HTMLImageElement) => img.naturalWidth),
    )
    .toBeGreaterThan(0);
  // Only rows near the viewport are in the DOM.
  expect(await page.locator("[data-photo-id]").count()).toBeLessThan(
    facets.total,
  );

  // Scrolling through the timeline a screen at a time shows every day.
  const timeline = page.getByTestId("timeline");
  const seen = new Map<string, string>();
  for (let i = 0; i < 200; i++) {
    for (const header of await headers.all()) {
      const day = await header.getAttribute("data-day");
      if (day) seen.set(day, (await header.textContent()) ?? "");
    }
    const atEnd = await timeline.evaluate((el) => {
      el.scrollTop += el.clientHeight;
      return el.scrollTop + el.clientHeight >= el.scrollHeight - 1;
    });
    await page.waitForTimeout(50);
    if (atEnd && seen.has("2001-02-01")) break;
  }
  for (const day of DAYS) {
    expect(seen.get(day), day).toContain(`${PER_DAY} photos`);
  }
});

test("selects photos with click, shift-click and ctrl-click", async ({
  page,
}) => {
  await page.goto("/");
  const tiles = page.locator("[data-photo-id]");
  await expect(tiles.nth(5)).toBeVisible();

  await tiles.nth(0).click();
  await expect(page.getByTestId("selection-count")).toHaveText("1 selected");
  await tiles.nth(3).click({ modifiers: ["Shift"] });
  await expect(page.getByTestId("selection-count")).toHaveText("4 selected");
  await tiles.nth(5).click({ modifiers: ["ControlOrMeta"] });
  await expect(page.getByTestId("selection-count")).toHaveText("5 selected");
  await expect(tiles.nth(2)).toHaveAttribute("aria-pressed", "true");
});
