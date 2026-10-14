import { expect, test, type Page } from "@playwright/test";
import { call, importFiles } from "./helpers";

type SeriesView = { ranking: { photo_id: number; score: number | null }[] };

/** A large JPEG drawn in the browser, so zoomed panes can scroll. */
async function bigJpeg(page: Page, hue: number): Promise<Buffer> {
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

test("compares a series side by side with the keyboard", async ({
  page,
  request,
}) => {
  const DAY = "2005-07-07";
  await page.goto("/");
  const at = (s: number) => new Date(`${DAY}T08:00:0${s}`);
  await importFiles(request, {
    "compare/a.JPG": { bytes: await bigJpeg(page, 200), time: at(0) },
    "compare/b.JPG": { bytes: await bigJpeg(page, 201), time: at(1) },
    "compare/c.JPG": { bytes: await bigJpeg(page, 202), time: at(2) },
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

  await page.reload();
  await page.getByTestId("filter-from").fill(DAY);
  await page.getByTestId("filter-to").fill(DAY);
  const timeline = page.getByTestId("timeline");
  await expect(timeline.locator("button.tile")).toHaveCount(3);

  // Select one photo of the series and compare: its series, 4-up.
  await timeline.locator(`[data-photo-id="${ids[1]}"]`).click();
  await page.keyboard.press("c");
  const view = page.getByTestId("compare");
  await expect(view).toBeVisible();
  const panes = view.getByTestId("compare-pane");
  await expect(panes).toHaveCount(3);
  await expect(view.locator(".panes")).toHaveAttribute("data-panes", "4");
  await expect(view.locator('[data-active="true"]')).toHaveAttribute(
    "data-photo-id",
    String(ids[1]),
  );
  await page.screenshot({ path: test.info().outputPath("compare-4up.png") });

  // 2-up, move to the next photo, reject it and rate it.
  await page.keyboard.press("l");
  await expect(panes).toHaveCount(2);
  await page.keyboard.press("End");
  const active = view.locator('[data-active="true"]');
  const activeId = Number(await active.getAttribute("data-photo-id"));
  await page.keyboard.press("x");
  await page.keyboard.press("3");
  await expect(active).toContainText("Rejected");
  await expect(active).toContainText("★★★");
  // The best photo can be chosen from compare too.
  await page.keyboard.press("Home");
  await page.keyboard.press("b");
  await expect(view.locator('[data-active="true"]')).toContainText("★ Best");

  // Zoom and pan stay in sync across panes.
  await page.keyboard.press("z");
  const zoomed = view.getByTestId("compare-zoomed");
  await expect(zoomed).toHaveCount(2);
  await expect(zoomed.locator("img.full.hidden")).toHaveCount(0);
  await page.keyboard.press("Shift+ArrowRight");
  await page.keyboard.press("Shift+ArrowDown");
  const scrolls = () =>
    zoomed.evaluateAll((els) => els.map((e) => [e.scrollLeft, e.scrollTop]));
  await expect.poll(scrolls).not.toEqual([
    [0, 0],
    [0, 0],
  ]);
  const [first, second] = await scrolls();
  expect(first).toEqual(second);
  expect(first![0]).toBeGreaterThan(0);
  await page.screenshot({ path: test.info().outputPath("compare-zoomed.png") });

  // Esc leaves zoom, then compare; the decisions show in the grid.
  await page.keyboard.press("Escape");
  await expect(zoomed).toHaveCount(0);
  await page.keyboard.press("Escape");
  await expect(view).toHaveCount(0);
  await expect(
    timeline.locator(`[data-photo-id="${activeId}"]`).locator(".badge.reject"),
  ).toBeVisible();
  // Ctrl+Z works from the grid for changes made in compare.
  await page.keyboard.press("Control+z");
  await page.keyboard.press("Control+z");
  await expect(
    timeline.locator(`[data-photo-id="${activeId}"]`).locator(".badge.reject"),
  ).toHaveCount(0);
  await call(request, "choose_best", { photo_id: ids[0], best: false });
});

test("compares the selected photos", async ({ page }) => {
  await page.goto("/");
  const timeline = page.getByTestId("timeline");
  const tiles = timeline.locator("button.tile");
  await expect(tiles.first()).toBeVisible();
  await tiles.nth(0).click();
  await tiles.nth(1).click({ modifiers: ["Control"] });
  await page.keyboard.press("c");
  await expect(page.getByTestId("compare-pane")).toHaveCount(2);
  await expect(page.getByTestId("compare-position")).toHaveText(/\/ 2$/);
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("compare")).toHaveCount(0);
});
