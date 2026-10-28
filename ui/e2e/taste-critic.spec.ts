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
        issues: ["horizon"],
        potential: 9,
        fixable: "easy",
        fix_steps: ["Straighten the horizon"],
        improvements: ["Wait for the boat to cross"],
        camera_tips: ["Use a tripod"],
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
    // An earlier test or run may have stored a critique for this photo.
    const first = page
      .getByTestId("critique-run")
      .or(page.getByTestId("critique-again"));
    await first.click();
    await expect(page.getByTestId("critique-summary")).toContainText(
      "8.0/10 A calm, well balanced frame.",
    );
    await expect(page.getByTestId("critique-weaknesses")).toContainText(
      "tilted horizon",
    );
    await expect(page.getByTestId("critique-issues")).toContainText("Horizon");
    await page.getByTestId("critique-advice").locator("summary").click();
    await expect(page.getByTestId("critique-advice")).toContainText(
      "Wait for the boat to cross",
    );
    await expect(page.getByTestId("critique-advice")).toContainText(
      "Use a tripod",
    );
    await expect(page.getByTestId("critique-raw")).toContainText(
      "Easy to improve with editing",
    );
    await expect(page.getByTestId("critique-raw")).toContainText(
      "could reach 9.0/10",
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

test("shows the critique pill and keeps a critique after closing the loupe", async ({
  page,
  request,
}) => {
  // Chat completions wait on this gate, so the background worker's first
  // critique stays "running" and the critique pill stays visible (it is hidden
  // once the queue is done). Released right before the loupe part.
  // A model name unique to this run makes earlier critiques (from other
  // tests or repeats) stale, so there is always work left and the pill shows.
  const model = `tiny-vl-${Date.now()}`;
  let release!: () => void;
  const gate = new Promise<void>((resolve) => (release = resolve));
  const server = createServer((req, res) => {
    void (async () => {
      res.writeHead(200, { "content-type": "application/json" });
      if (req.url === "/v1/models") {
        res.end(JSON.stringify({ data: [{ id: model }] }));
        return;
      }
      await body(req);
      await gate;
      const critique = {
        rating: 6,
        summary: "Plain but clean.",
        strengths: ["clean"],
        weaknesses: ["flat"],
      };
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
      "pill/a.JPG": {
        bytes: jpeg(9310),
        time: new Date("1985-06-06T10:00:00"),
      },
    });
    await call(request, "update_settings", {
      library_root: LIBRARY,
      onboarding: { tour_done: true },
      critic: {
        ...CRITIC_DEFAULTS,
        enabled: true,
        backend: "server",
        server: `http://127.0.0.1:${port}/v1`,
        model,
        background: true,
        intensity: "full",
      },
    });
    await page.goto("/");
    await expect(page.getByTestId("critique-ring")).toBeVisible();
    await expect(page.getByTestId("critique-ring-label")).toContainText(
      /^Critique(?: paused ·)? \d+ \/ \d+/,
    );
    // Only one popover is open at a time.
    const popovers = page.getByTestId("activity-popover");
    await page.getByTestId("critique-ring").click({ timeout: 5000 });
    await expect(popovers).toHaveCount(1);
    await page.getByTestId("activity-ring").click({ timeout: 5000 });
    await expect(popovers).toHaveCount(1);
    await page.locator("h1").click();
    await expect(popovers).toHaveCount(0);
    await page
      .getByRole("navigation")
      .getByRole("button", { name: "Activity" })
      .click();
    await expect(page.getByTestId("activity-critique-count")).toContainText(
      "photos critiqued",
    );

    release();
    await page
      .getByRole("navigation")
      .getByRole("button", { name: "Library" })
      .click();
    // Narrow the timeline to the day of this test's photo (the shared
    // library holds older critiques on other photos).
    await page.getByTestId("filter-from").fill("1985-06-06");
    await page.getByTestId("filter-to").fill("1985-06-06");
    await page.locator("[data-photo-id]").first().click({ timeout: 10_000 });
    await page.keyboard.press("Enter");
    const summary = page.getByTestId("critique-summary");
    // The background worker may store the critique before or after the loupe
    // opens: wait for either, and run it ourselves only if it is still missing.
    const run = page.getByTestId("critique-run");
    await expect(summary.or(run)).toBeVisible();
    const outdated = page.getByTestId("critique-outdated");
    if (await run.isVisible()) {
      await run.click({ timeout: 5000 }).catch(() => {
        // The background critique landed first and replaced the button.
      });
    }
    // A critique from an earlier test is shown as outdated: ask again.
    if (await outdated.isVisible()) {
      await page.getByTestId("critique-again").click();
    }
    await expect(summary).toContainText("Plain but clean.");
    await page.keyboard.press("Escape");
    await page.keyboard.press("Enter");
    await expect(summary).toContainText("Plain but clean.");
  } finally {
    release();
    // Restore first: stopping the worker may still need the server to answer.
    await call(request, "update_settings", { critic: CRITIC_DEFAULTS });
    server.close();
  }
});
