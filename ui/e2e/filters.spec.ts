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
