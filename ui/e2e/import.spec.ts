import { readdirSync } from "node:fs";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { folder, jpeg } from "./fixtures";

function filesUnder(dir: string): string[] {
  return readdirSync(dir, { recursive: true, withFileTypes: true })
    .filter((e) => e.isFile())
    .map((e) => join(e.parentPath, e.name));
}

test("imports a folder into the library through the UI", async ({ page }) => {
  const library = mkdtempSync(join(tmpdir(), "picturesque-e2e-lib-"));
  const card = folder({
    "DCIM/IMG_0001.JPG": jpeg(1),
    "DCIM/IMG_0002.JPG": jpeg(2),
    "DCIM/IMG_0003.JPG": jpeg(3),
    "DCIM/notes.txt": Buffer.from("not a photo"),
  });
  const backup = folder({ "copy-of-1.JPG": jpeg(1) });

  await page.goto("/");

  // Settings: library folder and a template, with live preview.
  await page.getByTestId("library-root").fill(library);
  await page.getByTestId("path-template").fill("{year|Undated}/{name}");
  await expect(page.getByTestId("template-error")).toContainText(".{ext}");
  await page
    .getByTestId("path-template")
    .fill("{year|Undated}/{date}/{name}.{ext}");
  await expect(page.getByTestId("template-preview")).toContainText(
    "2026/2026-10-12/IMG_0042.CR3",
  );
  await page.getByTestId("save-settings").click();
  await expect(page.getByTestId("save-settings")).toBeDisabled();

  // Sources.
  await page.getByTestId("source-path").fill(card);
  await page.getByTestId("add-source").click();
  await page.getByTestId("source-path").fill(backup);
  await page.getByTestId("source-path").press("Enter");
  await expect(page.getByTestId("sources")).toContainText(backup);

  // Plan and review.
  await page.getByTestId("plan-import").click();
  const summary = page.getByTestId("import-summary");
  await expect(summary).toContainText("3to import");
  await expect(summary).toContainText("1duplicates skipped");
  await expect(page.getByTestId("plan-table")).toContainText("IMG_0002.JPG");
  expect(filesUnder(library)).toHaveLength(0);

  // Copy.
  await page.getByTestId("start-import").click();
  await expect(page.getByTestId("import-outcome")).toContainText(
    "Import finished",
  );
  await expect(summary).toContainText("3imported");
  expect(filesUnder(library)).toHaveLength(3);

  await page.getByTestId("new-import").click();
  await expect(page.getByTestId("plan-import")).toBeVisible();
});

test("explains problems from the backend", async ({ page }) => {
  await page.goto("/");

  await page.getByTestId("library-root").fill("/definitely/not/here");
  await page.getByTestId("save-settings").click();

  await expect(page.getByTestId("settings-error")).toContainText(
    "does not exist",
  );
});
