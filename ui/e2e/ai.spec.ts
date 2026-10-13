import { expect, test } from "@playwright/test";
import { call } from "./helpers";

type AiStatus = {
  available: string[];
  choice: string;
  provider: string | null;
  problem: string | null;
};

test("chooses the AI provider and shows which one runs", async ({
  page,
  request,
}) => {
  const initial = await call<AiStatus>(request, "ai_status", null);
  expect(initial.available.at(-1)).toBe("cpu");
  expect(initial.provider).not.toBeNull();

  await page.goto("/");
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Import" })
    .click();
  const status = page.getByTestId("ai-status");
  await expect(status).toContainText("Models run on");

  await page.getByTestId("ai-provider").selectOption("cpu");
  await page.getByTestId("save-settings").click();
  await expect(status).toContainText("Models run on CPU.");
  expect((await call<AiStatus>(request, "ai_status", null)).choice).toBe("cpu");

  // Restore the default for other tests.
  await page.getByTestId("ai-provider").selectOption("auto");
  await page.getByTestId("save-settings").click();
  await expect(page.getByTestId("save-settings")).toBeDisabled();
  await expect(status).toContainText("Models run on");
});
