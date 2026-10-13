// Smoke test of the real desktop app (#54): starts it through tauri-driver
// (WebDriver), imports fixture photos and picks one.
//
//   node ui/smoke/app-smoke.mjs <path to picturesque-desktop> [screenshot dir]
//
// Needs tauri-driver and WebKitWebDriver on PATH and an X display (Xvfb in
// CI). The app gets fresh data, cache and config folders.

import { Buffer } from "node:buffer";
import { spawn } from "node:child_process";
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const app = resolve(process.argv[2] ?? "target/release/picturesque-desktop");
const shots = resolve(process.argv[3] ?? "smoke-results");
const DRIVER = "http://127.0.0.1:4444";
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

const env = {
  ...process.env,
  XDG_DATA_HOME: join(work, "data"),
  XDG_CACHE_HOME: join(work, "cache"),
  XDG_CONFIG_HOME: join(work, "config"),
  GDK_BACKEND: "x11",
};
delete env.WAYLAND_DISPLAY;
const driver = spawn("tauri-driver", [], { env, stdio: "inherit" });

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let session;

async function wd(method, path, body) {
  const response = await fetch(`${DRIVER}${path}`, {
    method,
    headers: { "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const json = await response.json();
  if (!response.ok)
    throw new Error(`${method} ${path}: ${JSON.stringify(json.value)}`);
  return json.value;
}

const s = (path) => `/session/${session}${path}`;
const ELEMENT = "element-6066-11e4-a339-4f5ad6eda2b9";

/** Waits for an element (CSS, or XPath when starting with "/"). */
async function find(selector, timeout = 30_000) {
  const using = selector.startsWith("/") ? "xpath" : "css selector";
  const end = Date.now() + timeout;
  for (;;) {
    try {
      const found = await wd("POST", s("/element"), { using, value: selector });
      // W3C key, or the legacy one some drivers still use.
      const id = found[ELEMENT] ?? found.ELEMENT ?? Object.values(found)[0];
      if (!id) throw new Error(`no element id in ${JSON.stringify(found)}`);
      return id;
    } catch (e) {
      if (Date.now() > end)
        throw new Error(`not found: ${selector} (${e.message})`, {
          cause: e,
        });
      await sleep(250);
    }
  }
}

async function until(what, check, timeout = 60_000) {
  const end = Date.now() + timeout;
  while (!(await check().catch(() => false))) {
    if (Date.now() > end) throw new Error(`timed out: ${what}`);
    await sleep(250);
  }
}

const click = async (selector) =>
  wd("POST", s(`/element/${await find(selector)}/click`), {});
async function type(selector, text) {
  const el = await find(selector);
  await wd("POST", s(`/element/${el}/clear`), {});
  await wd("POST", s(`/element/${el}/value`), { text });
}
const text = async (selector) =>
  wd("GET", s(`/element/${await find(selector)}/text`));
async function screenshot(name) {
  mkdirSync(shots, { recursive: true });
  const png = await wd("GET", s("/screenshot"));
  writeFileSync(join(shots, `${name}.png`), Buffer.from(png, "base64"));
}

const step = (name) => console.log(`smoke: ${name}`);

try {
  await until(
    "tauri-driver",
    () => fetch(`${DRIVER}/status`).then((r) => r.ok),
    30_000,
  );
  session = (
    await wd("POST", "/session", {
      capabilities: { alwaysMatch: { "tauri:options": { application: app } } },
    })
  ).sessionId;

  step("the window shows the empty library");
  await find("//button[normalize-space()='Import photos']");
  await screenshot("1-empty");

  step("set the library folder");
  await click("//nav//button[normalize-space()='Import']");
  await type("[data-testid=library-root]", library);
  await click("[data-testid=save-settings]");
  await until(
    "settings saved",
    async () =>
      (await wd(
        "GET",
        s(`/element/${await find("[data-testid=save-settings]")}/enabled`),
      )) === false,
  );

  step("import the card");
  await type("[data-testid=source-path]", card);
  await click("[data-testid=add-source]");
  await click("[data-testid=plan-import]");
  await find("[data-testid=start-import]");
  await click("[data-testid=start-import]");
  await until("import finished", async () =>
    (await text("[data-testid=import-outcome]")).includes("Import finished"),
  );
  const imported = readdirSync(library, { recursive: true }).filter((f) =>
    String(f).endsWith(".JPG"),
  );
  if (imported.length !== 3)
    throw new Error(`expected 3 imported files, found ${imported}`);

  step("pick a photo");
  await click("//nav//button[normalize-space()='Library']");
  await click("button.tile");
  await wd("POST", s("/actions"), {
    actions: [
      {
        type: "key",
        id: "keyboard",
        actions: [
          { type: "keyDown", value: "p" },
          { type: "keyUp", value: "p" },
        ],
      },
    ],
  });
  await find("button.tile .badge.pick", 10_000);
  await until("the picks count follows", async () =>
    (await text("//aside//button[contains(., 'Picks')]")).includes("1"),
  );
  await screenshot("2-picked");
  step("passed");
} catch (e) {
  console.error(`smoke: FAILED: ${e.message}`);
  if (session) await screenshot("failure").catch(() => {});
  process.exitCode = 1;
} finally {
  if (session) await wd("DELETE", s("")).catch(() => {});
  driver.kill();
}
