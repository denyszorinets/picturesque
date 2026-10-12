import { expect, test } from "@playwright/test";

test("shows the backend version through the development server", async ({
  page,
}) => {
  await page.goto("/");

  await expect(
    page.getByRole("heading", { name: "Picturesque" }),
  ).toBeVisible();
  await expect(page.getByTestId("app-version")).toHaveText(/^v\d+\.\d+\.\d+$/);
});
