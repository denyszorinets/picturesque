import { describe, expect, it } from "vitest";
import {
  computeLayout,
  neighbour,
  resizedScrollTop,
  rowTopOf,
  seriesSegments,
  visibleRows,
} from "./layout";

const sections = [
  { day: "2026-11-18", start: 0, count: 1317 },
  { day: "2026-11-17", start: 1317, count: 3701 },
  { day: null, start: 5018, count: 171 },
];
const layout = (minTile: number, width = 1000) =>
  computeLayout(sections, { width, minTile, gap: 10, headerHeight: 44 });

describe("resizing a 5189-photo timeline", () => {
  it.each([120, 180, 240, 300, 360])(
    "keeps every photo and date boundary at size %i",
    (size) => {
      const l = layout(size);
      const tiles = l.rows.filter((r) => r.kind === "tiles");
      expect(
        tiles.flatMap((r) =>
          Array.from({ length: r.end - r.start }, (_, i) => r.start + i),
        ),
      ).toEqual(Array.from({ length: 5189 }, (_, i) => i));
      expect(l.rows.filter((r) => r.kind === "header")).toHaveLength(3);
      for (const row of tiles) {
        expect(row.end).toBeLessThanOrEqual(
          row.section.start + row.section.count,
        );
        expect((row.end - row.start) * (l.tile + 10) - 10).toBeLessThanOrEqual(
          1000.001,
        );
      }
      const top = rowTopOf(l.rows, 4000)!;
      const visible = visibleRows(l.rows, top, 700, 600);
      expect(visible.length).toBeLessThan(22);
      expect(neighbour(l.rows, 4000, "down", 5189)).toBe(4000 + l.columns);
      const boundary = tiles.findIndex((r) => r.end === 1317);
      expect(neighbour(l.rows, 1316, "down", 5189)).toBe(
        Math.min(
          tiles[boundary + 1]!.end - 1,
          1317 + 1316 - tiles[boundary]!.start,
        ),
      );
      const seriesOf = (i: number) => (i >= 3998 && i < 4011 ? 42 : null);
      for (const row of visible) {
        if (row.kind !== "tiles") continue;
        for (const segment of seriesSegments(
          row.start,
          row.end,
          seriesOf,
          l.tile,
          10,
        )) {
          expect(segment.left).toBeGreaterThanOrEqual(0);
          expect(segment.left + segment.width).toBeLessThanOrEqual(1000.001);
          expect(segment.openStart).toBe(row.start > 3998);
          expect(segment.openEnd).toBe(row.end < 4011);
        }
      }
    },
  );
  it("anchors the visible focus or first visible row, not an old offscreen selection", () => {
    const before = layout(180),
      after = layout(360);
    const top = rowTopOf(before.rows, 4000)!;
    expect(resizedScrollTop(before, after, top + 25, 700, 4002)).toBe(
      rowTopOf(after.rows, 4002)! + top + 25 - rowTopOf(before.rows, 4002)!,
    );
    expect(resizedScrollTop(before, after, top + 25, 700, 0)).toBe(
      rowTopOf(after.rows, 3997)! + (25 / before.tile) * after.tile,
    );
  });
  it("keeps a mostly clipped tile visible when shrinking", () => {
    const before = layout(360),
      after = layout(120);
    const row = before.rows.find(
      (r) => r.kind === "tiles" && r.start <= 4000 && r.end > 4000,
    )!;
    if (row.kind !== "tiles") throw new Error("expected photo row");
    const next = resizedScrollTop(
      before,
      after,
      row.top + before.tile * 0.9,
      700,
      null,
    );
    const top = rowTopOf(after.rows, row.start)!;
    expect(next).toBeGreaterThan(top);
    expect(next).toBeLessThan(top + after.tile);
  });

  it("clamps the bottom and supports a narrow one-column grid", () => {
    const big = layout(360),
      small = layout(120);
    const scroll = resizedScrollTop(big, small, big.height - 600, 600, 5188);
    expect(scroll).toBeGreaterThanOrEqual(0);
    expect(scroll).toBeLessThanOrEqual(small.height - 600);
    expect(layout(360, 210).columns).toBe(1);
    expect(layout(360, 210).tile).toBe(210);
  });
});
