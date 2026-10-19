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

  // Back to the library; the folder is in the recent list.
  await switcher.click();
  await page.getByTestId("open-library").click();
  await expect(switcher).toContainText("Library");
  await switcher.click();
  await expect(page.getByTestId("recent-folder")).toContainText("shoot");
});
