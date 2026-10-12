<script lang="ts">
  import { computeLayout, rowTopOf, visibleRows, type Section } from "./layout";
  import type { Selection } from "./selection.svelte";
  import type { LibraryStore } from "./store.svelte";
  import Tile from "./Tile.svelte";

  interface Props {
    store: LibraryStore;
    selection: Selection;
    /** Double-click or Enter on a photo. */
    onopen?: (index: number) => void;
  }

  let { store, selection, onopen }: Props = $props();

  const GAP = 6;
  const HEADER = 44;
  const OVERSCAN = 600;

  let viewport: HTMLDivElement | undefined = $state();
  let width = $state(0);
  let height = $state(0);
  let scrollTop = $state(0);

  const layout = $derived(
    computeLayout(store.sections, {
      width: Math.max(0, width),
      minTile: 180,
      gap: GAP,
      headerHeight: HEADER,
    }),
  );
  const rows = $derived(visibleRows(layout.rows, scrollTop, height, OVERSCAN));

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

  // Keep the focused photo in view.
  $effect(() => {
    const focus = selection.focus;
    if (focus === null || !viewport) return;
    const top = rowTopOf(layout.rows, focus);
    if (top === null) return;
    if (top < viewport.scrollTop) viewport.scrollTop = top - HEADER;
    else if (top + layout.tile > viewport.scrollTop + height)
      viewport.scrollTop = top + layout.tile - height + GAP;
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
    if (section.day === null) return "No date";
    const [y, m, d] = section.day.split("-").map(Number);
    return dayFormat.format(new Date(y!, (m ?? 1) - 1, d ?? 1));
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
  <div class="canvas" style:height={`${layout.height}px`}>
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
          {#each { length: row.end - row.start } as _, k (row.start + k)}
            {@const index = row.start + k}
            {@const photo = store.photos[index]}
            {#if photo}
              <Tile
                {photo}
                size={layout.tile}
                selected={selection.has(index)}
                focused={selection.focus === index}
                onclick={(e) => select(index, e)}
                ondblclick={() => onopen?.(index)}
              />
            {:else}
              <div
                class="placeholder"
                style:width={`${layout.tile}px`}
                style:height={`${layout.tile}px`}
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

  .placeholder {
    border-radius: 6px;
    background: var(--surface-2);
  }
</style>
