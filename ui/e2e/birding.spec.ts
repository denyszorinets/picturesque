import { expect, test, type Page } from "@playwright/test";
import { jpeg } from "./fixtures";
import { call, importFiles } from "./helpers";

/** Near-identical gradients with different bytes, so they form a series. */
async function similar(page: Page, hue: number): Promise<Buffer> {
  const data = await page.evaluate((hue) => {
    const canvas = document.createElement("canvas");
    canvas.width = 3000;
    canvas.height = 2000;
    const ctx = canvas.getContext("2d")!;
    const g = ctx.createLinearGradient(0, 0, 3000, 2000);
    g.addColorStop(0, `hsl(${hue} 60% 30%)`);
    g.addColorStop(1, `hsl(${hue + 40} 60% 70%)`);
    ctx.fillStyle = g;
    ctx.fillRect(0, 0, 3000, 2000);
    ctx.fillStyle = "#fff";
    for (let x = 0; x < 3000; x += 150)
      for (let y = 0; y < 2000; y += 150) ctx.fillRect(x, y, 40, 40);
    return canvas.toDataURL("image/jpeg", 0.8).split(",")[1]!;
  }, hue);
  return Buffer.from(data, "base64");
}

type Photos = { photos: { id: number; flag: string }[] };
type SeriesView = { photo_ids: number[] };

async function showDay(page: Page, day: string) {
  await page.getByTestId("filter-from").fill(day);
  await page.getByTestId("filter-to").fill(day);
}

test("stacked, the loupe steps through every photo of each stack, with the stacks up next", async ({
  page,
  request,
}) => {
  const DAY = "2004-04-04";
  const at = (t: string) => new Date(`${DAY}T${t}`);
  await page.goto("/");
  // A burst of three, a single photo an hour later, then a burst of two.
  await importFiles(request, {
    "stacks/a1.JPG": { bytes: await similar(page, 20), time: at("08:00:00") },
    "stacks/a2.JPG": { bytes: await similar(page, 21), time: at("08:00:01") },
    "stacks/a3.JPG": { bytes: await similar(page, 22), time: at("08:00:02") },
    "stacks/single.JPG": { bytes: jpeg(8801), time: at("09:00:00") },
    "stacks/b1.JPG": { bytes: await similar(page, 220), time: at("10:00:00") },
    "stacks/b2.JPG": { bytes: await similar(page, 221), time: at("10:00:01") },
  });
  const ids = (
    await call<Photos>(request, "query_photos", {
      from: DAY,
      to: DAY,
      sort: "captured_asc",
    })
  ).photos.map((p) => p.id);
  for (const [first, size] of [
    [ids[0]!, 3],
    [ids[4]!, 2],
  ] as const)
    await expect
      .poll(
        async () =>
          (await call<SeriesView>(request, "photo_series", { photo_id: first }))
            .photo_ids.length,
        { timeout: 60_000 },
      )
      .toBe(size);

  await page.reload();
  await showDay(page, DAY);
  await page.getByTestId("sort").selectOption("captured_asc");
  await page.getByTestId("toggle-stack").click();
  const timeline = page.getByTestId("timeline");
  await expect(timeline.locator("button.tile")).toHaveCount(3);
  await timeline.locator("button.tile").first().dblclick();

  const count = page.getByTestId("series-ribbon-count");
  const position = page.getByTestId("loupe-position");
  await expect(position).toHaveText("Stack 1 / 3");
  // The stacks up next: this one and the two after it.
  await expect(page.getByTestId("upcoming-stacks")).toBeVisible();
  await expect(page.getByTestId("upcoming-entry")).toHaveCount(3);
  await expect(page.getByTestId("upcoming-remaining")).toContainText(
    "1 of 3 · 2 more",
  );

  // Every frame of the first stack, then the single photo, then the
  // second stack from its first frame.
  // The stack opens on its best photo; every frame can be opened.
  await page.getByTestId("series-ribbon-frame").first().click();
  await expect(count).toHaveText("1 of 3");
  await page.keyboard.press("ArrowRight");
  await expect(count).toHaveText("2 of 3");
  await expect(position).toHaveText("Stack 1 / 3");

  // A decision applies to the frame shown, not the stack's cover.
  await page.keyboard.press("p");
  await expect
    .poll(async () => {
      const page = await call<Photos>(request, "query_photos", {
        photo_ids: [ids[1]!],
      });
      return page.photos[0]?.flag;
    })
    .toBe("pick");

  await page.keyboard.press("ArrowRight");
  await expect(count).toHaveText("3 of 3");
  await page.keyboard.press("ArrowRight");
  await expect(position).toHaveText("Stack 2 / 3");
  await expect(page.getByTestId("series-ribbon")).toHaveCount(0);
  await page.keyboard.press("ArrowRight");
  await expect(position).toHaveText("Stack 3 / 3");
  await expect(count).toHaveText("1 of 2");
  await expect(page.getByTestId("upcoming-remaining")).toContainText(
    "last stack",
  );
  // Stacks already looked at stay in the pane, to go back to.
  await expect(page.getByTestId("upcoming-entry")).toHaveCount(3);

  // Back: the previous stack is entered at its last frame.
  await page.keyboard.press("ArrowLeft");
  await expect(position).toHaveText("Stack 2 / 3");
  await page.keyboard.press("ArrowLeft");
  await expect(position).toHaveText("Stack 1 / 3");
  await expect(count).toHaveText("3 of 3");
  // Shift skips the rest of a stack.
  await page.keyboard.press("Shift+ArrowRight");
  await expect(position).toHaveText("Stack 2 / 3");

  // Choosing a stack from the pane opens it.
  await page.getByTestId("upcoming-entry").nth(2).click();
  await expect(position).toHaveText("Stack 3 / 3");
  await page.getByTestId("upcoming-entry").nth(0).click();
  await expect(position).toHaveText("Stack 1 / 3");
  // The pane widens by dragging its edge, and its thumbnails with it.
  const thumb = page.getByTestId("upcoming-entry").first().locator("img");
  const before = (await thumb.boundingBox())!.width;
  const handle = page
    .getByTestId("upcoming-stacks")
    .getByTestId("resize-handle");
  const edge = (await handle.boundingBox())!;
  await page.mouse.move(edge.x + 4, edge.y + 200);
  await page.mouse.down();
  await page.mouse.move(edge.x + 160, edge.y + 200, { steps: 5 });
  await page.mouse.up();
  expect((await thumb.boundingBox())!.width).toBeGreaterThan(before * 1.5);
  // N hides the pane.
  await page.keyboard.press("n");
  await expect(page.getByTestId("upcoming-stacks")).toHaveCount(0);
  await page.keyboard.press("n");
  await page.screenshot({ path: test.info().outputPath("stacks.png") });
  await page.keyboard.press("Escape");
});

test("Bird ID asks for a box, then for the model when it is not downloaded", async ({
  page,
  request,
}) => {
  const DAY = "2004-05-05";
  await page.goto("/");
  await importFiles(request, {
    "birdid/one.JPG": {
      bytes: jpeg(8810),
      time: new Date(`${DAY}T07:00:00`),
    },
  });
  await page.reload();
  await showDay(page, DAY);
  await page.getByTestId("timeline").locator("button.tile").first().dblclick();
  await expect(page.getByTestId("loupe-image")).toBeVisible();

  await page.keyboard.press("d");
  await expect(page.getByTestId("bird-id-draw")).toBeVisible();
  const boxer = page.getByTestId("bird-id-boxer");
  const box = (await boxer.boundingBox())!;
  await page.mouse.move(box.x + box.width * 0.3, box.y + box.height * 0.3);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.6, box.y + box.height * 0.6, {
    steps: 5,
  });
  await page.mouse.up();

  await expect(page.getByTestId("bird-id")).toContainText("Download");
  // Finding every bird needs the models too.
  await page.keyboard.press("f");
  await expect(page.getByTestId("bird-id")).toContainText("Download the");
  await page.screenshot({ path: test.info().outputPath("bird-id.png") });
  // Esc closes the panel, then the loupe.
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("bird-id")).toHaveCount(0);
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("loupe")).toHaveCount(0);
});

test("Find birds numbers every bird and names the photo with several species", async ({
  page,
  request,
}) => {
  const DAY = "2004-05-15";
  await page.goto("/");
  await importFiles(request, {
    "findbirds/pair.JPG": {
      bytes: await similar(page, 90),
      time: new Date(`${DAY}T07:00:00`),
    },
  });
  // The bird models are not in the test manifest: the finder's answer is
  // stubbed; choosing species goes to the real backend.
  const species = (common: string, code: string) => ({
    common,
    scientific: `${common} (sci)`,
    code,
  });
  const mallard = species("Mallard", "mallar3");
  const wood = species("Wood Duck", "wooduc");
  await page.route("**/api/commands/find_birds", (route) =>
    route.fulfill({
      json: {
        region: null,
        birds: [
          {
            box: { x: 0.1, y: 0.2, width: 0.3, height: 0.4 },
            score: 0.95,
            suggestions: [
              { species: mallard, confidence: 0.81 },
              { species: species("Hawaiian Duck", "hawduc"), confidence: 0.1 },
            ],
            elsewhere: null,
          },
          {
            box: { x: 0.55, y: 0.3, width: 0.3, height: 0.4 },
            score: 0.9,
            suggestions: [{ species: wood, confidence: 0.7 }],
            elsewhere: null,
          },
        ],
      },
    }),
  );
  await page.reload();
  await showDay(page, DAY);
  await page.getByTestId("timeline").locator("button.tile").first().dblclick();
  await expect(page.getByTestId("loupe-image")).toBeVisible();

  await page.keyboard.press("f");
  await expect(page.getByTestId("bird-id-box")).toHaveCount(2);
  await expect(page.getByTestId("bird-id-bird")).toHaveCount(2);
  // The first bird is open: add its top suggestion.
  await page.getByTestId("bird-id-suggestion").first().click();
  // Choose the second bird by its box on the photo, and add it too.
  await page.getByTestId("bird-id-box").nth(1).click();
  await page.getByTestId("bird-id-suggestion").first().click();

  await expect(page.getByTestId("bird-id-current")).toContainText("Mallard");
  await expect(page.getByTestId("bird-id-current")).toContainText("Wood Duck");
  await expect(page.getByTestId("loupe-species")).toContainText("Wood Duck");
  await page.screenshot({ path: test.info().outputPath("find-birds.png") });
  const id = (
    await call<Photos>(request, "query_photos", { from: DAY, to: DAY })
  ).photos[0]!.id;
  const saved = await call<{ species: { code: string }[] }[]>(
    request,
    "photo_species",
    { photo_ids: [id] },
  );
  expect(saved[0]!.species.map((s) => s.code)).toEqual(["mallar3", "wooduc"]);

  // Removing one keeps the other.
  await page.getByTestId("bird-id-forget").first().click();
  await expect(page.getByTestId("bird-id-current")).not.toContainText(
    "Mallard",
  );
  await expect(page.getByTestId("bird-id-current")).toContainText("Wood Duck");
});

test("Clear view removes the photos in view, and only those", async ({
  page,
  request,
}) => {
  const DAY = "2004-06-06";
  const OTHER = "2004-06-07";
  await page.goto("/");
  await importFiles(request, {
    "clearview/a.JPG": { bytes: jpeg(8820), time: new Date(`${DAY}T07:00:00`) },
    "clearview/b.JPG": { bytes: jpeg(8821), time: new Date(`${DAY}T08:00:00`) },
    "clearview/c.JPG": {
      bytes: jpeg(8822),
      time: new Date(`${OTHER}T08:00:00`),
    },
  });
  await page.reload();
  await showDay(page, DAY);
  await expect(page.getByTestId("library-count")).toHaveText("2 photos");

  await page.getByTestId("open-clear-view").click();
  await expect(page.getByTestId("clear-view-summary")).toContainText(
    "the 2 photos matching the filters",
  );
  await expect(page.getByTestId("clear-view-summary")).toContainText(
    "No file is deleted",
  );
  await page.getByTestId("clear-view-go").click();

  await expect(page.getByTestId("no-match")).toBeVisible();
  const left = async (day: string) =>
    (await call<Photos>(request, "query_photos", { from: day, to: day })).photos
      .length;
  expect(await left(DAY)).toBe(0);
  expect(await left(OTHER)).toBe(1);
});

test("fast mode shows in the bar, hides auto-pick and is left with Score photos", async ({
  page,
  request,
}) => {
  await page.goto("/");
  await importFiles(request, {
    "fast/a.JPG": { bytes: jpeg(8830), time: new Date("2004-07-07T07:00:00") },
  });
  await call(request, "update_settings", { fast_mode: true });
  await page.reload();
  await expect(page.getByTestId("fast-mode-on")).toBeVisible();
  await expect(page.getByTestId("filter-auto-picked")).toHaveCount(0);

  await page.getByTestId("score-photos").click();
  await expect(page.getByTestId("fast-mode-on")).toHaveCount(0);
  await expect(page.getByTestId("filter-auto-picked")).toBeVisible();
  const settings = await call<{ fast_mode: boolean }>(
    request,
    "get_settings",
    null,
  );
  expect(settings.fast_mode).toBe(false);
});
