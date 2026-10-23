import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ImportSource, Settings } from "../api";
import SetupWizard from "./SetupWizard.svelte";

const SETTINGS: Settings = {
  library_root: null,
  path_template: "{year}/{date}/{name}.{ext}",
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
  delete_after_copy: "verified",
  pair_view: "image",
};

const getSettings = vi.fn<() => Promise<Settings>>();
const updateSettings = vi.fn<(u: Partial<Settings>) => Promise<Settings>>();
const listImportSources = vi.fn<() => Promise<ImportSource[]>>();
const addImportSource = vi.fn<(path: string) => Promise<ImportSource>>();

vi.mock("../api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api")>();
  return {
    ...actual,
    getSettings: () => getSettings(),
    updateSettings: (u: Partial<Settings>) => updateSettings(u),
    templatePresets: () =>
      Promise.resolve([
        {
          id: "date",
          label: "Year / date",
          template: "{year}/{date}/{name}.{ext}",
        },
        { id: "flat", label: "One folder", template: "{name}.{ext}" },
      ]),
    listImportSources: () => listImportSources(),
    addImportSource: (path: string) => addImportSource(path),
    listModels: () =>
      Promise.resolve([
        {
          id: "clip-image",
          name: "CLIP image encoder",
          purpose: "Aesthetic scores and similar photos",
          version: 1,
          size: 350_000_000,
          state: "not_installed",
          downloaded: 0,
          error: null,
          license: "MIT",
          license_url: "https://example.test/license",
          homepage: "https://example.test",
        },
      ]),
    onAppEvent: () => () => {},
    canPickFolders: () => false,
  };
});

function source(id: number, path: string): ImportSource {
  return { id, path, watch: true, scanned_at: null, scan_error: null };
}

beforeEach(() => {
  getSettings.mockResolvedValue(SETTINGS);
  updateSettings.mockImplementation(async (u) => ({ ...SETTINGS, ...u }));
  const sources: ImportSource[] = [];
  listImportSources.mockImplementation(async () => [...sources]);
  addImportSource.mockImplementation(async (path) => {
    const added = source(sources.length + 1, path);
    sources.push(added);
    return added;
  });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

const next = () => fireEvent.click(screen.getByTestId("setup-next"));

describe("SetupWizard", () => {
  it("sets the library folder, import folders and offers the models", async () => {
    const onclose = vi.fn();
    render(SetupWizard, { onclose });

    // Step 1: the library.
    expect(await screen.findByText("Step 1 of 3")).toBeInTheDocument();
    const root = await screen.findByTestId("setup-library-root");
    // Photos are copied; originals are only read.
    expect(screen.getByTestId("setup-copy-only")).toHaveTextContent(
      "Your originals are never moved, renamed, edited or deleted",
    );
    await fireEvent.input(root, { target: { value: " /photos/library " } });
    await fireEvent.click(await screen.findByLabelText(/One folder/));
    await next();
    expect(updateSettings).toHaveBeenCalledWith({
      library_root: "/photos/library",
      path_template: "{name}.{ext}",
    });

    // Step 2: import folders.
    expect(await screen.findByText("Step 2 of 3")).toBeInTheDocument();
    const folder = screen.getByTestId("setup-source");
    await fireEvent.input(folder, { target: { value: "/media/card" } });
    await fireEvent.click(screen.getByTestId("setup-add-source"));
    expect(addImportSource).toHaveBeenCalledWith("/media/card");
    expect(await screen.findByText("/media/card")).toBeInTheDocument();
    await next();

    // Step 3: AI models, optional.
    expect(await screen.findByText("Step 3 of 3")).toBeInTheDocument();
    expect(await screen.findByText("CLIP image encoder")).toBeInTheDocument();
    await fireEvent.click(screen.getByTestId("setup-next"));
    expect(onclose).toHaveBeenCalledOnce();
  });

  it("keeps a step open and shows why when saving fails", async () => {
    updateSettings.mockRejectedValue(new Error("folder is not writable"));
    render(SetupWizard, { onclose: vi.fn() });

    const root = await screen.findByTestId("setup-library-root");
    await fireEvent.input(root, { target: { value: "/read-only" } });
    await next();

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "folder is not writable",
    );
    expect(screen.getByText("Step 1 of 3")).toBeInTheDocument();
  });

  it("needs a library folder to go on, but a step can be skipped", async () => {
    render(SetupWizard, { onclose: vi.fn() });

    await screen.findByTestId("setup-library-root");
    expect(screen.getByTestId("setup-next")).toBeDisabled();
    await fireEvent.click(screen.getByTestId("setup-skip"));

    expect(await screen.findByText("Step 2 of 3")).toBeInTheDocument();
    expect(updateSettings).not.toHaveBeenCalled();
    // Importing waits for a library folder: say so.
    expect(screen.getByTestId("setup-needs-library")).toBeInTheDocument();
  });

  it("starts from the settings already saved", async () => {
    getSettings.mockResolvedValue({ ...SETTINGS, library_root: "/lib" });
    render(SetupWizard, { onclose: vi.fn() });

    expect(await screen.findByDisplayValue("/lib")).toBeInTheDocument();
    await next();
    // Nothing changed: nothing to save.
    expect(updateSettings).not.toHaveBeenCalled();
    expect(await screen.findByText("Step 2 of 3")).toBeInTheDocument();
  });

  it("closes with Close or Escape", async () => {
    const onclose = vi.fn();
    render(SetupWizard, { onclose });
    await screen.findByText("Step 1 of 3");

    await fireEvent.keyDown(window, { key: "Escape" });
    expect(onclose).toHaveBeenCalledOnce();
    await fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(onclose).toHaveBeenCalledTimes(2);
  });
});
