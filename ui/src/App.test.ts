import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/svelte";
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
        onboarding: { tour_done: false },
        open_folder: null,
        recent_folders: [],
        labels: { pick: "green", auto_pick: "yellow" },
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
  });

  it("points an empty library without a folder to setup", async () => {
    appInfo.mockResolvedValue({ name: "Picturesque", version: "0.1.0" });

    render(App);

    // Close the first-launch wizard: the library says what to do next.
    await screen.findByTestId("setup-wizard");
    await fireEvent.keyDown(window, { key: "Escape" });
    const empty = await screen.findByTestId("library-empty");
    expect(empty).toHaveTextContent("Choose an output folder first");
    await fireEvent.click(
      within(empty).getByRole("button", { name: "Set up Picturesque" }),
    );
    expect(await screen.findByTestId("setup-wizard")).toBeInTheDocument();

    // Activity says the same, and goes to the same place.
    await fireEvent.keyDown(window, { key: "Escape" });
    await fireEvent.click(screen.getByRole("button", { name: "Activity" }));
    const notice = await screen.findByTestId("needs-library");
    await fireEvent.click(
      within(notice).getByRole("button", { name: "Set up Picturesque" }),
    );
    expect(await screen.findByTestId("setup-wizard")).toBeInTheDocument();
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

    const ring = await screen.findByTestId("activity-ring");
    expect(ring).toHaveTextContent("Importing · 12 left");
    expect(ring).toHaveTextContent("2 failed");
    // The empty library says photos are on their way.
    expect(await screen.findByText("Importing…")).toBeInTheDocument();

    ring.click();
    const details = await screen.findByTestId("activity-popover");
    expect(details).toHaveTextContent("copying IMG_0003.CR3");
    screen.getByRole("button", { name: "Open Activity" }).click();
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

  it("opens Help with ? or the header button", async () => {
    appInfo.mockResolvedValue({ name: "Picturesque", version: "0.1.0" });

    render(App);

    // Close the first-launch wizard first.
    await screen.findByTestId("setup-wizard");
    await fireEvent.keyDown(window, { key: "Escape" });
    const button = await screen.findByTestId("open-help");
    await fireEvent.keyDown(window, { key: "?" });
    expect(await screen.findByTestId("help-dialog")).toBeInTheDocument();
    await fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByTestId("help-dialog")).toBeNull();

    await fireEvent.click(button);
    expect(await screen.findByTestId("help-dialog")).toBeInTheDocument();
  });

  it("shows an error when the backend is unavailable", async () => {
    appInfo.mockRejectedValue(new Error("no backend"));

    render(App);

    expect(await screen.findByRole("alert")).toHaveTextContent("no backend");
  });
});
