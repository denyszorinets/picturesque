import { createServer, type IncomingMessage } from "node:http";
import type { AddressInfo } from "node:net";
import { expect, test } from "@playwright/test";
import { jpeg } from "./fixtures";
import { call, importFiles, LIBRARY } from "./helpers";

type Settings = {
  taste: { personal_weight: number };
  critic: {
    enabled: boolean;
    backend: string;
    server: string;
    model: string;
    rubric: string | null;
    background: boolean;
    intensity: string;
    score_weight: number;
  };
};

const CRITIC_DEFAULTS = {
  enabled: false,
  backend: "builtin",
  server: "http://127.0.0.1:11434/v1",
  model: "qwen3-vl:4b",
  rubric: null,
  background: false,
  intensity: "gentle",
  score_weight: 0.25,
};

function body(req: IncomingMessage): Promise<string> {
  return new Promise((resolve) => {
    let text = "";
    req.on("data", (chunk: Buffer) => (text += chunk.toString()));
    req.on("end", () => resolve(text));
  });
}

test.afterEach(async ({ request }) => {
  await call(request, "update_settings", {
    taste: { personal_weight: 0.5 },
    critic: CRITIC_DEFAULTS,
  });
});

test("sets how much your taste counts and says what it learned", async ({
  page,
  request,
}) => {
  await call(request, "update_settings", {
    library_root: LIBRARY,
    onboarding: { tour_done: true },
  });
  // Other tests may have picked and rejected enough photos to teach it.
  const taste = await call<{ learned: boolean; needed: number }>(
    request,
    "taste_status",
    null,
  );
  await page.goto("/");
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Settings" })
    .click();
  await expect(page.getByTestId("taste-status")).toContainText(
    taste.learned
      ? "Learned from"
      : `Pick and reject at least ${taste.needed} analyzed photos each`,
  );
  await page.getByTestId("taste-weight").fill("");
  await expect(page.getByTestId("taste-weight-error")).toHaveText(
    "Enter a percentage from 0 to 100.",
  );
  await expect(page.getByTestId("save-settings")).toBeDisabled();
  await page.getByTestId("taste-weight").fill("80");
  await expect(page.getByTestId("taste-weight-error")).toHaveCount(0);
  await page.getByTestId("save-settings").click();
  await expect(page.getByTestId("save-settings")).toBeDisabled();
  const settings = await call<Settings>(request, "get_settings", null);
  expect(settings.taste.personal_weight).toBeCloseTo(0.8);
});

test("sets up the critic and critiques a photo in the loupe", async ({
  page,
  request,
}) => {
  // A stub OpenAI-compatible server with one vision model.
  const asked: { model: string; image: string; rubric: string }[] = [];
  const server = createServer((req, res) => {
    void (async () => {
      if (req.url === "/v1/models") {
        res.writeHead(200, { "content-type": "application/json" });
        res.end(JSON.stringify({ data: [{ id: "tiny-vl" }] }));
        return;
      }
      const request = JSON.parse(await body(req));
      asked.push({
        model: request.model,
        image: request.messages[1].content[1].image_url.url,
        rubric: request.messages[0].content,
      });
      const critique = {
        rating: 8,
        summary: "A calm, well balanced frame.",
        strengths: ["soft light"],
        weaknesses: ["tilted horizon"],
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
    await importFiles(request, {
      "critic/a.JPG": {
        bytes: jpeg(9301),
        time: new Date("2003-04-04T10:00:00"),
      },
    });
    await page.goto("/");
    await page
      .getByRole("navigation")
      .getByRole("button", { name: "Settings" })
      .click();
    await page.getByTestId("critic-enabled").check();
    // The built-in critic is the default; its model is not downloaded here.
    await expect(page.getByTestId("critic-builtin")).toBeChecked();
    await page.getByTestId("save-settings").click();
    await expect(page.getByTestId("critic-status")).toContainText(
      "Settings → AI models",
    );
    await page.getByTestId("critic-use-server").check();
    // Another computer gets a warning.
    await page
      .getByTestId("critic-server")
      .fill("http://192.168.1.20:11434/v1");
    await expect(page.getByTestId("critic-remote")).toBeVisible();
    await page.getByTestId("critic-server").fill(`http://127.0.0.1:${port}/v1`);
    await expect(page.getByTestId("critic-remote")).toBeHidden();
    await expect(page.getByTestId("critic-status")).toContainText(
      "Save to use these settings.",
    );
    await page.getByTestId("critic-model").fill("tiny-vl");
    await page.getByTestId("critic-rubric").fill("Only the light.");
    await page.getByTestId("save-settings").click();
    await expect(page.getByTestId("critic-status")).toContainText(
      "Ready: tiny-vl is installed.",
    );
    // Suggested models show their license.
    await expect(page.getByTestId("critic-suggested")).toContainText(
      "qwen3-vl:4b",
    );
    await expect(page.getByTestId("critic-suggested")).toContainText(
      "Apache-2.0",
    );

    await page
      .getByRole("navigation")
      .getByRole("button", { name: "Library" })
      .click();
    await page.locator("[data-photo-id]").first().click();
    await page.keyboard.press("Enter");
    await page.getByTestId("critique-run").click();
    await expect(page.getByTestId("critique-summary")).toContainText(
      "8.0/10 A calm, well balanced frame.",
    );
    await expect(page.getByTestId("critique-weaknesses")).toContainText(
      "tilted horizon",
    );
    expect(asked).toHaveLength(1);
    expect(asked[0]!.model).toBe("tiny-vl");
    expect(asked[0]!.image).toMatch(/^data:image\/jpeg;base64,\/9j\//);
    expect(asked[0]!.rubric).toContain("Only the light.");

    // Kept: coming back shows it without asking again.
    await page.keyboard.press("Escape");
    await page.keyboard.press("Enter");
    await expect(page.getByTestId("critique-summary")).toBeVisible();
    expect(asked).toHaveLength(1);
  } finally {
    server.close();
  }
});

test("sets up the background critic without starting it", async ({
  page,
  request,
}) => {
  await call(request, "update_settings", {
    library_root: LIBRARY,
    onboarding: { tour_done: true },
  });
  await page.goto("/");
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Settings" })
    .click();
  await page.getByTestId("critic-enabled").check();
  await page.getByTestId("critic-background").check();
  await page.getByTestId("critic-intensity").selectOption("when_idle");
  await page.getByTestId("critic-score-weight").fill("");
  await expect(page.getByTestId("critic-score-weight-error")).toHaveText(
    "Enter a percentage from 0 to 100.",
  );
  await expect(page.getByTestId("save-settings")).toBeDisabled();
  await page.getByTestId("critic-score-weight").fill("40");
  await expect(page.getByTestId("critic-score-weight-error")).toHaveCount(0);
  await expect(page.getByTestId("critic-progress")).toContainText(
    "Save to use these settings.",
  );
  await page.getByTestId("save-settings").click();
  await expect(page.getByTestId("save-settings")).toBeDisabled();
  const settings = await call<Settings>(request, "get_settings", null);
  expect(settings.critic.background).toBe(true);
  expect(settings.critic.intensity).toBe("when_idle");
  expect(settings.critic.score_weight).toBeCloseTo(0.4);
});
