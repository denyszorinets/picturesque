import { readFileSync } from "node:fs";
import { createServer, type Server } from "node:http";
import { expect, test } from "@playwright/test";
import { call } from "./helpers";

// Serves the test model at the URLs in fixtures/models.json.
const MODEL = readFileSync(
  new URL(
    "../../crates/infra/tests/fixtures/onnx/affine.onnx",
    import.meta.url,
  ),
);
let server: Server;

test.beforeAll(async () => {
  server = createServer((req, res) => {
    if (req.url === "/affine.onnx") {
      res.writeHead(200, { "content-length": MODEL.length });
      res.end(MODEL);
    } else {
      res.writeHead(404).end();
    }
  });
  await new Promise<void>((resolve) =>
    server.listen(5175, "127.0.0.1", resolve),
  );
});

test.afterAll(() => {
  server.close();
});

test("downloads, verifies and removes models", async ({ page, request }) => {
  await page.goto("/");
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Import" })
    .click();
  const list = page.getByTestId("model-list");
  const good = list.locator('[data-model-id="e2e-affine"]');
  const corrupt = list.locator('[data-model-id="e2e-corrupt"]');
  await expect(good).toContainText("Test model");
  await expect(good.getByRole("link", { name: "MIT" })).toHaveAttribute(
    "href",
    "https://opensource.org/license/mit",
  );

  await good.getByRole("button", { name: "Download" }).click();
  await expect(good.getByTestId("model-state")).toHaveAttribute(
    "data-state",
    "installed",
  );
  await expect(good).toContainText("Downloaded");

  // A download that does not match its checksum is rejected.
  await corrupt.getByRole("button", { name: "Download" }).click();
  await expect(corrupt.getByTestId("model-error")).toContainText("corrupt");
  await expect(corrupt.getByRole("button", { name: "Retry" })).toBeVisible();

  // Installed models stay installed without the server (offline).
  server.close();
  await page.reload();
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Import" })
    .click();
  await expect(good.getByTestId("model-state")).toHaveAttribute(
    "data-state",
    "installed",
  );

  await good.getByRole("button", { name: "Remove" }).click();
  await expect(good.getByTestId("model-state")).toHaveAttribute(
    "data-state",
    "not_installed",
  );
  const models = await call<{ id: string; state: string }[]>(
    request,
    "list_models",
    null,
  );
  expect(models.map((m) => m.state)).toEqual(["not_installed", "failed"]);
});
