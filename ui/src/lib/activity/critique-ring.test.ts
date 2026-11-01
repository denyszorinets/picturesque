import { describe, expect, it } from "vitest";
import {
  critiqueDetails,
  critiqueExtraLines,
  critiqueFailedLine,
  critiqueHeadline,
  critiqueRing,
} from "./critique-ring";
import { STATUS } from "./critique.test";

describe("critiqueRing", () => {
  it("is hidden when unknown, off, or idle with nothing to do", () => {
    expect(critiqueRing(null)).toBeNull();
    expect(critiqueRing({ ...STATUS, enabled: false })).toBeNull();
    expect(
      critiqueRing({
        ...STATUS,
        running: false,
        missing: 0,
        stale: 0,
        critiqued: 10,
      }),
    ).toBeNull();
  });

  it("counts current critiques out of all photos while running", () => {
    const ring = critiqueRing(STATUS);
    expect(ring?.label).toBe("Critique 3 / 10");
    expect(ring?.tone).toBe("busy");
    expect(ring?.progress).toBeCloseTo(0.3);
  });

  it("stays busy while waiting for its turn", () => {
    expect(critiqueRing({ ...STATUS, running: false })?.tone).toBe("busy");
  });

  it("says why it is paused", () => {
    const ring = critiqueRing({
      ...STATUS,
      running: false,
      paused: "You are using the app",
    });
    expect(ring?.tone).toBe("paused");
    expect(ring?.label).toBe("Critique paused · 3 / 10");
    expect(ring?.details[0]).toContain("You are using the app");
  });

  it("shows failures and timing in the details", () => {
    const ring = critiqueRing({ ...STATUS, failed: 2 });
    expect(ring?.failed).toBe(2);
    expect(ring?.details.join("\n")).toContain("20 s per photo");
    expect(ring?.details.join("\n")).toContain("about 2 min left");
    expect(ring?.details.join("\n")).toContain("1 to redo");
  });

  it("reports a finished run", () => {
    expect(
      critiqueDetails({ ...STATUS, missing: 0, stale: 0, critiqued: 10 })[0],
    ).toBe("Critique: all 10 photos critiqued");
  });

  it("splits the lines the Activity tab shows", () => {
    const s = { ...STATUS, failed: 2 };
    expect(critiqueHeadline(s)).toBe("Critiquing: 3 / 10 done");
    expect(critiqueExtraLines(s)).toEqual([
      "1 to redo (earlier model or prompt)",
      "20 s per photo",
      "about 2 min left",
    ]);
    expect(critiqueFailedLine(s)).toContain("2 failed");
    expect(critiqueFailedLine(STATUS)).toBeNull();
  });
});
