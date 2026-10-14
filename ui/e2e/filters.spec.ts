import { expect, test } from "@playwright/test";
import { jpeg } from "./fixtures";
import { call, importFiles } from "./helpers";

const DAY = "2003-04-05";

test("filters the timeline and keeps filters for the session", async ({
  page,
  request,
}) => {
  await importFiles(
    request,
    Object.fromEntries(
      [0, 1, 2, 3, 4].map((i) => [
        `filter/IMG_${i}.JPG`,
        { bytes: jpeg(8000 + i), time: new Date(`${DAY}T09:00:0${i}`) },
      ]),
    ),
  );
  // Pick two of today's photos through the API.
  const ours = await call<{ photos: { id: number }[] }>(
    request,
    "query_photos",
    { from: DAY, to: DAY },
  );
  expect(ours.photos).toHaveLength(5);
  await call(request, "set_decisions", {
    changes: ours.photos
      .slice(0, 2)
      .map((p) => ({ photo_id: p.id, flag: "pick" })),
  });
  const expected = async (query: object) =>
    (
      await call<{ total: number }>(request, "query_photos", {
        ...query,
        limit: 1,
      })
    ).total;
  const all = await expected({});
  const picks = await expected({ flags: ["pick"] });

  await page.goto("/");
  const count = page.getByTestId("library-count");
  await expect(count).toHaveText(`${all} photos`);

  // Flag filter.
  await page
    .getByTestId("filter-panel")
    .getByRole("button", { name: /^Picks/ })
    .click();
  await expect(count).toHaveText(`${picks} photos`);
  await expect(
    page.getByRole("button", { name: "Remove filter Picks" }),
  ).toBeVisible();

  // Date range narrows further.
  await page.getByTestId("filter-from").fill(DAY);
  await page.getByTestId("filter-to").fill(DAY);
  await expect(count).toHaveText("2 photos");

  // Filters survive a reload in the same session.
  await page.reload();
  await expect(count).toHaveText("2 photos");

  // Removing one chip, then clearing everything.
  await page.getByRole("button", { name: "Remove filter Picks" }).click();
  await expect(count).toHaveText("5 photos");
  await page.getByTestId("clear-filters").click();
  await expect(count).toHaveText(`${all} photos`);

  // A filter nothing matches.
  await page.getByTestId("filter-from").fill("1990-01-01");
  await page.getByTestId("filter-to").fill("1990-01-02");
  await expect(page.getByTestId("no-match")).toBeVisible();
  await page.getByTestId("clear-filters").click();
});

test("shows the filtered photos right away after scrolling down", async ({
  page,
  request,
}) => {
  // Many days, so the timeline is much taller than the window.
  const days = Array.from({ length: 40 }, (_, i) => {
    const d = new Date(Date.UTC(1999, 0, 1 + i));
    return d.toISOString().slice(0, 10);
  });
  await importFiles(
    request,
    Object.fromEntries(
      days.map((day, i) => [
        `scrolled/IMG_${i}.JPG`,
        { bytes: jpeg(9000 + i), time: new Date(`${day}T10:00:00`) },
      ]),
    ),
  );

  await page.setViewportSize({ width: 1280, height: 720 });
  await page.goto("/");
  const timeline = page.getByTestId("timeline");
  await expect(timeline.locator("button.tile").first()).toBeVisible();
  // Scroll far down, past where the filtered result will end.
  await timeline.evaluate((el) => (el.scrollTop = el.scrollHeight));
  await expect(
    page.locator("[data-testid=day-header][data-day='1999-01-01']"),
  ).toBeVisible();

  // The newest day only: the timeline gets much shorter.
  const last = days[days.length - 1]!;
  await page.getByTestId("filter-from").fill(last);
  await page.getByTestId("filter-to").fill(last);
  await expect(page.getByTestId("library-count")).toHaveText("1 photo");
  // Without scrolling, the photo is shown.
  await expect(timeline.locator("button.tile")).toHaveCount(1);
  await expect(timeline.locator("button.tile")).toBeInViewport();
  await page.getByTestId("clear-filters").click();
});

test("never shows a previous photo's thumbnail after filtering", async ({
  page,
  request,
}) => {
  const day = "2002-06-07";
  await importFiles(
    request,
    Object.fromEntries(
      [0, 1, 2, 3, 4, 5].map((i) => [
        `stale/IMG_${i}.JPG`,
        { bytes: jpeg(9500 + i), time: new Date(`${day}T08:00:0${i}`) },
      ]),
    ),
  );
  const ours = await call<{ photos: { id: number }[] }>(
    request,
    "query_photos",
    { from: day, to: day },
  );
  // Picks that are not the first photos of the day: under the Picks
  // filter they move to other places in the grid.
  const picked = ours.photos.slice(3).map((p) => p.id);
  await call(request, "set_decisions", {
    changes: picked.map((photo_id) => ({ photo_id, flag: "pick" })),
  });

  await page.goto("/");
  await page.getByTestId("filter-from").fill(day);
  await page.getByTestId("filter-to").fill(day);
  const tiles = page.getByTestId("timeline").locator("button.tile");
  await expect(tiles).toHaveCount(6);
  await expect
    .poll(() =>
      tiles.evaluateAll((all) =>
        all.every((t) => t.querySelector("img")?.complete),
      ),
    )
    .toBe(true);

  // Browsers keep showing an <img>'s previous picture until a new src has
  // loaded (WebKitGTK does, until the thumbnail is created), so a tile must
  // not reuse the image of a different photo.
  await tiles.evaluateAll((all) =>
    all.forEach((t) => {
      t.querySelector("img")!.dataset.shown = t.getAttribute("data-photo-id")!;
    }),
  );
  await page
    .getByTestId("filter-panel")
    .getByRole("button", { name: /^Picks/ })
    .click();
  await expect(tiles).toHaveCount(3);
  const shown = await tiles.evaluateAll((all) =>
    all.map((t) => [
      t.getAttribute("data-photo-id"),
      t.querySelector("img")!.dataset.shown ?? null,
    ]),
  );
  for (const [id, before] of shown)
    expect(
      before === null || before === id,
      `tile ${id} showed ${before}`,
    ).toBe(true);
  await page.getByTestId("clear-filters").click();
});
