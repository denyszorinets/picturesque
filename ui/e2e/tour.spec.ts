import { expect, test } from "@playwright/test";
import { jpeg } from "./fixtures";
import { call, importFiles } from "./helpers";

type Settings = { onboarding: { tour_done: boolean } };

test("tours the library once, and again from Settings", async ({
  page,
  request,
}) => {
  await importFiles(request, { "tour/a.JPG": { bytes: jpeg(16001) } });
  // As on a first run with photos.
  await call(request, "update_settings", { onboarding: { tour_done: false } });

  await page.goto("/");
  const tour = page.getByTestId("tour");
  await expect(tour).toContainText("1 of 7");
  await expect(tour).toContainText("Your photos");
  await expect(page.getByTestId("tour-highlight")).toBeVisible();

  // Arrow keys and buttons move through it; the library ignores them.
  await page.keyboard.press("ArrowRight");
  await expect(tour).toContainText("Pick, reject, rate");
  await expect(page.getByTestId("loupe")).toHaveCount(0);
  for (let i = 2; i < 7; i++) await page.getByTestId("tour-next").click();
  await expect(tour).toContainText("Need a hand?");
  await page.getByTestId("tour-next").click();
  await expect(tour).toBeHidden();
  await expect
    .poll(
      async () =>
        (await call<Settings>(request, "get_settings", null)).onboarding
          .tour_done,
    )
    .toBe(true);

  // Not again after a reload, but on request.
  await page.reload();
  await expect(page.getByTestId("timeline")).toBeVisible();
  await expect(tour).toHaveCount(0);
  await page
    .getByRole("navigation", { name: "Main" })
    .getByRole("button", { name: "Settings" })
    .click();
  await page.getByTestId("open-tour").click();
  await expect(tour).toContainText("1 of 7");
  await page.keyboard.press("Escape");
  await expect(tour).toBeHidden();
});
