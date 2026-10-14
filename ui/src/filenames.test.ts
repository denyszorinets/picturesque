import { readdirSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

/** Every file under `dir`, relative to it. */
function files(dir: string, prefix = ""): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
    e.isDirectory()
      ? files(join(dir, e.name), `${prefix}${e.name}/`)
      : [`${prefix}${e.name}`],
  );
}

describe("source file names", () => {
  // macOS and Windows file systems ignore case: `./compare.svelte` would
  // resolve to `Compare.svelte` instead of `compare.svelte.ts` there.
  it("never differ only in case, as imports resolve them", () => {
    const seen = new Map<string, string>();
    for (const file of files(join(__dirname))) {
      const key = file.toLowerCase().replace(/\.svelte\.ts$/, ".svelte");
      expect(seen.get(key), `${file} and ${seen.get(key)}`).toBeUndefined();
      seen.set(key, file);
    }
  });
});
