import { readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { folder, jpeg } from "./fixtures";
import { LIBRARY, waitForFolder } from "./helpers";

function filesUnder(dir: string): string[] {
  return readdirSync(dir, { recursive: true, withFileTypes: true })
    .filter((e) => e.isFile())
    .map((e) => join(e.parentPath, e.name));
}

test("imports a watched folder and shows each photo's progress", async ({
  page,
  request,
}) => {
  const library = LIBRARY;
  const before = filesUnder(library).length;
  const card = folder({
    "DCIM/IMG_0001.JPG": jpeg(101),
    "DCIM/IMG_0002.JPG": jpeg(102),
    "DCIM/IMG_0003.JPG": jpeg(103),
    "DCIM/notes.txt": Buffer.from("not a photo"),
  });
  const backup = folder({ "copy-of-1.JPG": jpeg(101) });

  await page.goto("/");
  const nav = page.getByRole("navigation");

  // Settings: library folder and a template, with live preview.
  await nav.getByRole("button", { name: "Settings" }).click();
  await page.getByTestId("library-root").fill(library);
  await page.getByTestId("path-template").fill("{year|Undated}/{name}");
  await expect(page.getByTestId("template-error")).toContainText(".{ext}");
  // A non-default template, so saving is a change whatever ran before.
  await page.getByTestId("path-template").fill("{year}/{date}/{name}.{ext}");
  await expect(page.getByTestId("template-preview")).toContainText(
    "2026/2026-10-12/IMG_0042.CR3",
  );
  await page.getByTestId("save-settings").click();
  await expect(page.getByTestId("save-settings")).toBeDisabled();

  // Adding a folder starts importing it.
  await nav.getByRole("button", { name: "Activity" }).click();
  await page.getByTestId("source-path").fill(card);
  await page.getByTestId("add-source").click();
  await expect(page.getByTestId("sources")).toContainText(card);
  await waitForFolder(request, card);
  expect(filesUnder(library)).toHaveLength(before + 3);

  const row = page
    .getByTestId("job")
    .filter({ hasText: "IMG_0002.JPG" })
    .first();
  await expect(row).toHaveAttribute("data-stage", "imported");
  await expect(row.getByRole("img", { name: "Verified: done" })).toBeVisible();
  await expect(
    row.getByRole("img", { name: "Catalogued: done" }),
  ).toBeVisible();
  await expect(row.getByRole("img", { name: "Thumbnail: done" })).toBeVisible({
    timeout: 15_000,
  });
  await expect(page.getByTestId("log")).toContainText(
    "IMG_0002.JPG: imported to",
  );

  // A second folder holding a copy: skipped as a duplicate.
  await page.getByTestId("source-path").fill(backup);
  await page.getByTestId("source-path").press("Enter");
  await waitForFolder(request, backup);
  await page.getByTestId("filter-duplicate").click();
  await expect(
    page.getByTestId("job").filter({ hasText: "copy-of-1.JPG" }),
  ).toContainText("already in the library");

  // A new photo dropped into the watched card folder is imported on its own.
  writeFileSync(join(card, "DCIM", "IMG_0004.JPG"), jpeg(104));
  await expect
    .poll(() => filesUnder(library).length, { timeout: 30_000 })
    .toBe(before + 4);

  // And shows up in the library.
  await nav.getByRole("button", { name: "Library" }).click();
  await expect(page.getByTestId("library-empty")).toHaveCount(0);
});

test("explains problems from the backend", async ({ page }) => {
  await page.goto("/");
  const nav = page.getByRole("navigation");

  await nav.getByRole("button", { name: "Settings" }).click();
  await page.getByTestId("library-root").fill("/definitely/not/here");
  await page.getByTestId("save-settings").click();
  // Either the folder is missing, or (once photos exist) switching
  // libraries is refused; both are explained.
  await expect(page.getByTestId("settings-error")).toContainText(
    /does not exist|not supported yet/,
  );

  await nav.getByRole("button", { name: "Activity" }).click();
  await page.getByTestId("source-path").fill("/definitely/not/here");
  await page.getByTestId("add-source").click();
  await expect(page.getByTestId("activity-error")).toContainText("Cannot read");
});
