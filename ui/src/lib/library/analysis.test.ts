import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AnalysisStatus } from "../api";
import {
  AnalysisProgress,
  PRIORITY_DELAY_MS,
  type AnalysisApi,
} from "./analysis.svelte";

function status(analyzed: number, total = 10): AnalysisStatus {
  return { analyzed, total, running: analyzed < total };
}

function api(initial = status(2)) {
  return {
    analysisStatus: vi.fn<AnalysisApi["analysisStatus"]>(async () => initial),
    prioritizeAnalysis: vi.fn<AnalysisApi["prioritizeAnalysis"]>(
      async () => initial,
    ),
  };
}

describe("AnalysisProgress", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("loads status and follows progress events", async () => {
    const backend = api();
    const progress = new AnalysisProgress(backend);
    expect(progress.remaining).toBe(0);

    await progress.load();
    expect(progress.remaining).toBe(8);

    progress.handle({ type: "analysis_progress", ...status(9) });
    expect(progress.status).toEqual(status(9));
    expect(progress.remaining).toBe(1);
  });

  it("sends visible photos once scrolling settles", async () => {
    const backend = api();
    const progress = new AnalysisProgress(backend);
    await progress.load();

    progress.visible([1, 2]);
    progress.visible([2, 3]);
    vi.advanceTimersByTime(PRIORITY_DELAY_MS - 1);
    expect(backend.prioritizeAnalysis).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(backend.prioritizeAnalysis).toHaveBeenCalledExactlyOnceWith([2, 3]);

    // The same photos again (a re-render) are not resent.
    progress.visible([2, 3]);
    vi.advanceTimersByTime(PRIORITY_DELAY_MS);
    expect(backend.prioritizeAnalysis).toHaveBeenCalledTimes(1);
  });

  it("asks nothing when analysis is done", async () => {
    const backend = api(status(10));
    const progress = new AnalysisProgress(backend);
    await progress.load();

    progress.visible([1]);
    vi.advanceTimersByTime(PRIORITY_DELAY_MS);
    expect(backend.prioritizeAnalysis).not.toHaveBeenCalled();
  });
});
