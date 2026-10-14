import { expect, test } from "@playwright/test";
import { call, drawn, importFiles } from "./helpers";

type Summary = {
  id: number;
  quality: number | null;
  keeper: boolean;
};

test("auto-picks keepers and turns them into picks", async ({
  page,
  request,
}) => {
  const DAY = "2008-08-08";
  await page.goto("/");
  await importFiles(request, {
    "auto/sharp.JPG": {
      bytes: await drawn(page, 0, 60),
      time: new Date(`${DAY}T10:00:00`),
    },
    "auto/soft.JPG": {
      bytes: await drawn(page, 12, 60),
      time: new Date(`${DAY}T10:00:02`),
    },
    "auto/other.JPG": {
      bytes: await drawn(page, 0, 260),
      time: new Date(`${DAY}T16:00:00`),
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
  await call(request, "edit_series", { photo_id: soft!.id, edit: "join" });
  // Whatever these drawings score, every good-enough photo counts.
  await call(request, "update_settings", {
    auto_pick: { enabled: true, min_quality: 0 },
  });
  // The best of the series and the single photo are keepers.
  const auto = (await photos()).filter((p) => p.keeper).map((p) => p.id);
  expect(auto.sort()).toEqual([sharp!.id, other!.id].sort());

  await page.reload();
  await page.getByTestId("filter-from").fill(DAY);
  await page.getByTestId("filter-to").fill(DAY);
  const tiles = page.getByTestId("timeline").locator("button.tile");
  await expect(tiles).toHaveCount(3);
  await expect(page.getByTestId("tile-auto-picked")).toHaveCount(2);

  // The filter shows only them.
  await page.getByTestId("filter-auto-picked").click();
  await expect(tiles).toHaveCount(2);

  // Rejecting one replaces the suggestion: it leaves the view.
  await tiles.first().click();
  await page.keyboard.press("x");
  await expect(tiles).toHaveCount(1);
  await expect(page.getByTestId("tile-auto-picked")).toHaveCount(1);
  await page.keyboard.press("Control+z");
  await expect(page.getByTestId("tile-auto-picked")).toHaveCount(2);

  // Accepting turns every auto-pick (in the whole library) into a pick.
  await page.getByTestId("accept-auto-picks").click();
  await expect(tiles).toHaveCount(0);
  await page.getByTestId("filter-auto-picked").click();
  await expect(tiles).toHaveCount(3);
  await expect(tiles.locator(".badge.pick")).toHaveCount(2);
  await expect(page.getByTestId("tile-auto-picked")).toHaveCount(0);
  await page.screenshot({ path: test.info().outputPath("auto-picked.png") });
  // And Undo takes it back.
  await page.keyboard.press("Control+z");
  await expect(page.getByTestId("tile-auto-picked")).toHaveCount(2);

  // Leave the shared library as it was.
  await page.getByTestId("clear-filters").click();
  await call(request, "update_settings", {
    auto_pick: { enabled: true, min_quality: 0.5 },
  });
  await call(request, "edit_series", { photo_id: soft!.id, edit: null });
});
