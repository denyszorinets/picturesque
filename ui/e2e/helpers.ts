import { readFileSync, utimesSync } from "node:fs";
import { createServer, type Server } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, type APIRequestContext } from "@playwright/test";
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
