import { existsSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { folder, jpeg } from "./fixtures";
import { LIBRARY, call } from "./helpers";

// Leave the shared backend in the library for the other tests.
test.afterEach(async ({ request }) => {
  await call(request, "open_library", null);
});

test("culls a folder in place and goes back to the library", async ({
  page,
  request,
}) => {
  await call(request, "update_settings", {
    library_root: LIBRARY,
    onboarding: { tour_done: true },
  });
  const shoot = folder({
    "shoot/a.JPG": jpeg(9301),
    "shoot/b.JPG": jpeg(9302),
  });
  const dir = join(shoot, "shoot");

  await page.goto("/");
  const switcher = page.getByTestId("session-switcher");
  await switcher.click();
  // A browser has no folder dialog: the path is typed.
  await page.getByTestId("open-folder").click();
  await page.getByTestId("open-folder-path").fill(dir);
  await page.getByTestId("open-folder-go").click();

  await expect(switcher).toContainText("shoot");
  await expect(page.locator("[data-photo-id]")).toHaveCount(2, {
    timeout: 30_000,
  });
  // Nothing was copied: the photos are catalogued where they are.
  const settings = await call<{ library_root: string; open_folder: string }>(
    request,
    "get_settings",
    null,
  );
  expect(settings.open_folder).toBe(dir);

  // Decisions go to sidecars next to the originals.
  await page.locator("[data-photo-id]").first().click();
  await page.keyboard.press("p");
  await expect
    .poll(
      () =>
        existsSync(join(dir, "a.JPG.xmp")) ||
        existsSync(join(dir, "b.JPG.xmp")),
      { timeout: 10_000 },
    )
    .toBe(true);

  // Finishing writes every sidecar and labels the auto-picks.
  await page.getByTestId("open-finish").click();
  await page.getByTestId("finish-go").click();
  await expect(page.getByTestId("finish-result")).toContainText(
    "every decision is in the XMP sidecars",
  );
  // Copy the keepers out, each with a sidecar holding its decision.
  await page.getByTestId("finish-export").click();
  const out = folder({});
  await page.getByTestId("export-keepers").check();
  await page.getByTestId("export-destination").fill(out);
  await page.getByTestId("export-start").click();
  await expect(page.getByTestId("export-status")).toContainText("all verified");
  const picked = existsSync(join(out, "a.JPG")) ? "a.JPG" : "b.JPG";
  expect(existsSync(join(out, picked))).toBe(true);
  expect(existsSync(join(out, `${picked}.xmp`))).toBe(true);
  await page.getByTestId("export-done").click();

  // Back to the library; the folder is in the recent list.
  await switcher.click();
  await page.getByTestId("open-library").click();
  await expect(switcher).toContainText("Library");
  await switcher.click();
  await expect(page.getByTestId("recent-folder")).toContainText("shoot");
});
