import { expect, test, type Page } from "@playwright/test";
import { jpeg } from "./fixtures";
import { call, importFiles } from "./helpers";

/** Near-identical gradients with different bytes, so they form a series. */
async function similar(page: Page, hue: number): Promise<Buffer> {
  const data = await page.evaluate((hue) => {
    const canvas = document.createElement("canvas");
    canvas.width = 3000;
    canvas.height = 2000;
    const ctx = canvas.getContext("2d")!;
    const g = ctx.createLinearGradient(0, 0, 3000, 2000);
    g.addColorStop(0, `hsl(${hue} 60% 30%)`);
    g.addColorStop(1, `hsl(${hue + 40} 60% 70%)`);
    ctx.fillStyle = g;
    ctx.fillRect(0, 0, 3000, 2000);
    ctx.fillStyle = "#fff";
    for (let x = 0; x < 3000; x += 150)
      for (let y = 0; y < 2000; y += 150) ctx.fillRect(x, y, 40, 40);
    return canvas.toDataURL("image/jpeg", 0.8).split(",")[1]!;
  }, hue);
  return Buffer.from(data, "base64");
}

type SeriesView = {
  photo_ids: number[];
  best_photo_id: number | null;
  ranking: { photo_id: number; score: number | null }[];
};

test("shows the series in a ribbon and the best photo beside the current one", async ({
  page,
  request,
}) => {
  const DAY = "2003-03-03";
  const at = (s: number) => new Date(`${DAY}T08:00:0${s}`);
  await page.goto("/");
  await importFiles(request, {
    "ribbon/a.JPG": { bytes: await similar(page, 120), time: at(0) },
    "ribbon/b.JPG": { bytes: await similar(page, 121), time: at(1) },
    "ribbon/c.JPG": { bytes: await similar(page, 122), time: at(2) },
  });
  const ids = (
    await call<{ photos: { id: number }[] }>(request, "query_photos", {
      from: DAY,
      to: DAY,
      sort: "captured_asc",
    })
  ).photos.map((p) => p.id);
  await expect
    .poll(
      async () =>
        (
          await call<SeriesView>(request, "photo_series", { photo_id: ids[0] })
        ).ranking.filter((r) => r.score !== null).length,
      { timeout: 60_000 },
    )
    .toBe(3);
  const best = (
    await call<SeriesView>(request, "photo_series", { photo_id: ids[0] })
  ).best_photo_id!;
  const other = ids.find((id) => id !== best)!;

  await page.reload();
  await page.getByTestId("filter-from").fill(DAY);
  await page.getByTestId("filter-to").fill(DAY);
  const timeline = page.getByTestId("timeline");
  await expect(timeline.locator("button.tile")).toHaveCount(3);
  await timeline.locator(`[data-photo-id="${other}"]`).dblclick();

  // The ribbon lists the series in capture order and marks the best.
  const ribbon = page.getByTestId("series-ribbon");
  await expect(ribbon).toBeVisible();
  await expect(page.getByTestId("series-ribbon-frame")).toHaveCount(3);
  await expect(page.getByTestId("series-ribbon-count")).toHaveText(
    `${ids.indexOf(other) + 1} of 3`,
  );
  await expect(page.getByTestId("series-ribbon-best")).toHaveCount(1);
  await expect(ribbon.locator("[aria-current=true]")).toHaveCount(1);
  await expect(page.getByTestId("loupe-rival")).toHaveCount(0);

  // The switch shows the best photo on the right.
  await page.getByTestId("loupe-compare-best").click();
  await expect(page.getByTestId("loupe-rival-label")).toContainText(
    "Best of the series",
  );
  await expect(page.getByTestId("loupe-rival-image")).toBeVisible();
  await page.screenshot({ path: test.info().outputPath("loupe-compare.png") });

  // Clicking a frame opens that photo; on the best one the runner-up shows.
  await ribbon.locator("button.frame").nth(ids.indexOf(best)).click();
  await expect(page.getByTestId("series-ribbon-count")).toHaveText(
    `${ids.indexOf(best) + 1} of 3`,
  );
  await expect(page.getByTestId("loupe-rival-label")).toContainText(
    "Runner-up",
  );

  // V switches it off and on; the choice survives closing the loupe.
  await page.keyboard.press("v");
  await expect(page.getByTestId("loupe-rival")).toHaveCount(0);
  await page.keyboard.press("v");
  await expect(page.getByTestId("loupe-rival")).toBeVisible();
  await page.keyboard.press("Escape");
  await timeline.locator(`[data-photo-id="${other}"]`).dblclick();
  await expect(page.getByTestId("loupe-rival")).toBeVisible();
  await expect(page.getByTestId("loupe-compare-best")).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await page.keyboard.press("v");
});

test("a photo in no series has no ribbon and no switch", async ({
  page,
  request,
}) => {
  await page.goto("/");
  await importFiles(request, {
    "ribbon/solo.JPG": {
      bytes: jpeg(9390),
      time: new Date("1999-09-09T09:09:09"),
    },
  });
  await page.reload();
  await page.getByTestId("filter-from").fill("1999-09-09");
  await page.getByTestId("filter-to").fill("1999-09-09");
  await page.getByTestId("timeline").locator("button.tile").first().dblclick();
  await expect(page.getByTestId("loupe")).toBeVisible();
  await expect(page.getByTestId("series-ribbon")).toHaveCount(0);
  await expect(page.getByTestId("loupe-compare-best")).toHaveCount(0);
});
