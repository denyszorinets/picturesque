// Smoke test of the real desktop app (#54, #108): starts it through
// tauri-driver (WebDriver), imports fixture photos from a watched folder,
// picks one, restarts the app and checks that the library, the decision
// and the folder are still there, then drops a new photo into the folder
// and waits for it to be imported on its own.
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
const fixture = (n) =>
  join(repo, "crates/infra/tests/fixtures", `orient_${n}.jpg`);

const work = mkdtempSync(join(tmpdir(), "picturesque-smoke-"));
const library = join(work, "Library");
const card = join(work, "Card");
mkdirSync(library, { recursive: true });
mkdirSync(join(card, "DCIM"), { recursive: true });
for (const n of [1, 3, 6])
  copyFileSync(fixture(n), join(card, "DCIM", `IMG_000${n}.JPG`));

const step = (name) => console.log(`smoke: ${name}`);
const jpegsInLibrary = () =>
  readdirSync(library, { recursive: true }).filter((f) =>
    String(f).endsWith(".JPG"),
  ).length;
const nav = (tab) => `//nav//button[normalize-space()='${tab}']`;
const driver = await startDriver(work);
let window;

try {
  window = await App.launch(app, shots);

  step("the window shows the empty library");
  await window.find("//button[normalize-space()='Add import folder']");
  await window.screenshot("1-empty");

  step("set the library folder");
  await window.click(nav("Settings"));
  await window.type("[data-testid=library-root]", library);
  await window.click("[data-testid=save-settings]");
  await until(
    "settings saved",
    async () => (await window.enabled("[data-testid=save-settings]")) === false,
  );

  step("add the card as a watched import folder");
  await window.click(nav("Library"));
  await window.click("//button[normalize-space()='Add import folder']");
  await window.type("[data-testid=source-path]", card);
  await window.click("[data-testid=add-source]");

  step("photos appear in the library without reloading");
  await window.click(nav("Library"));
  await until(
    "3 photos in the library",
    async () => (await window.count("button.tile")) === 3,
  );
  if (jpegsInLibrary() !== 3)
    throw new Error(`expected 3 imported files, found ${jpegsInLibrary()}`);

  step("each photo's steps are shown");
  await window.click(nav("Activity"));
  await until(
    "3 photos imported",
    async () => (await window.count("tr[data-stage=imported]")) === 3,
  );
  await window.screenshot("2-activity");

  step("pick a photo");
  await window.click(nav("Library"));
  await window.click("button.tile");
  await window.press("p");
  await window.find("button.tile .badge.pick", 10_000);
  await until("the picks count follows", async () =>
    (await window.text("//aside//button[contains(., 'Picks')]")).includes("1"),
  );
  await window.screenshot("3-picked");

  step("reject in the loupe with its button, then pick it again with P");
  await window.press("\uE007"); // Enter
  await window.click("[data-testid=loupe-reject]");
  await until(
    "the reject shows",
    async () =>
      (await window.run(
        "return document.querySelector('[data-testid=loupe-reject]').getAttribute('aria-pressed')",
      )) === "true",
  );
  await window.screenshot("3b-loupe");
  await window.press("p");
  await window.press("\uE00C"); // Escape
  await window.find("button.tile .badge.pick", 10_000);

  step("restart the app");
  await window.close();
  window = await App.launch(app, shots);

  step("the library, the pick and the folder are still there");
  await until(
    "3 photos after restart",
    async () => (await window.count("button.tile")) === 3,
  );
  await window.find("button.tile .badge.pick", 10_000);
  await window.click(nav("Activity"));
  await until("the folder is listed", async () =>
    (await window.text("[data-testid=sources]")).includes(card),
  );
  await window.screenshot("4-restarted");

  step("a photo dropped into the folder is imported on its own");
  await window.click(nav("Library"));
  copyFileSync(fixture(8), join(card, "DCIM", "IMG_0008.JPG"));
  await until(
    "4 photos in the library",
    async () => (await window.count("button.tile")) === 4,
  );
  if (jpegsInLibrary() !== 4)
    throw new Error(`expected 4 imported files, found ${jpegsInLibrary()}`);
  await window.screenshot("5-auto-imported");
  step("passed");
} catch (e) {
  console.error(`smoke: FAILED: ${e.message}`);
  await window?.screenshot("failure").catch(() => {});
  process.exitCode = 1;
} finally {
  await window?.close();
  driver.kill();
}
