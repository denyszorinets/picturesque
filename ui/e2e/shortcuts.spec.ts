import { expect, test } from "@playwright/test";
import { jpeg } from "./fixtures";
import { importFiles } from "./helpers";

test("configures letters, validates conflicts and restores defaults", async ({
  page,
  request,
}) => {
  await importFiles(request, {
    "shortcut-settings/a.JPG": { bytes: jpeg(91999) },
  });
  await page.goto("/");
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  const fields = page.getByTestId("shortcut-settings");
  await fields.getByLabel("Pick shortcut", { exact: true }).fill("x");
  await expect(fields.getByRole("alert")).toContainText("both Pick and Reject");
  await expect(
    fields.getByRole("button", { name: "Save shortcuts" }),
  ).toBeDisabled();
  await fields
    .getByRole("button", { name: "One-hand culling (A / D / F)" })
    .click();
  await fields.getByRole("button", { name: "Save shortcuts" }).click();
  await expect(fields.getByRole("status")).toHaveText("Shortcuts saved.");
  await fields.screenshot({
    path: test.info().outputPath("shortcuts-settings.png"),
  });
  await page.reload();
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(fields.getByLabel("Pick shortcut", { exact: true })).toHaveValue(
    "a",
  );
  await page.getByTestId("open-help").click();
  await expect(
    page.getByTestId("shortcuts-Culling").locator("kbd").first(),
  ).toHaveText("A");
  await page.keyboard.press("Escape");
  await fields
    .getByRole("button", { name: "Restore default shortcuts" })
    .click();
  await fields.getByRole("button", { name: "Save shortcuts" }).click();
  await page.reload();
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(fields.getByLabel("Pick shortcut", { exact: true })).toHaveValue(
    "p",
  );
});

test("uses chosen keys in the grid, loupe and compare, with fixed undo", async ({
  page,
  request,
}) => {
  const day = "2096-04-03";
  await importFiles(
    request,
    Object.fromEntries(
      [0, 1, 2].map((i) => [
        `shortcuts/${i}.JPG`,
        { bytes: jpeg(92000 + i), time: new Date(`${day}T10:00:0${i}`) },
      ]),
    ),
  );
  await page.goto("/");
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  const fields = page.getByTestId("shortcut-settings");
  await fields
    .getByRole("button", { name: "One-hand culling (A / D / F)" })
    .click();
  // Reuse old action letters: Zoom moves to P, and Pick takes Z.
  await fields.getByLabel("Pick shortcut", { exact: true }).fill("z");
  await fields.getByLabel("Zoom shortcut", { exact: true }).fill("p");
  await fields.getByLabel("Compare photos shortcut", { exact: true }).fill("q");
  await fields.getByRole("button", { name: "Save shortcuts" }).click();
  await page.getByRole("button", { name: "Library", exact: true }).click();
  await page.getByTestId("filter-from").fill(day);
  await page.getByTestId("filter-to").fill(day);
  const tiles = page.getByTestId("timeline").locator("button.tile");
  await expect(tiles).toHaveCount(3);
  await tiles.nth(0).click();
  await page.keyboard.press("z");
  await expect(tiles.nth(0).getByTitle("Pick", { exact: true })).toBeVisible();
  await page.keyboard.press("ControlOrMeta+z");
  await expect(tiles.nth(0).getByTitle("Pick", { exact: true })).toBeHidden();
  await page.keyboard.press("Shift+z");
  await expect(tiles.nth(0).getByTitle("Pick", { exact: true })).toBeVisible();
  await expect(tiles.nth(1)).toHaveAttribute("aria-pressed", "true");
  await page.keyboard.press("Enter");
  const loupe = page.getByTestId("loupe");
  await expect(loupe.getByTitle("Pick (Z)", { exact: true })).toBeVisible();
  await page.keyboard.press("d");
  await expect(loupe.getByTitle("Reject (D)", { exact: true })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await page.keyboard.press("f");
  await expect(loupe.getByTitle("Unflag (F)", { exact: true })).toBeDisabled();
  await page.keyboard.press("Escape");
  await tiles.nth(0).click();
  await tiles.nth(1).click({ modifiers: ["ControlOrMeta"] });
  await page.keyboard.press("q");
  await expect(page.getByTestId("compare-pane")).toHaveCount(2);
  await expect(
    page.getByTestId("compare").locator('[data-active="true"]'),
  ).toHaveAttribute("data-photo-id", /\d+/);
  await page.keyboard.press("d");
  await expect(
    page.getByTestId("compare").locator('[data-active="true"]'),
  ).toContainText("Rejected");
  await page.keyboard.press("Escape");
});
