import { existsSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { folder, jpeg } from "./fixtures";
import { call, LIBRARY, waitForFolder } from "./helpers";

test("offers to delete originals after an import, and deletes only after the confirmation", async ({
  page,
  request,
}) => {
  const card = folder({
    "DCIM/IMG_A.JPG": jpeg(9801),
    "DCIM/IMG_B.JPG": jpeg(9802),
  });
  await call(request, "update_settings", {
    library_root: LIBRARY,
    onboarding: { tour_done: true },
  });
  // The setting keeps originals; this folder asks for itself.
  const source = await call<{ id: number; delete_originals: string | null }>(
    request,
    "add_import_source",
    { path: card, watch: false, delete_originals: "ask" },
  );
  expect(source.delete_originals).toBe("ask");
  await waitForFolder(request, card);

  try {
    await page.goto("/");
    await page
      .getByRole("navigation")
      .getByRole("button", { name: "Activity" })
      .click();
    const offer = page.getByTestId(`originals-offer-${source.id}`);
    await expect(offer).toBeVisible();

    await offer.click();
    const dialog = page.getByTestId("originals-dialog");
    await expect(page.getByTestId("originals-summary")).toContainText(
      "2 photos",
    );
    await expect(dialog).toContainText("Deleting is permanent");
    // Nothing happens before the confirmation.
    expect(existsSync(join(card, "DCIM", "IMG_A.JPG"))).toBe(true);

    await page.getByTestId("originals-go").click();
    await expect(page.getByTestId("originals-result")).toContainText(
      "Deleted the originals of 2 photos",
    );
    expect(existsSync(join(card, "DCIM", "IMG_A.JPG"))).toBe(false);
    expect(existsSync(join(card, "DCIM", "IMG_B.JPG"))).toBe(false);

    await dialog.getByRole("button", { name: "Done" }).click();
    await expect(offer).toHaveCount(0);
    // The photos are still in the library.
    const found = await call<{ photos: unknown[] }>(request, "query_photos", {
      limit: 1000,
    });
    expect(found.photos.length).toBeGreaterThanOrEqual(2);
  } finally {
    await call(request, "remove_import_source", { id: source.id });
  }
});

test("the folder's choice is set when adding it and can be changed", async ({
  page,
  request,
}) => {
  const card = folder({ "IMG_C.JPG": jpeg(9803) });
  await call(request, "update_settings", {
    library_root: LIBRARY,
    onboarding: { tour_done: true },
  });
  await page.goto("/");
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Activity" })
    .click();

  await page.getByTestId("add-source-originals").selectOption("never");
  await page.getByTestId("source-path").fill(card);
  await page.getByTestId("add-source").click();
  const row = page.getByTestId("sources").locator("li", { hasText: card });
  const choice = row.getByTestId("source-originals");
  await expect(choice).toHaveValue("never");

  await choice.selectOption("ask");
  await expect
    .poll(async () => {
      const sources = await call<
        { path: string; delete_originals: string | null }[]
      >(request, "list_import_sources", null);
      return sources.find((s) => s.path === card)?.delete_originals;
    })
    .toBe("ask");

  const sources = await call<{ id: number; path: string }[]>(
    request,
    "list_import_sources",
    null,
  );
  await call(request, "remove_import_source", {
    id: sources.find((s) => s.path === card)!.id,
  });
});

test("deleting right after import needs a confirmation and frees each original as it is copied", async ({
  page,
  request,
}) => {
  const card = folder({
    "IMG_D.JPG": jpeg(9804),
    "IMG_E.JPG": jpeg(9805),
  });
  await call(request, "update_settings", {
    library_root: LIBRARY,
    onboarding: { tour_done: true },
  });
  await page.goto("/");
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Activity" })
    .click();

  // Cancelling leaves the choice as it was.
  const choose = page.getByTestId("add-source-originals");
  await choose.selectOption("immediately");
  await expect(page.getByTestId("immediate-dialog")).toContainText(
    "no trash and no undo",
  );
  await page.getByTestId("immediate-cancel").click();
  await expect(choose).toHaveValue("default");
  expect(existsSync(join(card, "IMG_D.JPG"))).toBe(true);

  await choose.selectOption("immediately");
  await page.getByTestId("immediate-confirm").click();
  await expect(choose).toHaveValue("immediately");
  await page.getByTestId("source-path").fill(card);
  await page.getByTestId("add-source").click();

  // Imported, verified and gone from the card without any further click.
  await expect
    .poll(
      () =>
        existsSync(join(card, "IMG_D.JPG")) ||
        existsSync(join(card, "IMG_E.JPG")),
    )
    .toBe(false);
  const found = await call<{ photos: unknown[] }>(request, "query_photos", {
    limit: 1000,
  });
  expect(found.photos.length).toBeGreaterThanOrEqual(2);

  const sources = await call<
    { id: number; path: string; delete_originals: string | null }[]
  >(request, "list_import_sources", null);
  const mine = sources.find((s) => s.path === card)!;
  expect(mine.delete_originals).toBe("immediately");
  await call(request, "remove_import_source", { id: mine.id });
});
