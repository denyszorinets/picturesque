import { expect, test } from "@playwright/test";
import { jpeg } from "./fixtures";
import { call, importFiles } from "./helpers";

test("reviews a filtered set, persists it and reverses it without changing picks", async ({
  page,
  request,
}) => {
  const day = "2094-06-07";
  await importFiles(
    request,
    Object.fromEntries(
      [0, 1, 2].map((i) => [
        `review/${i}.JPG`,
        { bytes: jpeg(94000 + i), time: new Date(`${day}T10:00:0${i}`) },
      ]),
    ),
  );
  const query = { from: day, to: day };
  const photos = (
    await call<{ photos: { id: number }[] }>(request, "query_photos", query)
  ).photos;
  const picked = photos[0]!.id;
  await call(request, "set_decisions", {
    changes: [{ photo_id: picked, flag: "pick" }],
  });
  await page.goto("/");
  await page.getByTestId("filter-from").fill(day);
  await page.getByTestId("filter-to").fill(day);
  if (!(await page.getByTestId("filter-panel").isVisible()))
    await page.getByTestId("toggle-filters").click();
  await page
    .getByTestId("filter-panel")
    .getByRole("button", { name: /^Picks/ })
    .click();
  await expect(page.getByTestId("library-count")).toHaveText("1 photo");
  await page.getByTestId("open-review").click();
  const dialog = page.getByRole("dialog", { name: "Review filtered photos" });
  await expect(
    dialog.getByRole("radio", { name: "Current filtered view" }),
  ).toBeChecked();
  await expect(dialog.getByTestId("review-count")).toContainText("1 photos");
  await dialog
    .getByRole("button", { name: "Mark reviewed", exact: true })
    .click();
  await expect(dialog.getByRole("status")).toContainText(
    "1 photos marked reviewed",
  );
  await dialog.getByRole("button", { name: "Done" }).click();
  await page.reload();
  await expect(page.getByTestId("filter-panel")).toBeVisible();
  await page
    .getByLabel("Review progress", { exact: true })
    .selectOption("reviewed");
  await expect(page.getByTestId("library-count")).toHaveText("1 photo");
  await expect(
    page.getByTestId("timeline").getByTitle("Pick", { exact: true }),
  ).toBeVisible();
  await page.getByTestId("open-review").click();
  await expect(dialog.getByTestId("review-count")).toContainText("1 photos");
  await dialog.getByRole("button", { name: "Mark not reviewed" }).click();
  await expect(dialog.getByRole("status")).toContainText(
    "1 photos marked not reviewed",
  );
  await dialog.getByRole("button", { name: "Done" }).click();
  await expect(page.getByTestId("library-count")).toHaveText("0 photos");
  const remaining = await call<{ total: number }>(request, "query_photos", {
    ...query,
    flags: ["pick"],
  });
  expect(remaining.total).toBe(1);
});
