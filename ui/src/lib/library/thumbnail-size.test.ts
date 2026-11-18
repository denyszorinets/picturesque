import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  loadThumbnailSize,
  saveThumbnailSize,
  thumbnailSize,
} from "./thumbnail-size";

beforeEach(() => localStorage.clear());
describe("thumbnail size preference", () => {
  it("starts at the existing size and remembers changes across new views", () => {
    expect(loadThumbnailSize()).toBe(180);
    saveThumbnailSize(300);
    expect(loadThumbnailSize()).toBe(300);
  });
  it("normalizes malformed and out-of-range values", () => {
    localStorage.setItem("picturesque.library.thumbnailSize", "broken");
    expect(loadThumbnailSize()).toBe(180);
    expect(thumbnailSize(-1)).toBe(120);
    expect(thumbnailSize(1000)).toBe(360);
    expect(thumbnailSize(193)).toBe(190);
  });
  it("still works when storage is blocked or full", () => {
    const get = vi
      .spyOn(Storage.prototype, "getItem")
      .mockImplementation(() => {
        throw Error("blocked");
      });
    const set = vi
      .spyOn(Storage.prototype, "setItem")
      .mockImplementation(() => {
        throw Error("full");
      });
    expect(loadThumbnailSize()).toBe(180);
    expect(() => saveThumbnailSize(240)).not.toThrow();
    get.mockRestore();
    set.mockRestore();
  });
});
