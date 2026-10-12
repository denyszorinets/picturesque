/**
 * Timeline layout: day sections laid out as a header row followed by rows
 * of square tiles. Everything is computed from photo counts per day, so the
 * whole timeline has its exact height before any photo is loaded, and only
 * rows near the viewport need to be rendered.
 */

/** A day (or the undated photos) and where its photos start in the list. */
export interface Section {
  /** `YYYY-MM-DD`, or `null` for photos without a capture date. */
  day: string | null;
  /** Index of the section's first photo in the newest-first photo list. */
  start: number;
  count: number;
}

export type Row =
  | { kind: "header"; section: Section; top: number; height: number }
  | {
      kind: "tiles";
      section: Section;
      top: number;
      height: number;
      start: number;
      end: number;
    };

export interface Metrics {
  /** Usable width of the grid. */
  width: number;
  /** Smallest tile edge; tiles grow to fill the width. */
  minTile: number;
  gap: number;
  headerHeight: number;
}

export interface Layout {
  rows: Row[];
  height: number;
  columns: number;
  tile: number;
}

/**
 * Sections in newest-first order: days as given (newest first), then the
 * undated photos, matching the order `query_photos` returns.
 */
export function buildSections(
  days: { value: string; count: number }[],
  total: number,
): Section[] {
  const sections: Section[] = [];
  let start = 0;
  for (const d of days) {
    if (d.count > 0) sections.push({ day: d.value, start, count: d.count });
    start += d.count;
  }
  if (total > start) sections.push({ day: null, start, count: total - start });
  return sections;
}

export function computeLayout(sections: Section[], m: Metrics): Layout {
  const columns = Math.max(
    1,
    Math.floor((m.width + m.gap) / (m.minTile + m.gap)),
  );
  const tile = Math.max(1, (m.width - m.gap * (columns - 1)) / columns);
  const rows: Row[] = [];
  let top = 0;
  for (const section of sections) {
    rows.push({ kind: "header", section, top, height: m.headerHeight });
    top += m.headerHeight;
    for (let i = 0; i < section.count; i += columns) {
      const start = section.start + i;
      const end = Math.min(section.start + section.count, start + columns);
      rows.push({
        kind: "tiles",
        section,
        top,
        height: tile + m.gap,
        start,
        end,
      });
      top += tile + m.gap;
    }
  }
  return { rows, height: top, columns, tile };
}

/** Rows overlapping [scrollTop - overscan, scrollTop + viewport + overscan]. */
export function visibleRows(
  rows: Row[],
  scrollTop: number,
  viewport: number,
  overscan: number,
): Row[] {
  const from = scrollTop - overscan;
  const to = scrollTop + viewport + overscan;
  // First row whose bottom is below `from`.
  let lo = 0;
  let hi = rows.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    const r = rows[mid]!;
    if (r.top + r.height <= from) lo = mid + 1;
    else hi = mid;
  }
  const out: Row[] = [];
  for (let i = lo; i < rows.length && rows[i]!.top < to; i++)
    out.push(rows[i]!);
  return out;
}

/** Top of the row containing photo `index`, for scrolling it into view. */
export function rowTopOf(rows: Row[], index: number): number | null {
  for (const r of rows) {
    if (r.kind === "tiles" && index >= r.start && index < r.end) return r.top;
  }
  return null;
}

export type Direction = "left" | "right" | "up" | "down";

/**
 * The photo next to `index` in the laid-out timeline. Up and down go to the
 * same column of the previous or next row of tiles (across day headers),
 * or the last tile of a shorter row. Returns `index` at the edges.
 */
export function neighbour(
  rows: Row[],
  index: number,
  direction: Direction,
  total: number,
): number {
  if (direction === "left") return Math.max(0, index - 1);
  if (direction === "right") return Math.min(total - 1, index + 1);
  const tileRows = rows.filter(
    (r): r is Extract<Row, { kind: "tiles" }> => r.kind === "tiles",
  );
  const at = tileRows.findIndex((r) => index >= r.start && index < r.end);
  if (at === -1) return index;
  const target = tileRows[direction === "up" ? at - 1 : at + 1];
  if (!target) return index;
  const column = index - tileRows[at]!.start;
  return Math.min(target.start + column, target.end - 1);
}
