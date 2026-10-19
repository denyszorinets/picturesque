import { expect, test } from "@playwright/test";
import { jpeg, folder } from "./fixtures";
import { call, LIBRARY, waitForFolder } from "./helpers";

type Source = { id: number; path: string };

test("sets up the library, an import folder and models in the wizard", async ({
  page,
  request,
}) => {
  const card = folder({ "DCIM/IMG_0001.JPG": jpeg(15001) });

  await page.goto("/");
  // First launch opens the wizard by itself; other tests may have set a
  // library folder already, so open it from Settings if needed.
  const wizard = page.getByTestId("setup-wizard");
  if (!(await wizard.isVisible())) {
    await page
      .getByRole("navigation", { name: "Main" })
      .getByRole("button", { name: "Settings" })
      .click();
    await page.getByTestId("open-setup").click();
  }
  await expect(wizard).toContainText("Step 1 of 3");

  await page.getByTestId("setup-library-root").fill(LIBRARY);
  await wizard.getByRole("radio").first().check();
  await page.getByTestId("setup-next").click();
  await expect(wizard).toContainText("Step 2 of 3");
  const settings = await call<{ library_root: string }>(
    request,
    "get_settings",
    null,
  );
  expect(settings.library_root).toBe(LIBRARY);

  await page.getByTestId("setup-source").fill(card);
  await page.getByTestId("setup-add-source").click();
  await expect(page.getByTestId("setup-sources")).toContainText(card);
  await page.getByTestId("setup-next").click();

  await expect(wizard).toContainText("Step 3 of 3");
  await expect(wizard.getByTestId("model-list")).toBeVisible();
  await page.getByTestId("setup-next").click();
  await expect(wizard).toBeHidden();

  // The folder is imported, and the library shows its photo.
  await waitForFolder(request, card);
  await expect(
    page.getByRole("button", { name: "Library", exact: true }),
  ).toHaveAttribute("aria-current", "page");
  await expect(page.getByTestId("library-empty")).toHaveCount(0);

  // Leave the shared import folders as they were.
  const sources = await call<Source[]>(request, "list_import_sources", null);
  const added = sources.find((s) => s.path === card);
  if (added) await call(request, "remove_import_source", { id: added.id });
});
