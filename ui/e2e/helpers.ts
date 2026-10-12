import { utimesSync } from "node:fs";
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

async function waitFor(
  request: APIRequestContext,
  runId: number,
  state: string,
) {
  await expect
    .poll(
      async () =>
        (
          await call<{ state: string }>(request, "import_status", {
            run_id: runId,
          })
        ).state,
      {
        timeout: 60_000,
      },
    )
    .toBe(state);
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
  const plan = await call<{ run_id: number }>(request, "plan_import", {
    sources: [src],
  });
  await waitFor(request, plan.run_id, "planned");
  await call(request, "start_import", { run_id: plan.run_id });
  await waitFor(request, plan.run_id, "completed");
}
