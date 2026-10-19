import { describe, expect, it, vi } from "vitest";
import { ApiError, type JobView, type PipelineStatus } from "../api";
import {
  isBusy,
  PAGE,
  Pipeline,
  steps,
  type PipelineApi,
} from "./pipeline.svelte";

const idle: PipelineStatus = {
  queued: 0,
  imported: 0,
  duplicates: 0,
  failed: 0,
  active: null,
  scanning: false,
  paused: null,
};

function job(overrides: Partial<JobView> = {}): JobView {
  return {
    id: 1,
    file_name: "IMG_0001.CR3",
    source_path: "/card/IMG_0001.CR3",
    files: ["IMG_0001.CR3", "IMG_0001.JPG"],
    stage: "queued",
    error: null,
    photo_id: null,
    updated_at: "2026-10-14T10:00:00Z",
    failed_stage: null,
    thumbnail: null,
    analysis: null,
    ...overrides,
  };
}

const states = (j: JobView) => steps(j).map((s) => s.state);

function api() {
  return {
    pipelineStatus: vi.fn<PipelineApi["pipelineStatus"]>(async () => idle),
    listImportSources: vi.fn<PipelineApi["listImportSources"]>(async () => []),
    addImportSource: vi.fn<PipelineApi["addImportSource"]>(
      async (path, watch) => ({
        id: 1,
        path,
        watch: watch ?? true,
        scanned_at: null,
        scan_error: null,
      }),
    ),
    removeImportSource: vi.fn<PipelineApi["removeImportSource"]>(
      async () => [],
    ),
    setImportSourceWatch: vi.fn<PipelineApi["setImportSourceWatch"]>(),
    scanImportSources: vi.fn<PipelineApi["scanImportSources"]>(
      async () => idle,
    ),
    pipelineJobs: vi.fn<PipelineApi["pipelineJobs"]>(async () => []),
    pipelineLog: vi.fn<PipelineApi["pipelineLog"]>(async () => []),
    retryFailedImports: vi.fn<PipelineApi["retryFailedImports"]>(
      async () => idle,
    ),
  };
}

describe("steps", () => {
  it("follows a photo from found to analyzed", () => {
    expect(states(job())).toEqual([
      "done",
      "pending",
      "pending",
      "pending",
      "pending",
      "pending",
      "pending",
    ]);
    expect(states(job({ stage: "reading" })).slice(0, 3)).toEqual([
      "done",
      "active",
      "pending",
    ]);
    expect(states(job({ stage: "copying" })).slice(0, 5)).toEqual([
      "done",
      "done",
      "active",
      "active",
      "pending",
    ]);
    const imported = steps(
      job({ stage: "imported", thumbnail: false, analysis: [1, 3] }),
    );
    expect(imported.map((s) => s.state)).toEqual([
      "done",
      "done",
      "done",
      "done",
      "done",
      "active",
      "active",
    ]);
    expect(imported[6]?.detail).toBe("1/3");
    expect(
      states(job({ stage: "imported", thumbnail: true, analysis: [3, 3] })),
    ).toEqual(Array(7).fill("done"));
  });

  it("shows where a photo failed and what was skipped", () => {
    expect(states(job({ stage: "failed", failed_stage: "reading" }))).toEqual([
      "done",
      "failed",
      "skipped",
      "skipped",
      "skipped",
      "skipped",
      "skipped",
    ]);
    expect(
      states(job({ stage: "failed", failed_stage: "copying" })).slice(0, 4),
    ).toEqual(["done", "done", "failed", "skipped"]);
    expect(states(job({ stage: "duplicate" })).slice(0, 3)).toEqual([
      "done",
      "done",
      "skipped",
    ]);
  });
});

describe("isBusy", () => {
  it("is busy while photos wait or a folder is scanned", () => {
    expect(isBusy(null)).toBe(false);
    expect(isBusy(idle)).toBe(false);
    expect(isBusy({ ...idle, queued: 1 })).toBe(true);
    expect(isBusy({ ...idle, scanning: true })).toBe(true);
  });
});

describe("Pipeline", () => {
  it("follows progress events", () => {
    const pipeline = new Pipeline(api());
    expect(
      pipeline.handle({ type: "pipeline_progress", ...idle, queued: 3 }),
    ).toBe(true);
    expect(pipeline.status?.queued).toBe(3);
    expect(pipeline.busy).toBe(true);
    expect(
      pipeline.handle({ type: "series_changed", series: 1, photos: 2 }),
    ).toBe(false);
  });

  it("adds folders and explains errors", async () => {
    const backend = api();
    const pipeline = new Pipeline(backend);
    expect(await pipeline.addSource("  ")).toBe(false);
    expect(backend.addImportSource).not.toHaveBeenCalled();

    expect(await pipeline.addSource(" /card ")).toBe(true);
    expect(backend.addImportSource).toHaveBeenCalledWith("/card", true);
    expect(backend.listImportSources).toHaveBeenCalled();

    backend.addImportSource.mockRejectedValueOnce(
      new ApiError("invalid_arguments", "Cannot read `/x`: missing."),
    );
    expect(await pipeline.addSource("/x")).toBe(false);
    expect(pipeline.error).toBe("Cannot read `/x`: missing.");
  });

  it("filters and pages photos and the log", async () => {
    const page = Array.from({ length: PAGE }, (_, i) => job({ id: 500 - i }));
    const backend = api();
    backend.pipelineJobs.mockImplementation(async (args) =>
      args.before ? [job({ id: 1 })] : page,
    );
    const pipeline = new Pipeline(backend);
    await pipeline.refresh();
    expect(pipeline.jobs).toHaveLength(PAGE);
    expect(pipeline.jobsComplete).toBe(false);

    await pipeline.moreJobs();
    expect(backend.pipelineJobs).toHaveBeenLastCalledWith({
      filter: "all",
      limit: PAGE,
      before: 401,
    });
    expect(pipeline.jobs).toHaveLength(PAGE + 1);
    expect(pipeline.jobsComplete).toBe(true);

    // A refresh keeps everything loaded so far.
    await pipeline.refresh();
    expect(backend.pipelineJobs).toHaveBeenLastCalledWith({
      filter: "all",
      limit: PAGE + 1,
    });

    await pipeline.setFilter("failed");
    expect(backend.pipelineJobs).toHaveBeenLastCalledWith({
      filter: "failed",
      limit: PAGE,
    });
    await pipeline.setLogLevel("warn");
    expect(backend.pipelineLog).toHaveBeenLastCalledWith({
      min_level: "warn",
      limit: PAGE,
    });
  });

  it("retries failed photos", async () => {
    const backend = api();
    const pipeline = new Pipeline(backend);
    await pipeline.retry();
    expect(backend.retryFailedImports).toHaveBeenCalledWith(undefined);
  });
});
