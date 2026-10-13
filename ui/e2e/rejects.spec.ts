import { existsSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { jpeg } from "./fixtures";
import { LIBRARY, call, importFiles } from "./helpers";

type Detail = { files: { path: string }[] };

test("moves rejected photos to _Rejected and undoes it", async ({
  page,
  request,
}) => {
  const DAY = "2010-05-05";
  await importFiles(request, {
    "rej/keep.JPG": { bytes: jpeg(9701), time: new Date(`${DAY}T10:00:00`) },
    "rej/drop.JPG": { bytes: jpeg(9702), time: new Date(`${DAY}T11:00:00`) },
  });
  const photos = (
    await call<{ photos: { id: number; file_name: string }[] }>(
      request,
      "query_photos",
      { from: DAY, to: DAY },
    )
  ).photos;
  const drop = photos.find((p) => p.file_name === "drop.JPG")!;
  await call(request, "set_decisions", {
    changes: [{ photo_id: drop.id, flag: "reject" }],
  });
  const path = async () =>
    (await call<Detail>(request, "get_photo", { photo_id: drop.id })).files[0]!
      .path;
  const before = await path();

  await page.goto("/");
  await page.getByTestId("rejects-open").click();
  const confirm = page.getByTestId("rejects-confirm");
  await expect(confirm).toContainText("to _Rejected?");
  await page.getByTestId("rejects-move").click();
  await expect(page.getByTestId("rejects-result")).toContainText(
    "to _Rejected",
  );
  const after = await path();
  expect(after).toBe(`_Rejected/${before}`);
  expect(existsSync(join(LIBRARY, after))).toBe(true);
  expect(existsSync(join(LIBRARY, before))).toBe(false);
  // The photo still shows.
  await page.getByTestId("filter-from").fill(DAY);
  await page.getByTestId("filter-to").fill(DAY);
  await expect(
    page.locator(`[data-photo-id="${drop.id}"] img`),
  ).toHaveJSProperty("complete", true);

  await page.getByTestId("rejects-undo").click();
  await expect(page.getByTestId("rejects-result")).toContainText("back");
  expect(await path()).toBe(before);
  expect(existsSync(join(LIBRARY, before))).toBe(true);
});
