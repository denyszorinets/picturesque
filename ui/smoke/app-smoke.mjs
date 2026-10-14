// Smoke test of the real desktop app (#54): starts it through tauri-driver
// (WebDriver), imports fixture photos and picks one.
//
//   node ui/smoke/app-smoke.mjs <path to picturesque-desktop> [screenshot dir]
//
// Needs tauri-driver and WebKitWebDriver on PATH and an X display (Xvfb in
// CI). The app gets fresh data, cache and config folders.

import { copyFileSync, mkdirSync, mkdtempSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { App, startDriver, until } from "./driver.mjs";

const app = resolve(process.argv[2] ?? "target/release/picturesque-desktop");
const shots = resolve(process.argv[3] ?? "smoke-results");
const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

const work = mkdtempSync(join(tmpdir(), "picturesque-smoke-"));
const library = join(work, "Library");
const card = join(work, "Card", "DCIM");
mkdirSync(library, { recursive: true });
mkdirSync(card, { recursive: true });
for (const n of [1, 3, 6]) {
  copyFileSync(
    join(repo, "crates/infra/tests/fixtures", `orient_${n}.jpg`),
    join(card, `IMG_000${n}.JPG`),
  );
}

const step = (name) => console.log(`smoke: ${name}`);
const driver = await startDriver(work);
let window;

try {
  window = await App.launch(app, shots);

  step("the window shows the empty library");
  await window.find("//button[normalize-space()='Import photos']");
  await window.screenshot("1-empty");

  step("set the library folder");
  await window.click("//nav//button[normalize-space()='Import']");
  await window.type("[data-testid=library-root]", library);
  await window.click("[data-testid=save-settings]");
  await until(
    "settings saved",
    async () => (await window.enabled("[data-testid=save-settings]")) === false,
  );

  step("import the card");
  await window.type("[data-testid=source-path]", card);
  await window.click("[data-testid=add-source]");
  await window.click("[data-testid=plan-import]");
  await window.click("[data-testid=start-import]");
  await until("import finished", async () =>
    (await window.text("[data-testid=import-outcome]")).includes(
      "Import finished",
    ),
  );
  const imported = readdirSync(library, { recursive: true }).filter((f) =>
    String(f).endsWith(".JPG"),
  );
  if (imported.length !== 3)
    throw new Error(`expected 3 imported files, found ${imported}`);

  step("pick a photo");
  await window.click("//nav//button[normalize-space()='Library']");
  await window.click("button.tile");
  await window.press("p");
  await window.find("button.tile .badge.pick", 10_000);
  await until("the picks count follows", async () =>
    (await window.text("//aside//button[contains(., 'Picks')]")).includes("1"),
  );
  await window.screenshot("2-picked");
  step("passed");
} catch (e) {
  console.error(`smoke: FAILED: ${e.message}`);
  await window?.screenshot("failure").catch(() => {});
  process.exitCode = 1;
} finally {
  await window?.close();
  driver.kill();
}
