import { expect, test } from "@playwright/test";
import { jpeg } from "./fixtures";
import { call, importFiles } from "./helpers";

type SeriesView = {
  photo_id: number;
  series_id: number | null;
  photo_ids: number[];
  edit: "split" | "join" | null;
  best_photo_id: number | null;
  best_chosen: boolean;
  ranking: { photo_id: number; score: number | null }[];
};
type Page = {
  photos: { id: number; series_id: number | null; best_in_series: boolean }[];
};

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

  // Identical photos tie on score; the earliest wins until the user
  // chooses another.
  await expect
    .poll(
      async () =>
        (
          await call<SeriesView>(request, "photo_series", { photo_id: a })
        ).ranking.every((r) => r.score !== null),
      { timeout: 60_000 },
    )
    .toBe(true);
  const first = await call<SeriesView>(request, "photo_series", {
    photo_id: a,
  });
  expect(first.ranking.map((r) => r.photo_id)).toEqual([a, b, c]);
  expect(first.best_photo_id).toBe(a);
  const chosen = await call<SeriesView>(request, "choose_best", {
    photo_id: c,
    best: true,
  });
  expect(chosen.best_photo_id).toBe(c);
  expect(chosen.best_chosen).toBe(true);
  const marked = await call<Page>(request, "query_photos", {
    from: "2003-05-05",
    to: "2003-05-05",
    sort: "captured_asc",
  });
  expect(marked.photos.map((p) => p.best_in_series)).toEqual([
    false,
    false,
    true,
    false,
  ]);
  await call(request, "choose_best", { photo_id: c, best: false });

  const split = await call<SeriesView>(request, "edit_series", {
    photo_id: c,
    edit: "split",
  });
  expect(split).toMatchObject({
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
