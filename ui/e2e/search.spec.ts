import type { Server } from "node:http";
import { expect, test } from "@playwright/test";
import { jpeg } from "./fixtures";
import { call, importFiles, serveModels } from "./helpers";

type Status = { analyzed: number; total: number };
type Model = { id: string; state: string };

let server: Server;

test.beforeAll(async () => {
  server = await serveModels();
});

test.afterAll(() => {
  server.close();
});

test("searches photos by description once the CLIP models are downloaded", async ({
  page,
  request,
}) => {
  await importFiles(request, {
    "search/a.JPG": {
      bytes: jpeg(9501),
      time: new Date("2008-01-01T10:00:00"),
    },
    "search/b.JPG": {
      bytes: jpeg(9502),
      time: new Date("2008-01-02T10:00:00"),
    },
  });
  await page.goto("/");
  const search = page.getByTestId("search");

  // Without the models, searching explains what to download.
  await search.fill("red flowers");
  await search.press("Enter");
  await expect(page.getByTestId("search-error")).toContainText("text encoder");
  await search.press("Escape");
  await expect(page.getByTestId("search-error")).toHaveCount(0);

  for (const id of ["clip-image", "clip-text"])
    await call(request, "install_model", { id });
  await expect
    .poll(
      async () =>
        (await call<Model[]>(request, "list_models", null))
          .filter((m) => m.id.startsWith("clip-"))
          .map((m) => m.state),
      { timeout: 30_000 },
    )
    .toEqual(["installed", "installed"]);
  await expect
    .poll(
      async () => {
        const s = await call<Status>(request, "analysis_status", null);
        return s.analyzed === s.total;
      },
      { timeout: 60_000 },
    )
    .toBe(true);

  // "/" focuses the search box; results replace the timeline in relevance
  // order under one heading (the stand-in models match arbitrarily).
  await page.locator("body").press("/");
  await expect(search).toBeFocused();
  await search.fill("red");
  await search.press("Enter");
  const expected = await call<{ photo_id: number }[]>(
    request,
    "search_photos",
    { text: "red" },
  );
  await expect(page.getByTestId("library-count")).toHaveText(
    `${expected.length} ${expected.length === 1 ? "photo" : "photos"}`,
  );
  const chip = page.getByRole("button", { name: "Remove filter “red”" });
  await expect(chip).toBeVisible();
  if (expected.length > 0) {
    await expect(page.getByTestId("day-header")).toHaveText(/Matching “red”/);
    await expect(
      page.getByTestId("timeline").locator("button.tile").first(),
    ).toHaveAttribute("data-photo-id", String(expected[0]!.photo_id));
  } else {
    await expect(page.getByTestId("no-match")).toHaveText(
      "No photos match “red”.",
    );
  }

  await page.screenshot({ path: test.info().outputPath("search.png") });

  // Removing the search brings the timeline back.
  await chip.click();
  await expect(search).toHaveValue("");
  await expect(page.getByTestId("day-header").first()).not.toHaveText(
    /Matching/,
  );

  for (const id of ["clip-image", "clip-text"])
    await call(request, "remove_model", { id });
});
