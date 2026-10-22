import { describe, expect, it } from "vitest";
import type { AnalysisStatus, PipelineStatus } from "../api";
import { ringState } from "./ring";

const idle: PipelineStatus = {
  queued: 0,
  imported: 40,
  duplicates: 0,
  failed: 0,
  active: null,
  scanning: false,
  paused: null,
};

function analysis(analyzed: number, total = 40): AnalysisStatus {
  return {
    analyzed,
    total,
    running: analyzed < total,
    paused: null,
    waiting_for_models: [],
    ai_unavailable: null,
  };
}

describe("ringState", () => {
  it("is unknown until a status arrives", () => {
    expect(ringState(null, null)).toBeNull();
  });

  it("shows importing before analysis", () => {
    const ring = ringState(
      {
        ...idle,
        queued: 12,
        active: { job_id: 3, file_name: "IMG_3.CR3", stage: "copying" },
      },
      analysis(5),
    );
    expect(ring).toMatchObject({
      label: "Importing · 12 left",
      tone: "busy",
      progress: null,
    });
    expect(ring?.details).toEqual([
      "Importing: 12 left — copying IMG_3.CR3",
      "Analyzing: 5 / 40 (photos on screen go first)",
    ]);
  });

  it("shows analysis progress as a fraction", () => {
    expect(ringState(idle, analysis(10))).toMatchObject({
      label: "Analyzing 10 / 40",
      tone: "busy",
      progress: 0.25,
    });
  });

  it("says why work is paused", () => {
    expect(
      ringState({ ...idle, queued: 3, paused: "The disk is full" }, null),
    ).toMatchObject({ label: "Import paused", tone: "paused" });
    expect(
      ringState(idle, { ...analysis(10), paused: "Cache full; retrying" })
        ?.details[1],
    ).toBe("Analysis paused: Cache full (10 / 40)");
  });

  it("is done when nothing waits, and keeps counting failures", () => {
    const ring = ringState({ ...idle, failed: 2 }, analysis(40));
    expect(ring).toMatchObject({
      label: "Up to date",
      tone: "done",
      progress: 1,
      failed: 2,
    });
    expect(ring?.details).toContain("2 photos could not be imported");
  });

  it("looks for photos while scanning", () => {
    expect(ringState({ ...idle, scanning: true }, analysis(40))?.label).toBe(
      "Looking for photos",
    );
  });
});
