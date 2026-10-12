import { cleanup, render, screen } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App.svelte";

const appInfo = vi.fn();
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
        ranking: { sharpness: 0.6, exposure: 0.3, noise: 0.1 },
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
        ratings: [0, 0, 0, 0, 0, 0],
      }),
    queryPhotos: () =>
      Promise.resolve({ photos: [], next_cursor: null, total: 0 }),
    canPickFolders: () => false,
  };
});

describe("App", () => {
  // Hooks use block bodies: a function returned from beforeEach runs as teardown.
  beforeEach(() => {
    appInfo.mockReset();
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

  it("shows an error when the backend is unavailable", async () => {
    appInfo.mockRejectedValue(new Error("no backend"));

    render(App);

    expect(await screen.findByRole("alert")).toHaveTextContent("no backend");
  });
});
