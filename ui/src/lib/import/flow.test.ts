import { describe, expect, it, vi } from "vitest";
import type { ImportPlanView, ImportStatus } from "../api";
import { ApiError } from "../api";
import { ImportFlow, type ImportApi } from "./flow.svelte";

function status(
  state: ImportStatus["state"],
  extra: Partial<ImportStatus> = {},
): ImportStatus {
  return {
    run_id: 1,
    state,
    phase: null,
    done: 0,
    total: 0,
    summary: null,
    error: null,
    ...extra,
  };
}

function plan(state: ImportPlanView["state"]): ImportPlanView {
  return {
    run_id: 1,
    state,
    library_root: "/lib",
    template: "{name}.{ext}",
    summary: {
      photos: 1,
      to_import: 1,
      imported: 0,
      duplicates: 0,
      failed: 0,
      files: 1,
      bytes: 10,
    },
    photos: [],
    problems: [],
    error: null,
  };
}

function fakeApi(overrides: Partial<ImportApi> = {}): ImportApi {
  return {
    planImport: vi.fn().mockResolvedValue(status("planning")),
    getImport: vi.fn().mockResolvedValue(plan("planned")),
    importStatus: vi.fn().mockResolvedValue(status("planning")),
    startImport: vi.fn().mockResolvedValue(status("copying")),
    cancelImport: vi.fn().mockResolvedValue(status("copying")),
    ...overrides,
  };
}

describe("ImportFlow", () => {
  it("adds sources once, trimmed, and removes them", () => {
    const flow = new ImportFlow(fakeApi());

    flow.addSource(" /a ");
    flow.addSource("/a");
    flow.addSource("");
    flow.addSource("/b");
    flow.removeSource("/a");

    expect(flow.sources).toEqual(["/b"]);
  });

  it("goes from planning to review when the plan is ready", async () => {
    const api = fakeApi();
    const flow = new ImportFlow(api);
    flow.addSource("/card");

    await flow.startPlanning();
    expect(flow.step).toBe("planning");
    expect(api.planImport).toHaveBeenCalledWith(["/card"]);

    await flow.onStatus(
      status("planning", { phase: "reading", done: 3, total: 4 }),
    );
    expect(flow.fraction).toBe(0.75);

    await flow.onStatus(status("planned"));
    expect(flow.step).toBe("review");
    expect(flow.plan?.state).toBe("planned");
  });

  it("goes from copying to done and keeps the outcome", async () => {
    const api = fakeApi({
      getImport: vi.fn().mockResolvedValue(plan("completed")),
    });
    const flow = new ImportFlow(api);
    await flow.startPlanning();
    await flow.onStatus(status("planned"));

    await flow.startCopying();
    expect(flow.step).toBe("copying");
    await flow.onStatus(status("completed"));

    expect(flow.step).toBe("done");
    expect(flow.plan?.state).toBe("completed");
  });

  it("ignores events for other runs", async () => {
    const flow = new ImportFlow(fakeApi());
    await flow.startPlanning();

    await flow.onStatus({ ...status("planned"), run_id: 99 });

    expect(flow.step).toBe("planning");
  });

  it("falls back to polling while a phase runs", async () => {
    const api = fakeApi({
      importStatus: vi.fn().mockResolvedValue(status("planned")),
    });
    const flow = new ImportFlow(api);
    await flow.startPlanning();

    await flow.refresh();

    expect(flow.step).toBe("review");
  });

  it("returns to choosing sources when planning fails", async () => {
    const flow = new ImportFlow(fakeApi());
    await flow.startPlanning();

    await flow.onStatus(status("failed", { error: "catalog broken" }));

    expect(flow.step).toBe("choose");
    expect(flow.error).toBe("catalog broken");
  });

  it("shows API errors without changing step", async () => {
    const api = fakeApi({
      planImport: vi
        .fn()
        .mockRejectedValue(
          new ApiError(
            "invalid_arguments",
            "Choose a library folder before importing.",
          ),
        ),
    });
    const flow = new ImportFlow(api);

    await flow.startPlanning();

    expect(flow.step).toBe("choose");
    expect(flow.error).toBe("Choose a library folder before importing.");
    expect(flow.busy).toBe(false);
  });
});
