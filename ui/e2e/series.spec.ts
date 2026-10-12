import { expect, test } from "@playwright/test";
import { jpeg } from "./fixtures";
import { call, importFiles } from "./helpers";

type SeriesView = {
  photo_id: number;
  series_id: number | null;
  photo_ids: number[];
  edit: "split" | "join" | null;
};
type Page = { photos: { id: number; series_id: number | null }[] };

test("groups a burst into a series that can be split and rejoined", async ({
  request,
}) => {
  const at = (s: number) => new Date(`2003-05-05T10:00:0${s}`);
  // Identical pixels shot a second apart: one series. A shot an hour
  // later stays alone.
  await importFiles(request, {
    "series/a.JPG": { bytes: jpeg(9201), time: at(0) },
    "series/b.JPG": { bytes: jpeg(9202), time: at(1) },
    "series/c.JPG": { bytes: jpeg(9203), time: at(2) },
    "series/d.JPG": {
      bytes: jpeg(9204),
      time: new Date("2003-05-05T11:00:00"),
    },
  });
  const page = await call<Page>(request, "query_photos", {
    from: "2003-05-05",
    to: "2003-05-05",
    sort: "captured_asc",
  });
  const [a, b, c, d] = page.photos.map((p) => p.id);

  await expect
    .poll(
      async () =>
        (await call<SeriesView>(request, "photo_series", { photo_id: a }))
          .photo_ids,
      { timeout: 60_000 },
    )
    .toEqual([a, b, c]);
  const alone = await call<SeriesView>(request, "photo_series", {
    photo_id: d,
  });
  expect(alone.series_id).toBeNull();

  const split = await call<SeriesView>(request, "edit_series", {
    photo_id: c,
    edit: "split",
  });
  expect(split).toEqual({
    photo_id: c,
    series_id: null,
    photo_ids: [],
    edit: "split",
  });

  const joined = await call<SeriesView>(request, "edit_series", {
    photo_id: d,
    edit: "join",
  });
  expect(joined.photo_ids).toEqual([c, d]);

  await call(request, "edit_series", { photo_id: c, edit: null });
  await call(request, "edit_series", { photo_id: d, edit: null });
  const again = await call<SeriesView>(request, "photo_series", {
    photo_id: b,
  });
  expect(again.photo_ids).toEqual([a, b, c]);
});

test("series thresholds are adjustable in settings", async ({ page }) => {
  await page.goto("/");
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Import" })
    .click();

  await page.getByTestId("series-max-gap").fill("45");
  await page.getByTestId("save-settings").click();
  await expect(page.getByTestId("save-settings")).toBeDisabled();
  await page.reload();
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Import" })
    .click();
  await expect(page.getByTestId("series-max-gap")).toHaveValue("45");

  // Restore the default for other tests.
  await page.getByTestId("series-max-gap").fill("30");
  await page.getByTestId("save-settings").click();
  await expect(page.getByTestId("save-settings")).toBeDisabled();
});
