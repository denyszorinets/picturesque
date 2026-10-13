import { describe, expect, it } from "vitest";
import {
  buildSections,
  computeLayout,
  rowTopOf,
  seriesSegments,
  visibleRows,
} from "./layout";

const metrics = { width: 1000, minTile: 180, gap: 8, headerHeight: 40 };

describe("buildSections", () => {
  it("lays days out newest first and puts undated photos last", () => {
    const sections = buildSections(
      [
        { value: "2026-10-12", count: 3 },
        { value: "2026-10-11", count: 2 },
      ],
      7,
    );

    expect(sections).toEqual([
      { day: "2026-10-12", start: 0, count: 3 },
      { day: "2026-10-11", start: 3, count: 2 },
      { day: null, start: 5, count: 2 },
    ]);
  });

  it("has no undated section when every photo is dated", () => {
    expect(buildSections([{ value: "2026-01-01", count: 4 }], 4)).toHaveLength(
      1,
    );
  });
});

describe("computeLayout", () => {
  it("fills the width with as many tiles as fit and wraps each day", () => {
    const layout = computeLayout(
      buildSections(
        [
          { value: "d1", count: 7 },
          { value: "d2", count: 1 },
        ],
        8,
      ),
      metrics,
    );

    // (1000 + 8) / (180 + 8) → 5 columns; tiles grow to 193.6 px.
    expect(layout.columns).toBe(5);
    expect(layout.tile).toBeCloseTo(193.6);
    expect(
      layout.rows.map((r) =>
        r.kind === "header" ? "H" : `${r.start}-${r.end}`,
      ),
    ).toEqual(["H", "0-5", "5-7", "H", "7-8"]);
    expect(layout.height).toBeCloseTo(40 * 2 + 3 * (193.6 + 8));
  });

  it("always has at least one column", () => {
    expect(
      computeLayout(buildSections([{ value: "d", count: 2 }], 2), {
        ...metrics,
        width: 50,
      }).columns,
    ).toBe(1);
  });

  it("lays out ten thousand photos quickly", () => {
    const days = Array.from({ length: 1000 }, (_, i) => ({
      value: `d${i}`,
      count: 10,
    }));
    const start = performance.now();
    const layout = computeLayout(buildSections(days, 10_000), metrics);
    const visible = visibleRows(layout.rows, layout.height / 2, 900, 400);

    expect(performance.now() - start).toBeLessThan(50);
    expect(visible.length).toBeGreaterThan(0);
    expect(visible.length).toBeLessThan(20);
  });
});

describe("visibleRows", () => {
  const layout = computeLayout(
    buildSections([{ value: "d", count: 50 }], 50),
    metrics,
  );

  it("returns only rows overlapping the viewport and overscan", () => {
    const rows = visibleRows(layout.rows, 1000, 500, 0);

    expect(rows.every((r) => r.top < 1500 && r.top + r.height > 1000)).toBe(
      true,
    );
    expect(rows.length).toBeGreaterThanOrEqual(2);
  });

  it("returns the first rows at the top", () => {
    expect(visibleRows(layout.rows, 0, 300, 0)[0]!.kind).toBe("header");
  });
});

describe("rowTopOf", () => {
  it("finds the row of a photo", () => {
    const layout = computeLayout(
      buildSections([{ value: "d", count: 12 }], 12),
      metrics,
    );

    expect(rowTopOf(layout.rows, 0)).toBe(40);
    expect(rowTopOf(layout.rows, 6)).toBeCloseTo(40 + 193.6 + 8);
    expect(rowTopOf(layout.rows, 99)).toBeNull();
  });
});

describe("neighbour", async () => {
  const { neighbour } = await import("./layout");
  // 5 columns: day one has 7 photos (rows 0-4, 5-6), day two has 8 (7-11, 12-14).
  const layout = computeLayout(
    buildSections(
      [
        { value: "d1", count: 7 },
        { value: "d2", count: 8 },
      ],
      15,
    ),
    metrics,
  );

  it("moves left and right through the list", () => {
    expect(neighbour(layout.rows, 4, "right", 15)).toBe(5);
    expect(neighbour(layout.rows, 0, "left", 15)).toBe(0);
    expect(neighbour(layout.rows, 14, "right", 15)).toBe(14);
  });

  it("moves up and down by row, keeping the column", () => {
    expect(neighbour(layout.rows, 1, "down", 15)).toBe(6);
    expect(neighbour(layout.rows, 6, "up", 15)).toBe(1);
  });

  it("crosses day headers and clamps to shorter rows", () => {
    // From the short second row of day one (5-6) down to day two's first row.
    expect(neighbour(layout.rows, 6, "down", 15)).toBe(8);
    // From column 4 of day two's first row up to the short row: its last tile.
    expect(neighbour(layout.rows, 11, "up", 15)).toBe(6);
  });

  it("stays put at the top and bottom", () => {
    expect(neighbour(layout.rows, 2, "up", 15)).toBe(2);
    expect(neighbour(layout.rows, 13, "down", 15)).toBe(13);
  });
});

describe("seriesSegments", () => {
  const series = [null, 7, 7, 7, null, 9, 9, 9, 9];
  const of = (i: number) => series[i] ?? null;

  it("frames runs of the same series", () => {
    expect(seriesSegments(0, 5, of, 100, 10)).toEqual([
      { seriesId: 7, left: 110, width: 320, openStart: false, openEnd: false },
    ]);
  });

  it("leaves edges open where a series continues on another row", () => {
    expect(seriesSegments(4, 7, of, 100, 10)).toEqual([
      { seriesId: 9, left: 110, width: 210, openStart: false, openEnd: true },
    ]);
    expect(seriesSegments(7, 9, of, 100, 10)).toEqual([
      { seriesId: 9, left: 0, width: 210, openStart: true, openEnd: false },
    ]);
  });

  it("keeps neighbouring series apart", () => {
    const touching = [1, 1, 2, 2];
    expect(
      seriesSegments(0, 4, (i) => touching[i] ?? null, 100, 10).map((s) => [
        s.seriesId,
        s.left,
        s.width,
      ]),
    ).toEqual([
      [1, 0, 210],
      [2, 220, 210],
    ]);
  });
});
