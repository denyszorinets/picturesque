// A minimal W3C WebDriver client for driving the real desktop app through
// tauri-driver (ui/smoke). No window ever reaches a real screen:
// WAYLAND_DISPLAY is cleared and GDK uses X11 (Xvfb).

import { Buffer } from "node:buffer";
import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import process from "node:process";

const DRIVER = "http://127.0.0.1:4444";
const ELEMENT = "element-6066-11e4-a339-4f5ad6eda2b9";

export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

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

export async function until(what, check, timeout = 60_000) {
  const end = Date.now() + timeout;
  while (!(await check().catch(() => false))) {
    if (Date.now() > end) throw new Error(`timed out: ${what}`);
    await sleep(250);
  }
}

/** Starts tauri-driver with the app's data, cache and config under `home`. */
export async function startDriver(home) {
  const env = {
    ...process.env,
    XDG_DATA_HOME: join(home, "data"),
    XDG_CACHE_HOME: join(home, "cache"),
    XDG_CONFIG_HOME: join(home, "config"),
    GDK_BACKEND: "x11",
  };
  delete env.WAYLAND_DISPLAY;
  const driver = spawn("tauri-driver", [], { env, stdio: "inherit" });
  await until(
    "tauri-driver",
    () => fetch(`${DRIVER}/status`).then((r) => r.ok),
    30_000,
  );
  return driver;
}

/** A running app window. */
export class App {
  constructor(id, shots) {
    this.id = id;
    this.shots = shots;
  }

  /** Launches the app binary in a new WebDriver session. */
  static async launch(application, shots) {
    const value = await wd("POST", "/session", {
      capabilities: { alwaysMatch: { "tauri:options": { application } } },
    });
    return new App(value.sessionId, shots);
  }

  s(path) {
    return `/session/${this.id}${path}`;
  }

  /** Waits for an element (CSS, or XPath when starting with "/"). */
  async find(selector, timeout = 30_000) {
    const using = selector.startsWith("/") ? "xpath" : "css selector";
    const end = Date.now() + timeout;
    for (;;) {
      try {
        const found = await wd("POST", this.s("/element"), {
          using,
          value: selector,
        });
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

  /** How many elements match (CSS). */
  async count(selector) {
    const found = await wd("POST", this.s("/elements"), {
      using: "css selector",
      value: selector,
    });
    return found.length;
  }

  async click(selector) {
    await wd("POST", this.s(`/element/${await this.find(selector)}/click`), {});
  }

  async type(selector, text) {
    const el = await this.find(selector);
    await wd("POST", this.s(`/element/${el}/clear`), {});
    await wd("POST", this.s(`/element/${el}/value`), { text });
  }

  async text(selector) {
    return wd("GET", this.s(`/element/${await this.find(selector)}/text`));
  }

  async enabled(selector) {
    return wd("GET", this.s(`/element/${await this.find(selector)}/enabled`));
  }

  /** Presses a key on the focused element (or the page). */
  async press(key) {
    await wd("POST", this.s("/actions"), {
      actions: [
        {
          type: "key",
          id: "keyboard",
          actions: [
            { type: "keyDown", value: key },
            { type: "keyUp", value: key },
          ],
        },
      ],
    });
  }

  /** Runs a script in the page and returns its result. */
  async run(script, args = []) {
    return wd("POST", this.s("/execute/sync"), { script, args });
  }

  async screenshot(name) {
    mkdirSync(this.shots, { recursive: true });
    const png = await wd("GET", this.s("/screenshot"));
    writeFileSync(join(this.shots, `${name}.png`), Buffer.from(png, "base64"));
  }

  async close() {
    await wd("DELETE", this.s("")).catch(() => {});
  }
}
