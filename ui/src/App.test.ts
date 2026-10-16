import { cleanup, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App.svelte";

const appInfo = vi.fn();
const pipelineStatus = vi.fn();
vi.mock("./lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./lib/api")>();
  return {
    ...actual,
    appInfo: () => appInfo(),
    getSettings: () =>
      Promise.resolve({
        library_root: null,
        path_template: "{name}.{ext}",
        sidecar_naming: "file_extension",
        series: { max_gap_seconds: 30, max_distance: 16 },
        ranking: { sharpness: 0.6, exposure: 0.3, noise: 0.1, aesthetic: 0.4 },
        ai: { provider: "auto" },
        import: { rescan_minutes: 5 },
        auto_pick: { enabled: true, min_quality: 0.5 },
      }),
    listModels: () => Promise.resolve([]),
    aiStatus: () =>
      Promise.resolve({
        available: ["cpu"],
        choice: "auto",
        provider: "cpu",
        problem: null,
      }),
    templatePresets: () => Promise.resolve([]),
    previewTemplate: () => Promise.resolve({ error: null, examples: [] }),
    onAppEvent: () => () => {},
    photoFacets: () =>
      Promise.resolve({
        total: 0,
        first_day: null,
        last_day: null,
        days: [],
        cameras: [],
        lenses: [],
        places: [],
        flags: [],
        auto_picked: 0,
        ratings: [0, 0, 0, 0, 0, 0],
      }),
    queryPhotos: () =>
      Promise.resolve({ photos: [], next_cursor: null, total: 0 }),
    canPickFolders: () => false,
    pipelineStatus: () => pipelineStatus(),
    listImportSources: () => Promise.resolve([]),
    pipelineJobs: () => Promise.resolve([]),
    pipelineLog: () => Promise.resolve([]),
  };
});

const idle = {
  queued: 0,
  imported: 0,
  duplicates: 0,
  failed: 0,
  active: null,
  scanning: false,
  paused: null,
};

describe("App", () => {
  // Hooks use block bodies: a function returned from beforeEach runs as teardown.
  beforeEach(() => {
    appInfo.mockReset();
    pipelineStatus.mockReset();
    pipelineStatus.mockResolvedValue(idle);
  });
  afterEach(() => {
    cleanup();
  });

  it("shows the backend name and version", async () => {
    appInfo.mockResolvedValue({ name: "Picturesque", version: "0.1.0" });

    render(App);

    expect(await screen.findByTestId("app-version")).toHaveTextContent(
      "v0.1.0",
    );
    expect(
      await screen.findByRole("button", { name: "Library" }),
    ).toBeInTheDocument();
    expect(await screen.findByText("No photos yet")).toBeInTheDocument();
  });

  it("shows import progress in the header and opens the activity", async () => {
    appInfo.mockResolvedValue({ name: "Picturesque", version: "0.1.0" });
    pipelineStatus.mockResolvedValue({
      ...idle,
      queued: 12,
      failed: 2,
      active: { job_id: 3, file_name: "IMG_0003.CR3", stage: "copying" },
    });

    render(App);

    const indicator = await screen.findByTestId("pipeline-indicator");
    expect(indicator).toHaveTextContent("Importing 12 photos");
    expect(indicator).toHaveTextContent("2 failed");
    expect(indicator).toHaveAttribute("title", "Copying IMG_0003.CR3");
    // The empty library says photos are on their way.
    expect(await screen.findByText("Importing…")).toBeInTheDocument();

    indicator.click();
    expect(
      await screen.findByRole("heading", { name: "Import folders" }),
    ).toBeVisible();
    expect(screen.getByRole("button", { name: "Activity" })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  it("opens the setup wizard on first launch", async () => {
    appInfo.mockResolvedValue({ name: "Picturesque", version: "0.1.0" });

    render(App);

    // No library folder is set yet.
    const wizard = await screen.findByTestId("setup-wizard");
    expect(wizard).toHaveTextContent("Welcome to Picturesque");
    screen.getByRole("button", { name: "Close" }).click();
    await waitFor(() =>
      expect(screen.queryByTestId("setup-wizard")).toBeNull(),
    );

    // Settings opens it again.
    screen.getByRole("button", { name: "Settings" }).click();
    (await screen.findByTestId("open-setup")).click();
    expect(await screen.findByTestId("setup-wizard")).toBeInTheDocument();
  });

  it("shows an error when the backend is unavailable", async () => {
    appInfo.mockRejectedValue(new Error("no backend"));

    render(App);

    expect(await screen.findByRole("alert")).toHaveTextContent("no backend");
  });
});
