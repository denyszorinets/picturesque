import { expect, test } from "@playwright/test";
import { call, drawn, importFiles } from "./helpers";

type Summary = { id: number; quality: number | null; series_id: number | null };

test("filters and sorts by quality, and shows only the best of series", async ({
  page,
  request,
}) => {
  const DAY = "2007-09-09";
  await page.goto("/");
  await importFiles(request, {
    "quality/sharp.JPG": {
      bytes: await drawn(page, 0, 30),
      time: new Date(`${DAY}T10:00:00`),
    },
    "quality/soft.JPG": {
      bytes: await drawn(page, 12, 30),
      time: new Date(`${DAY}T10:00:02`),
    },
    "quality/other.JPG": {
      bytes: await drawn(page, 0, 200),
      time: new Date(`${DAY}T15:00:00`),
    },
  });
  const photos = async () =>
    (
      await call<{ photos: Summary[] }>(request, "query_photos", {
        from: DAY,
        to: DAY,
        sort: "captured_asc",
      })
    ).photos;
  await expect
    .poll(async () => (await photos()).every((p) => p.quality !== null), {
      timeout: 60_000,
    })
    .toBe(true);
  const [sharp, soft, other] = await photos();
  // Blurring changes the perceptual hash a lot: make sure the two shots
  // form a series.
  await call(request, "edit_series", { photo_id: soft!.id, edit: "join" });
  expect(sharp!.quality!).toBeGreaterThan(soft!.quality!);

  await page.reload();
  await page.getByTestId("filter-from").fill(DAY);
  await page.getByTestId("filter-to").fill(DAY);
  const tiles = page.getByTestId("timeline").locator("button.tile");
  await expect(tiles).toHaveCount(3);

  // Best quality first, with scores on the tiles.
  await page.getByTestId("sort").selectOption("quality_desc");
  await expect(page.getByTestId("day-header")).toHaveText(/Best quality first/);
  await expect(tiles.last()).toHaveAttribute("data-photo-id", String(soft!.id));
  await expect(page.getByTestId("tile-quality")).toHaveCount(3);
  await page.screenshot({ path: test.info().outputPath("quality-sort.png") });

  // A sharpness threshold drops the blurred shot.
  await page.getByTestId("min-sharpness-50").click();
  await expect(tiles).toHaveCount(2);
  await expect(
    page.getByRole("button", { name: "Remove filter Sharpness ≥ 50%" }),
  ).toBeVisible();
  await page.getByTestId("min-sharpness-0").click();
  await expect(tiles).toHaveCount(3);

  // Best only keeps the sharp shot of the series and the single photo.
  await page.getByTestId("filter-best-only").check();
  await expect(tiles).toHaveCount(2);
  const shown = await tiles.evaluateAll((els) =>
    els.map((e) => Number(e.getAttribute("data-photo-id"))),
  );
  expect(shown.sort()).toEqual([sharp!.id, other!.id].sort());

  await page.getByTestId("clear-filters").click();
  await page.getByTestId("sort").selectOption("captured_desc");
  await expect(page.getByTestId("day-header").first()).not.toHaveText(
    /Best quality/,
  );
  await call(request, "edit_series", { photo_id: soft!.id, edit: null });
});
