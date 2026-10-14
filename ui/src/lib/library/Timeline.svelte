<script lang="ts">
  import {
    computeLayout,
    rowTopOf,
    seriesSegments,
    visibleRows,
    type Layout,
    type Section,
  } from "./layout";
  import type { Selection } from "./selection.svelte";
  import type { LibraryStore } from "./store.svelte";
  import type { PhotoSummary } from "../api";
  import Tile from "./Tile.svelte";

  interface Props {
    store: LibraryStore;
    selection: Selection;
    /** Double-click or Enter on a photo. */
    onopen?: (index: number) => void;
    /** Called with the current layout, for keyboard navigation. */
    onlayout?: (layout: Layout) => void;
    /** Called with the ids of the loaded photos on screen. */
    onvisible?: (photoIds: number[]) => void;
    /** Whether series are stacked to their best photo. */
    stacked?: boolean;
    /** Stacked series the user expanded. */
    expanded?: ReadonlySet<number>;
    /** The stack button of a series was pressed. */
    onstack?: (seriesId: number) => void;
    /** Show quality scores on tiles. */
    showQuality?: boolean;
  }

  let {
    store,
    selection,
    onopen,
    onlayout,
    onvisible,
    stacked = false,
    expanded = new Set(),
    onstack,
    showQuality = false,
  }: Props = $props();

  // Leaves room for series frames between tiles.
  const GAP = 10;
  /** How far a series frame reaches around its tiles. */
  const FRAME = 4;
  const HEADER = 44;
  const OVERSCAN = 600;

  let viewport: HTMLDivElement | undefined = $state();
  let width = $state(0);
  let height = $state(0);
  let scrollTop = $state(0);

  const computed = $derived(
    computeLayout(store.sections, {
      width: Math.max(0, width),
      minTile: 180,
      gap: GAP,
      headerHeight: HEADER,
    }),
  );
  $effect(() => {
    onlayout?.(computed);
  });
  const rows = $derived(
    visibleRows(computed.rows, scrollTop, height, OVERSCAN),
  );

  $effect(() => {
    if (!viewport) return;
    const observer = new ResizeObserver(([entry]) => {
      if (!entry) return;
      width = entry.contentRect.width;
      height = entry.contentRect.height;
    });
    observer.observe(viewport);
    return () => observer.disconnect();
  });

  // Load the photos the visible rows need.
  $effect(() => {
    let last = -1;
    for (const r of rows)
      if (r.kind === "tiles") last = Math.max(last, r.end - 1);
    if (last >= 0) void store.ensureLoaded(last);
  });

  // Report the photos on screen (without overscan).
  $effect(() => {
    if (!onvisible) return;
    const ids: number[] = [];
    for (const r of visibleRows(computed.rows, scrollTop, height, 0))
      if (r.kind === "tiles")
        for (let i = r.start; i < r.end; i++) {
          const photo = store.photos[i];
          if (photo) ids.push(photo.id);
        }
    onvisible(ids);
  });

  // Keep the focused photo in view.
  $effect(() => {
    const focus = selection.focus;
    if (focus === null || !viewport) return;
    const top = rowTopOf(computed.rows, focus);
    if (top === null) return;
    if (top < viewport.scrollTop) viewport.scrollTop = top - HEADER;
    else if (top + computed.tile > viewport.scrollTop + height)
      viewport.scrollTop = top + computed.tile - height + GAP;
  });

  let frame = 0;
  function onscroll() {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(() => (scrollTop = viewport?.scrollTop ?? 0));
  }

  const dayFormat = new Intl.DateTimeFormat(undefined, {
    weekday: "short",
    day: "numeric",
    month: "long",
    year: "numeric",
  });

  function dayLabel(section: Section): string {
    if (section.label) return section.label;
    if (section.day === null) return "No date";
    const [y, m, d] = section.day.split("-").map(Number);
    return dayFormat.format(new Date(y!, (m ?? 1) - 1, d ?? 1));
  }

  const seriesOf = (index: number) => store.photos[index]?.series_id ?? null;

  /** The stack button of a tile: on every stacked series' visible photo,
   * and on the best photo of an expanded one. */
  function stackOf(photo: PhotoSummary) {
    if (!stacked || photo.series_id === null || photo.series_size < 2)
      return null;
    const open = expanded.has(photo.series_id);
    if (open && !photo.best_in_series) return null;
    return { count: photo.series_size, expanded: open };
  }

  function select(index: number, e: MouseEvent) {
    selection.click(index, {
      shift: e.shiftKey,
      toggle: e.ctrlKey || e.metaKey,
    });
  }
</script>

<div
  class="viewport"
  bind:this={viewport}
  {onscroll}
  data-testid="timeline"
  tabindex="-1"
>
  <div class="canvas" style:height={`${computed.height}px`}>
    {#each rows as row (row.kind + row.top)}
      {#if row.kind === "header"}
        <div
          class="header"
          style:top={`${row.top}px`}
          style:height={`${row.height}px`}
          data-testid="day-header"
          data-day={row.section.day ?? "none"}
        >
          <h3>{dayLabel(row.section)}</h3>
          <span class="muted">
            {[
              ...store.places(row.section),
              `${row.section.count} photo${row.section.count === 1 ? "" : "s"}`,
            ].join(" · ")}
          </span>
        </div>
      {:else}
        <div class="row" style:top={`${row.top}px`} style:gap={`${GAP}px`}>
          {#each seriesSegments(row.start, row.end, seriesOf, computed.tile, GAP) as segment (segment.left)}
            <div
              class="series-frame"
              class:open-start={segment.openStart}
              class:open-end={segment.openEnd}
              style:left={`${segment.left - FRAME}px`}
              style:width={`${segment.width + 2 * FRAME}px`}
              style:height={`${computed.tile + 2 * FRAME}px`}
              data-testid="series-frame"
              data-series-id={segment.seriesId}
            ></div>
          {/each}
          <!-- Keyed by photo too: a reused <img> keeps showing the previous
               photo until the new thumbnail has loaded. -->
          {#each { length: row.end - row.start } as _, k (`${row.start + k}:${store.photos[row.start + k]?.id}`)}
            {@const index = row.start + k}
            {@const photo = store.photos[index]}
            {#if photo}
              <Tile
                {photo}
                size={computed.tile}
                selected={selection.has(index)}
                focused={selection.focus === index}
                onclick={(e) => select(index, e)}
                ondblclick={() => onopen?.(index)}
                ontoggle={() => selection.click(index, { toggle: true })}
                selecting={selection.size > 1}
                stack={stackOf(photo)}
                {showQuality}
                onstack={() =>
                  photo.series_id !== null && onstack?.(photo.series_id)}
              />
            {:else}
              <div
                class="placeholder"
                style:width={`${computed.tile}px`}
                style:height={`${computed.tile}px`}
              ></div>
            {/if}
          {/each}
        </div>
      {/if}
    {/each}
  </div>
</div>

<style>
  .viewport {
    position: relative;
    overflow-y: auto;
    height: 100%;
    /* Room for series frames at the edges. */
    padding: 0 6px 6px;
    outline: none;
  }

  .canvas {
    position: relative;
  }

  .header,
  .row {
    position: absolute;
    left: 0;
    right: 0;
  }

  .header {
    display: flex;
    align-items: baseline;
    gap: 0.8rem;
    padding-top: 0.9rem;
  }

  .header h3 {
    font-size: 1rem;
  }

  .header span {
    font-size: 0.85rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .row {
    display: flex;
  }

  .series-frame {
    position: absolute;
    top: -4px;
    border: 2px solid color-mix(in srgb, var(--fg) 28%, transparent);
    border-radius: 10px;
    background: color-mix(in srgb, var(--fg) 4%, transparent);
    pointer-events: none;
  }

  .series-frame.open-start {
    border-left-style: dashed;
    border-top-left-radius: 0;
    border-bottom-left-radius: 0;
  }

  .series-frame.open-end {
    border-right-style: dashed;
    border-top-right-radius: 0;
    border-bottom-right-radius: 0;
  }

  .placeholder {
    border-radius: 6px;
    background: var(--surface-2);
  }
</style>
