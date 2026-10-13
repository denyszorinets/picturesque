import { existsSync, mkdtempSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { jpeg } from "./fixtures";
import { call, importFiles } from "./helpers";

test("exports the picks to a folder", async ({ page, request }) => {
  const DAY = "2011-06-06";
  await importFiles(request, {
    "exp/one.JPG": { bytes: jpeg(9801), time: new Date(`${DAY}T10:00:00`) },
    "exp/two.JPG": { bytes: jpeg(9802), time: new Date(`${DAY}T11:00:00`) },
  });
  const photos = (
    await call<{ photos: { id: number; file_name: string }[] }>(
      request,
      "query_photos",
      { from: DAY, to: DAY },
    )
  ).photos;
  const one = photos.find((p) => p.file_name === "one.JPG")!;
  await call(request, "set_decisions", {
    changes: [{ photo_id: one.id, flag: "pick" }],
  });
  const out = mkdtempSync(join(tmpdir(), "picturesque-export-"));

  await page.goto("/");
  await page.getByTestId("open-export").click();
  const dialog = page.getByTestId("export-dialog");
  await dialog.getByTestId("export-destination").fill(out);
  await dialog.getByTestId("export-naming").fill("picks/{name}.{ext}");
  await page.screenshot({ path: test.info().outputPath("export.png") });
  await dialog.getByTestId("export-start").click();
  await expect(dialog.getByTestId("export-status")).toContainText(
    "all verified",
  );
  // Every pick in the shared library is exported; ours is among them.
  expect(existsSync(join(out, "picks", "one.JPG"))).toBe(true);
  expect(readdirSync(join(out, "picks"))).not.toContain("two.JPG");
  await dialog.getByTestId("export-done").click();
  await expect(dialog).toHaveCount(0);

  // A folder that does not exist is refused.
  await page.getByTestId("open-export").click();
  await dialog.getByTestId("export-destination").fill(join(out, "missing"));
  await dialog.getByTestId("export-start").click();
  await expect(dialog.getByTestId("export-error")).toContainText(
    "Cannot export",
  );
});
