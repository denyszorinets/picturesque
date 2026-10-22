// Captures the screenshots of the landing page and the docs
// (docs/development/website.rst). Run by `make screenshots`, which builds
// the demo card and starts the dev server and Vite first.
//
//   node scripts/screenshots/capture.mjs <card folder> <library folder> <folder-session folder>
//
// Imports the card into the (empty) dev server, waits for analysis, makes
// some decisions, then captures each screen in light and dark mode:
// site/assets/<name>-<theme>.webp (2400 px wide) and, in light mode,
// docs/images/screenshots/<name>.webp (1600 px wide).

import { execFileSync } from "node:child_process";
import { cpSync, mkdirSync, readdirSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";
import { chromium } from "@playwright/test";

const API = "http://127.0.0.1:5174/api/commands";
const UI = "http://127.0.0.1:5173";
const [card, library, folder] = process.argv.slice(2).map((p) => resolve(p));
const repo = resolve(import.meta.dirname, "../..");
const shots = join(repo, "target/screenshots/png");
const THEMES = ["light", "dark"];

async function call(command, args = null) {
  const response = await fetch(`${API}/${command}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(args),
  });
  const body = await response.json();
  if (!response.ok) throw new Error(`${command}: ${JSON.stringify(body)}`);
  return body;
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function until(what, check, timeout = 900_000) {
  const end = Date.now() + timeout;
  while (!(await check())) {
    if (Date.now() > end) throw new Error(`timed out: ${what}`);
    await sleep(2000);
  }
  console.log(`screenshots: ${what}`);
}

/** Every photo in the session, oldest first. */
async function photos() {
  const all = [];
  let cursor;
  do {
    const page = await call("query_photos", {
      sort: "captured_asc",
      limit: 500,
      cursor,
    });
    all.push(...page.photos);
    cursor = page.next_cursor ?? undefined;
  } while (cursor);
  return all;
}

async function analyzed(total) {
  await until(`${total} photos imported and analyzed`, async () => {
    const s = await call("analysis_status");
    return s.total >= total && s.analyzed >= s.total && !s.running;
  });
}

async function setUp(total) {
  await call("update_settings", {
    library_root: library,
    onboarding: { tour_done: true },
  });
  // Aesthetic scores, similar photos and closed eyes need the models.
  for (const id of ["clip-image", "faces"]) await call("install_model", { id });
  await until("models installed", async () =>
    (await call("list_models"))
      .filter((m) => ["clip-image", "faces"].includes(m.id))
      .every((m) => m.state === "installed"),
  );
  await call("add_import_source", { path: card, watch: false });
  await analyzed(total);
}

/** Picks the best photo of each Lapland and Dolomites series, rejects the
 * rest of the Dolomites, rates the portraits; the other trips stay
 * suggestions (auto-picked). */
async function decide() {
  const all = await photos();
  const changes = [];
  for (const p of all) {
    if (p.camera?.includes("X-T5") || p.camera?.includes("ILCE-7M4")) {
      if (p.best_in_series) changes.push({ photo_id: p.id, flag: "pick", rating: 4 });
    }
    if (p.camera?.includes("R6") && p.best_in_series)
      changes.push({ photo_id: p.id, rating: 5 });
  }
  // Rejects: the two weakest frames of each Dolomites series.
  const bySeries = new Map();
  for (const p of all)
    if (p.camera?.includes("ILCE-7M4") && p.series_id !== null && !p.best_in_series)
      bySeries.set(p.series_id, [...(bySeries.get(p.series_id) ?? []), p]);
  for (const members of bySeries.values())
    for (const p of members.sort((a, b) => (a.quality ?? 0) - (b.quality ?? 0)).slice(0, 2))
      changes.push({ photo_id: p.id, flag: "reject" });
  if (changes.length) await call("set_decisions", { changes });
  console.log(`screenshots: ${changes.length} decisions`);
  return all;
}

function save(name, theme) {
  const png = join(shots, `${name}-${theme}.png`);
  const site = join(repo, "site/assets", `${name}-${theme}.webp`);
  execFileSync("vips", ["thumbnail", png, `${site}[Q=80,strip]`, "2400"]);
  if (theme === "light") {
    const docs = join(repo, "docs/images/screenshots", `${name}.webp`);
    execFileSync("vips", ["thumbnail", png, `${docs}[Q=80,strip]`, "1600"]);
  }
}

/** Waits until every image on screen has loaded. */
async function settled(page) {
  await page.waitForLoadState("networkidle");
  await page.waitForFunction(() =>
    [...document.images].every((img) => img.complete && img.naturalWidth > 0),
  );
  await sleep(600);
}

async function shoot(page, name, theme) {
  await settled(page);
  await page.screenshot({ path: join(shots, `${name}-${theme}.png`) });
  save(name, theme);
  console.log(`screenshots: ${name}-${theme}`);
}

async function open(browser, theme) {
  const context = await browser.newContext({
    viewport: { width: 1600, height: 1000 },
    deviceScaleFactor: 2,
    colorScheme: theme,
    reducedMotion: "reduce",
  });
  const page = await context.newPage();
  await page.goto(UI);
  await page.locator("button.tile").first().waitFor({ timeout: 60_000 });
  return { context, page };
}

const tile = (page, id) => page.locator(`[data-photo-id="${id}"]`);

/** Scrolls the timeline until the photo's tile is rendered: only rows near
 * the visible ones are in the page. */
async function reveal(page, id) {
  const timeline = page.getByTestId("timeline");
  await timeline.evaluate((el) => (el.scrollTop = 0));
  for (let i = 0; i < 300; i++) {
    if ((await tile(page, id).count()) > 0) {
      // Rows re-render while scrolling settles; try again if this one was
      // replaced.
      await sleep(300);
      try {
        await tile(page, id).scrollIntoViewIfNeeded({ timeout: 3000 });
        await sleep(300);
        return;
      } catch {
        continue;
      }
    }
    await timeline.evaluate((el) => (el.scrollTop += el.clientHeight * 0.8));
    await sleep(150);
  }
  throw new Error(`photo ${id} is not in the timeline`);
}
const nav = (page, name) =>
  page.getByRole("navigation", { name: "Main" }).getByRole("button", { name });

async function libraryScreens(browser, all) {
  const portrait = all.find((p) => p.camera?.includes("R6") && p.best_in_series);
  const fox = all.find((p) => p.camera?.includes("ILCE-1") && p.series_size > 0);
  for (const theme of THEMES) {
    const { context, page } = await open(browser, theme);
    await shoot(page, "library", theme);

    await reveal(page, portrait.id);
    await tile(page, portrait.id).dblclick();
    // The info panel (camera, place, why it scored) opens with the loupe;
    // open it only if it is closed.
    await sleep(500);
    if ((await page.getByTestId("loupe-info").count()) === 0)
      await page.getByRole("button", { name: "Info" }).click();
    await page.getByTestId("loupe-info").waitFor();
    await shoot(page, "loupe", theme);
    await page.keyboard.press("Escape");

    await reveal(page, fox.id);
    await tile(page, fox.id).click();
    await page.keyboard.press("c");
    await shoot(page, "compare", theme);
    await page.keyboard.press("Escape");

    await nav(page, "Statistics").click();
    await shoot(page, "stats", theme);
    await nav(page, "Activity").click();
    await shoot(page, "activity", theme);
    await context.close();
  }
}

async function folderScreens(browser) {
  for (const theme of THEMES) {
    const { context, page } = await open(browser, theme);
    await shoot(page, "folder", theme);
    await page.getByTestId("open-export").click();
    await shoot(page, "copy-keepers", theme);
    await context.close();
  }
}

mkdirSync(shots, { recursive: true });
mkdirSync(join(repo, "docs/images/screenshots"), { recursive: true });
const total = Number(
  execFileSync("find", [card, "-name", "*.JPG"], { encoding: "utf8" })
    .trim()
    .split("\n").length,
);
await setUp(total);
const all = await decide();
const browser = await chromium.launch();
try {
  await libraryScreens(browser, all);
  // A folder session: one trip, culled where it is (darktable).
  rmSync(folder, { recursive: true, force: true });
  cpSync(join(card, "DCIM/Wildlife"), folder, { recursive: true });
  await call("open_folder", { path: folder });
  await analyzed(readdirSync(folder).filter((f) => f.endsWith(".JPG")).length);
  await folderScreens(browser);
} finally {
  await browser.close();
}
console.log("screenshots: done");
