import { expect, test } from "@playwright/test";
import { jpeg } from "./fixtures";
import { importFiles } from "./helpers";

// Newest photos come first, so these (dated in the future) are the first tiles.
const FILES = Object.fromEntries(
  [0, 1, 2, 3].map((i) => [
    `cull/IMG_${i}.JPG`,
    { bytes: jpeg(7000 + i), time: new Date(`2099-01-01T10:00:0${3 - i}`) },
  ]),
);

test("culls with the keyboard, undoes, and keeps decisions", async ({
  page,
  request,
}) => {
  await importFiles(request, FILES);
  await page.goto("/");
  const tiles = page.locator("[data-photo-id]");
  const tile = (i: number) => tiles.nth(i);
  await tile(0).click();

  // Pick and rate the selected photo.
  await page.keyboard.press("p");
  await expect(tile(0).getByTitle("Pick")).toBeVisible();
  await page.keyboard.press("3");
  await expect(tile(0).getByLabel("3 stars")).toBeVisible();

  // Arrow keys move; shift extends; reject applies to the selection.
  await page.keyboard.press("ArrowRight");
  await expect(tile(1)).toHaveAttribute("aria-pressed", "true");
  await page.keyboard.press("Shift+ArrowRight");
  await expect(page.getByTestId("selection-count")).toHaveText("2 selected");
  await page.keyboard.press("x");
  await expect(tile(1).getByTitle("Rejected")).toBeVisible();
  await expect(tile(2).getByTitle("Rejected")).toBeVisible();

  // Undo restores both; redo applies again.
  await page.keyboard.press("ControlOrMeta+z");
  await expect(tile(1).getByTitle("Rejected")).toBeHidden();
  await expect(tile(2).getByTitle("Rejected")).toBeHidden();
  await page.keyboard.press("ControlOrMeta+Shift+z");
  await expect(tile(2).getByTitle("Rejected")).toBeVisible();

  // Decisions are stored: they survive a reload.
  await page.reload();
  await expect(tile(0).getByTitle("Pick")).toBeVisible();
  await expect(tile(0).getByLabel("3 stars")).toBeVisible();
  await expect(tile(2).getByTitle("Rejected")).toBeVisible();

  // Culling works in the loupe too.
  await tile(3).click();
  await page.keyboard.press("Enter");
  await expect(page.getByTestId("loupe")).toBeVisible();
  await page.keyboard.press("p");
  await page.keyboard.press("Escape");
  await expect(tile(3).getByTitle("Pick")).toBeVisible();
});
