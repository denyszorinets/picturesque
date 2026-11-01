import { describe, expect, it } from "vitest";
import type { BackgroundCritiqueStatus } from "../api";
import { CritiqueProgress } from "./critique.svelte";

export const STATUS: BackgroundCritiqueStatus = {
  enabled: true,
  running: true,
  paused: null,
  critiqued: 3,
  stale: 1,
  missing: 6,
  failed: 0,
  total: 10,
  seconds_per_photo: 20,
  seconds_left: 140,
};

describe("CritiqueProgress", () => {
  it("loads the status and follows events", async () => {
    const progress = new CritiqueProgress({
      backgroundCritiqueStatus: async () => STATUS,
    });
    expect(progress.status).toBeNull();
    await progress.load();
    expect(progress.status?.critiqued).toBe(3);
    progress.handle({
      type: "background_critique_progress",
      ...STATUS,
      critiqued: 4,
    });
    expect(progress.status?.critiqued).toBe(4);
    progress.handle({ type: "analysis_progress" } as never);
    expect(progress.status?.critiqued).toBe(4);
  });

  it("keeps the last status when loading fails", async () => {
    let fail = false;
    const progress = new CritiqueProgress({
      backgroundCritiqueStatus: async () => {
        if (fail) throw new Error("down");
        return STATUS;
      },
    });
    await progress.load();
    fail = true;
    await progress.load();
    expect(progress.status).toEqual(STATUS);
  });
});
