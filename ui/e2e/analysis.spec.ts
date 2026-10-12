import { expect, test } from "@playwright/test";
import { call } from "./helpers";

type Status = { analyzed: number; total: number; running: boolean };

test("reports analysis progress and accepts priorities", async ({
  request,
}) => {
  const status = await call<Status>(request, "analysis_status", null);
  expect(status.analyzed).toBeLessThanOrEqual(status.total);

  const after = await call<Status>(request, "prioritize_analysis", {
    photo_ids: [1, 2, 3],
  });
  expect(after.total).toBe(status.total);

  const bad = await request.post("/api/commands/prioritize_analysis", {
    data: { photo_ids: "all" },
  });
  expect(bad.status()).toBe(400);
});
