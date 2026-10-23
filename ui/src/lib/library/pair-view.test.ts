import { describe, expect, it } from "vitest";
import type { PhotoSummary } from "../api";
import { imageFormat, isPair, photoUrl, viewLabel } from "./pair-view.svelte";

function photo(over: Partial<PhotoSummary>): PhotoSummary {
  return {
    id: 7,
    has_raw: false,
    has_image: true,
    file_name: "a.JPG",
    ...over,
  } as PhotoSummary;
}

describe("pair view", () => {
  const pair = photo({ has_raw: true, has_image: true });
  const rawOnly = photo({
    has_raw: true,
    has_image: false,
    file_name: "a.CR3",
  });
  const jpegOnly = photo({});

  it("knows a RAW+JPEG pair", () => {
    expect(isPair(pair)).toBe(true);
    expect(isPair(rawOnly)).toBe(false);
    expect(isPair(jpegOnly)).toBe(false);
  });

  it("names the view only for pairs", () => {
    expect(photoUrl(pair, "preview", "raw")).toBe("/api/images/preview/7/raw");
    expect(photoUrl(pair, "grid")).toBe("/api/images/grid/7/image");
    expect(photoUrl(rawOnly, "grid", "image")).toBe("/api/images/grid/7");
    expect(photoUrl(jpegOnly, "full", "raw")).toBe("/api/images/full/7");
  });

  it("labels the shown file", () => {
    expect(viewLabel(pair, "raw")).toBe("RAW");
    expect(viewLabel(pair, "image")).toBe("JPEG");
    expect(viewLabel(rawOnly, "image")).toBe("RAW");
    expect(viewLabel(photo({ file_name: "b.heic" }), "raw")).toBe("HEIC");
    expect(imageFormat("c.jpeg")).toBe("JPEG");
  });
});
