import { cleanup, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it } from "vitest";
import type { PhotoSummary } from "../api";
import Tile from "./Tile.svelte";
import tileSource from "./Tile.svelte?raw";

afterEach(cleanup);

function photo(over: Partial<PhotoSummary> = {}): PhotoSummary {
  return {
    id: 1,
    captured_at: null,
    day: null,
    camera: null,
    lens: null,
    city: null,
    country: null,
    flag: "unflagged",
    rating: 0,
    width: null,
    height: null,
    has_raw: false,
    has_image: true,
    file_name: "a.jpg",
    series_id: null,
    best_in_series: false,
    series_size: 1,
    quality: null,
    aesthetic: null,
    eyes_closed: null,
    faces: 0,
    keeper: false,
    ...over,
  };
}

function ring(p: PhotoSummary): string | null {
  render(Tile, {
    photo: p,
    size: 200,
    selected: false,
    focused: false,
    onclick: () => {},
    ondblclick: () => {},
  });
  return screen.queryByTestId("tile-ring")?.getAttribute("data-ring") ?? null;
}

describe("Tile ring", () => {
  it("rings picks green", () => {
    expect(ring(photo({ flag: "pick" }))).toBe("pick");
  });

  it("rings auto-picked photos light green", () => {
    expect(ring(photo({ keeper: true }))).toBe("auto");
  });

  it("does not ring a decided photo as auto-picked", () => {
    expect(ring(photo({ keeper: true, flag: "reject" }))).toBeNull();
    expect(ring(photo({ keeper: true, flag: "pick" }))).toBe("pick");
  });

  it("leaves other photos without a ring", () => {
    expect(ring(photo())).toBeNull();
  });
});

describe("Tile styles", () => {
  // WebKitGTK repaints every filtered tile on any change; with thousands of
  // photos the app froze and ran out of memory (#192).
  it("use no CSS filters", () => {
    const style = tileSource.slice(tileSource.indexOf("<style>"));
    expect(style).not.toMatch(/(^|[^-])filter\s*:/m);
    expect(style).not.toMatch(/backdrop-filter\s*:/);
  });
});
