import type { Server } from "node:http";
import { expect, test } from "@playwright/test";
import { jpeg } from "./fixtures";
import { call, importFiles, serveModels } from "./helpers";

type Status = { analyzed: number; total: number };
type Page = {
  total: number;
  photos: { id: number; aesthetic: number | null; faces: number }[];
};

let server: Server;

test.beforeAll(async () => {
  server = await serveModels();
});

test.afterAll(() => {
  server.close();
});

test("filters by AI signals and explains scores", async ({ page, request }) => {
  const DAY = "2009-03-03";
  await importFiles(request, {
    "ai/a.JPG": { bytes: jpeg(9601), time: new Date(`${DAY}T10:00:00`) },
    "ai/b.JPG": { bytes: jpeg(9602), time: new Date(`${DAY}T12:00:00`) },
  });
  await call(request, "install_model", { id: "clip-image" });
  await expect
    .poll(
      async () => {
        const s = await call<Status>(request, "analysis_status", null);
        const day = await call<Page>(request, "query_photos", {
          from: DAY,
          to: DAY,
        });
        return (
          s.analyzed === s.total &&
          day.photos.every((p) => p.aesthetic !== null)
        );
      },
      { timeout: 60_000 },
    )
    .toBe(true);

  await page.goto("/");
  await page.getByTestId("filter-from").fill(DAY);
  await page.getByTestId("filter-to").fill(DAY);
  const count = page.getByTestId("library-count");
  await expect(count).toHaveText("2 photos");

  // The aesthetic threshold matches the query API.
  await page.getByTestId("min-aesthetic-50").click();
  const expected = (
    await call<Page>(request, "query_photos", {
      from: DAY,
      to: DAY,
      min_aesthetic: 0.5,
    })
  ).total;
  await expect(count).toHaveText(
    `${expected} ${expected === 1 ? "photo" : "photos"}`,
  );
  await page.getByTestId("min-aesthetic-0").click();

  // No faces in these photos: "without people" keeps both, "with" none.
  await page.getByTestId("filter-people-true").click();
  await expect(page.getByTestId("no-match")).toBeVisible();
  await page.getByTestId("filter-people-false").click();
  await expect(count).toHaveText("2 photos");
  await expect(
    page.getByRole("button", { name: "Remove filter Without people" }),
  ).toBeVisible();
  await page.getByTestId("filter-people-null").click();
  await page.getByTestId("filter-hide-closed-eyes").check();
  await expect(count).toHaveText("2 photos");
  await page.getByTestId("filter-hide-closed-eyes").uncheck();

  // The loupe shows the combined score.
  await page.getByTestId("timeline").locator("button.tile").first().dblclick();
  await expect(page.getByTestId("loupe-score")).toBeVisible();
  await page.keyboard.press("Escape");

  await call(request, "remove_model", { id: "clip-image" });
});
