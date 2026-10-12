import { expect, test } from "@playwright/test";
import { jpeg } from "./fixtures";
import { call, importFiles } from "./helpers";

type Status = { analyzed: number; total: number; running: boolean };

test("analyzes photos in the background and shows their quality", async ({
  page,
  request,
}) => {
  await importFiles(request, {
    "analysis/a.JPG": { bytes: jpeg(9101) },
    "analysis/b.JPG": { bytes: jpeg(9102) },
  });

  await expect
    .poll(
      async () => {
        const s = await call<Status>(request, "analysis_status", null);
        return s.total > 0 && s.analyzed === s.total;
      },
      { timeout: 60_000 },
    )
    .toBe(true);

  await page.goto("/");
  await expect(page.getByTestId("analysis-progress")).toBeHidden();
  await page.locator("[data-photo-id]").first().dblclick();
  await expect(page.getByTestId("loupe-quality")).toContainText(
    /Sharpness \d+% · Exposure \d+% · Low noise \d+%/,
  );
});

test("accepts priorities and rejects bad arguments", async ({ request }) => {
  const status = await call<Status>(request, "analysis_status", null);
  const after = await call<Status>(request, "prioritize_analysis", {
    photo_ids: [1, 2, 3],
  });
  expect(after.total).toBe(status.total);

  const bad = await request.post("/api/commands/prioritize_analysis", {
    data: { photo_ids: "all" },
  });
  expect(bad.status()).toBe(400);
});
