import { createServer, type IncomingMessage } from "node:http";
import type { AddressInfo } from "node:net";
import { expect, test } from "@playwright/test";
import { jpeg } from "./fixtures";
import { call, importFiles, LIBRARY } from "./helpers";

const DAY = "2005-02-03";

const CRITIC_DEFAULTS = {
  enabled: false,
  backend: "builtin",
  server: "http://127.0.0.1:11434/v1",
  model: "qwen3-vl:4b",
  preset: "amateur",
  rubric: null,
  background: false,
  intensity: "gentle",
  score_weight: 0.25,
  potential_weight: 0.15,
};

function body(req: IncomingMessage): Promise<string> {
  return new Promise((resolve) => {
    let text = "";
    req.on("data", (chunk: Buffer) => (text += chunk.toString()));
    req.on("end", () => resolve(text));
  });
}

test.afterEach(async ({ request }) => {
  await call(request, "update_settings", { critic: CRITIC_DEFAULTS });
});

test("summarizes stored critiques into recurring problems with examples", async ({
  page,
  request,
}) => {
  // A stub OpenAI-compatible server whose critique names two problems.
  const server = createServer((req, res) => {
    void (async () => {
      if (req.url === "/v1/models") {
        res.writeHead(200, { "content-type": "application/json" });
        res.end(JSON.stringify({ data: [{ id: "tiny-vl" }] }));
        return;
      }
      await body(req);
      const critique = {
        rating: 4,
        summary: "Too dark and a little soft.",
        strengths: ["subject"],
        weaknesses: ["underexposed", "soft focus"],
        issues: ["exposure", "focus"],
      };
      res.writeHead(200, { "content-type": "application/json" });
      res.end(
        JSON.stringify({
          choices: [{ message: { content: JSON.stringify(critique) } }],
        }),
      );
    })();
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const port = (server.address() as AddressInfo).port;

  try {
    await call(request, "update_settings", {
      library_root: LIBRARY,
      onboarding: { tour_done: true },
    });
    await importFiles(request, {
      "insights/a.JPG": {
        bytes: jpeg(9701),
        time: new Date(`${DAY}T10:00:00`),
      },
    });
    await call(request, "update_settings", {
      critic: {
        ...CRITIC_DEFAULTS,
        enabled: true,
        backend: "server",
        server: `http://127.0.0.1:${port}/v1`,
        model: "tiny-vl",
      },
    });
    const { photos } = await call<{ photos: { id: number }[] }>(
      request,
      "query_photos",
      { from: DAY, to: DAY, limit: 10 },
    );
    expect(photos.length).toBeGreaterThan(0);
    await call(request, "critique_photo", {
      photo_id: photos[0]!.id,
      refresh: true,
    });

    await page.goto("/");
    await page
      .getByRole("navigation")
      .getByRole("button", { name: "Insights" })
      .click();
    // The e2e library is shared, so other critiques may be there as well.
    await expect(page.getByTestId("insights-issue-exposure")).toContainText(
      "Exposure",
    );
    await expect(page.getByTestId("insights-issue-focus")).toBeVisible();
    await expect(
      page
        .getByTestId("insights-issue-exposure")
        .getByTestId("insights-example")
        .first(),
    ).toBeVisible();
    await expect(page.getByTestId("insights-trend")).toBeVisible();
  } finally {
    await new Promise<void>((resolve) => server.close(() => resolve()));
  }
});
