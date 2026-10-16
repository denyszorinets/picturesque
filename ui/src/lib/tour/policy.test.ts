import { describe, expect, it } from "vitest";
import { tourWanted } from "./policy";

const base = {
  tourDone: false,
  replay: false,
  hasLibrary: true,
  setupOpen: false,
  helpOpen: false,
  onLibraryTab: true,
};

describe("tourWanted", () => {
  it("shows the tour once a library is set up, until it was seen", () => {
    expect(tourWanted(base)).toBe(true);
    expect(tourWanted({ ...base, tourDone: true })).toBe(false);
    expect(tourWanted({ ...base, hasLibrary: false })).toBe(false);
  });

  it("waits while another dialog or tab is shown", () => {
    expect(tourWanted({ ...base, setupOpen: true })).toBe(false);
    expect(tourWanted({ ...base, helpOpen: true })).toBe(false);
    expect(tourWanted({ ...base, onLibraryTab: false })).toBe(false);
  });

  it("replays on request even when seen", () => {
    expect(tourWanted({ ...base, tourDone: true, replay: true })).toBe(true);
    expect(
      tourWanted({ ...base, tourDone: true, replay: true, setupOpen: true }),
    ).toBe(false);
  });

  it("waits for the settings to load", () => {
    expect(tourWanted({ ...base, tourDone: null })).toBe(false);
  });
});
