import { describe, expect, it } from "vitest";
import { logLine } from "./log";

describe("logLine", () => {
  it("splits the file from what happened, and paths after each slash", () => {
    expect(
      logLine("IMG_0042.JPG: imported to 2026/2026-10-12/IMG_0042.JPG"),
    ).toEqual({
      subject: "IMG_0042.JPG",
      detail: ["imported to 2026/", "2026-10-12/", "IMG_0042.JPG"],
    });
  });

  it("keeps a message without a subject whole", () => {
    expect(logLine("Scan finished")).toEqual({
      subject: null,
      detail: ["Scan finished"],
    });
  });

  it("splits at the first colon only", () => {
    expect(logLine("a.jpg: failed: disk full")).toEqual({
      subject: "a.jpg",
      detail: ["failed: disk full"],
    });
  });
});
