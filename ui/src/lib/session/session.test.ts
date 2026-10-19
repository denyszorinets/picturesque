import { describe, expect, it } from "vitest";
import { folderName, sessionLabel } from "./session";

describe("folderName", () => {
  it("is the last part of a path on any platform", () => {
    expect(folderName("/home/me/Pictures/2026 Trip")).toBe("2026 Trip");
    expect(folderName("/home/me/Pictures/2026 Trip/")).toBe("2026 Trip");
    expect(folderName("E:\\DCIM\\100CANON")).toBe("100CANON");
    expect(folderName("/")).toBe("/");
  });
});

describe("sessionLabel", () => {
  it("names the open folder, or the library", () => {
    expect(sessionLabel({ open_folder: "/cards/DCIM" })).toBe("DCIM");
    expect(sessionLabel({ open_folder: null })).toBe("Library");
  });
});
