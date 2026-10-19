import { expect, test } from "@playwright/test";
import { jpeg } from "./fixtures";
import { call, importFiles } from "./helpers";

type Status = {
  analyzed: number;
  total: number;
  running: boolean;
  waiting_for_models: string[];
};

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
  await expect(page.getByTestId("activity-ring-label")).toHaveText(
    "Up to date",
  );
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

test("analyzes selected photos, or all of them, again", async ({
  page,
  request,
}) => {
  const day = "2006-02-03";
  await importFiles(request, {
    "reanalyze/a.JPG": {
      bytes: jpeg(14001),
      time: new Date(`${day}T08:00:00`),
    },
    "reanalyze/b.JPG": {
      bytes: jpeg(14002),
      time: new Date(`${day}T09:00:00`),
    },
  });
  const done = () =>
    expect
      .poll(
        async () => {
          const s = await call<Status>(request, "analysis_status", null);
          return s.total > 0 && s.analyzed === s.total;
        },
        { timeout: 120_000 },
      )
      .toBe(true);
  await done();

  await page.goto("/");
  await page.getByTestId("filter-from").fill(day);
  await page.getByTestId("filter-to").fill(day);
  const reanalyze = page.getByRole("button", {
    name: "Re-analyze",
    exact: true,
  });
  // Nothing selected yet.
  await expect(reanalyze).toBeDisabled();
  await page.locator("button.tile").first().click();
  const response = page.waitForResponse("**/api/commands/reanalyze_photos");
  await reanalyze.click();
  expect(await (await response).json()).toMatchObject({ photos: 1 });
  await done();
  // The photo keeps (or regains) its scores.
  await page.locator("button.tile").first().dblclick();
  await expect(page.getByTestId("loupe-quality")).toContainText(/Sharpness/);
  await page.keyboard.press("Escape");

  // The whole library, from Settings, after confirming.
  const total = (await call<Status>(request, "analysis_status", null)).total;
  await page
    .getByRole("navigation", { name: "Main" })
    .getByRole("button", { name: "Settings" })
    .click();
  // The library view stays mounted: look only in Settings.
  const settings = page.getByRole("region", { name: "Settings" });
  await settings.getByRole("button", { name: "Re-analyze all photos" }).click();
  await expect(settings.getByTestId("reanalyze-confirm")).toBeVisible();
  await settings.getByTestId("reanalyze-start").click();
  await expect(settings.getByTestId("reanalyze-message")).toHaveText(
    `Analyzing ${total} photos again.`,
  );
  await done();
});

test("re-analysis rejects bad arguments", async ({ request }) => {
  const bad = await request.post("/api/commands/reanalyze_photos", {
    data: { photo_ids: "all" },
  });
  expect(bad.status()).toBe(400);
  const none = await call<{ photos: number }>(request, "reanalyze_photos", {
    photo_ids: [987654],
  });
  expect(none.photos).toBe(0);
});

test("says AI analysis needs models while they are not downloaded", async ({
  page,
  request,
}) => {
  // Other tests download and remove models: go by the current status.
  const { waiting_for_models: waiting } = await call<Status>(
    request,
    "analysis_status",
    null,
  );
  expect(waiting).toContain("faces and closed eyes");

  await page.goto("/");
  const note = page.getByTestId("analysis-needs-models");
  await expect(note).toHaveText("AI analysis needs models");
  await expect(note).toHaveAttribute("title", /faces and closed eyes/);
  await note.click();
  const settings = page.getByRole("region", { name: "Settings" });
  await expect(settings).toBeVisible();

  await settings.getByRole("button", { name: "Re-analyze all photos" }).click();
  await settings.getByTestId("reanalyze-start").click();
  await expect(settings.getByTestId("reanalyze-models")).toContainText(
    "download them in Settings → AI models",
  );
});
