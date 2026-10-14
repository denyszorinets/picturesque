import { readFileSync, utimesSync } from "node:fs";
import { createServer, type Server } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, type APIRequestContext, type Page } from "@playwright/test";
import { folder } from "./fixtures";

/**
 * The library folder all tests use. A catalog holds photos from one library
 * folder, so tests must not switch folders (the backend refuses once the
 * library has photos).
 */
export const LIBRARY = join(tmpdir(), "picturesque-e2e-library");

export async function call<T>(
  request: APIRequestContext,
  command: string,
  args: unknown,
): Promise<T> {
  const response = await request.post(`/api/commands/${command}`, {
    data: args,
  });
  expect(response.ok(), await response.text()).toBeTruthy();
  return (await response.json()) as T;
}

interface Job {
  source_path: string;
  stage: string;
}

/**
 * Waits until the pipeline has scanned `folder` and finished every photo
 * found in it.
 */
export async function waitForFolder(
  request: APIRequestContext,
  folder: string,
): Promise<Job[]> {
  let jobs: Job[] = [];
  await expect
    .poll(
      async () => {
        const status = await call<{ scanning: boolean }>(
          request,
          "pipeline_status",
          null,
        );
        jobs = (
          await call<Job[]>(request, "pipeline_jobs", { limit: 500 })
        ).filter((j) => j.source_path.startsWith(folder + "/"));
        return (
          !status.scanning &&
          jobs.length > 0 &&
          jobs.every((j) =>
            ["imported", "duplicate", "failed"].includes(j.stage),
          )
        );
      },
      { timeout: 60_000 },
    )
    .toBe(true);
  return jobs;
}

/**
 * Imports `files` (name → bytes, optionally with a capture time used as the
 * file's modification time) through the API into the shared library.
 */
export async function importFiles(
  request: APIRequestContext,
  files: Record<string, { bytes: Buffer; time?: Date }>,
): Promise<void> {
  const src = folder(
    Object.fromEntries(
      Object.entries(files).map(([name, f]) => [name, f.bytes]),
    ),
  );
  for (const [name, f] of Object.entries(files)) {
    if (f.time) utimesSync(join(src, name), f.time, f.time);
  }
  await call(request, "update_settings", { library_root: LIBRARY });
  await call(request, "add_import_source", { path: src, watch: false });
  await waitForFolder(request, src);
}

/**
 * Serves the ONNX test fixtures at the URLs of fixtures/models.json
 * (http://127.0.0.1:5175/<file>).
 */
export async function serveModels(): Promise<Server> {
  const fixtures = new URL(
    "../../crates/infra/tests/fixtures/onnx/",
    import.meta.url,
  );
  const files = [
    "affine.onnx",
    "tiny_clip.onnx",
    "tiny_clip_text.onnx",
    "tiny_merges.txt",
  ];
  const server = createServer((req, res) => {
    const name = files.find((f) => req.url === `/${f}`);
    if (!name) {
      res.writeHead(404).end();
      return;
    }
    const body = readFileSync(new URL(name, fixtures));
    res.writeHead(200, { "content-length": body.length });
    res.end(body);
  });
  await new Promise<void>((resolve) =>
    server.listen(5175, "127.0.0.1", resolve),
  );
  return server;
}

/** A detailed JPEG drawn in the browser, optionally blurred. */
export async function drawn(
  page: Page,
  blur: number,
  hue: number,
): Promise<Buffer> {
  const data = await page.evaluate(
    ([blur, hue]) => {
      const canvas = document.createElement("canvas");
      canvas.width = 2400;
      canvas.height = 1600;
      const ctx = canvas.getContext("2d")!;
      ctx.fillStyle = `hsl(${hue} 50% 45%)`;
      ctx.fillRect(0, 0, 2400, 1600);
      for (let x = 0; x < 2400; x += 24)
        for (let y = 0; y < 1600; y += 24) {
          ctx.fillStyle = (x + y) % 48 === 0 ? "#111" : "#eee";
          ctx.fillRect(x, y, 12, 12);
        }
      ctx.fillStyle = "#000";
      ctx.fillRect(0, 0, 1200, 800);
      let out = canvas;
      if (blur > 0) {
        out = document.createElement("canvas");
        out.width = canvas.width;
        out.height = canvas.height;
        const blurred = out.getContext("2d")!;
        blurred.filter = `blur(${blur}px)`;
        blurred.drawImage(canvas, 0, 0);
      }
      return out.toDataURL("image/jpeg", 0.92).split(",")[1]!;
    },
    [blur, hue],
  );
  return Buffer.from(data, "base64");
}
