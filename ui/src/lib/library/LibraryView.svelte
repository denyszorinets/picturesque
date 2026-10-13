<script lang="ts">
  import * as api from "../api";
  import { AnalysisProgress } from "./analysis.svelte";
  import Compare from "./Compare.svelte";
  import { Culling } from "./culling.svelte";
  import FilterPanel from "./FilterPanel.svelte";
  import { Filters } from "./filters.svelte";
  import { neighbour, type Direction, type Layout } from "./layout";
  import Loupe from "./Loupe.svelte";
  import { Selection } from "./selection.svelte";
  import { LibraryStore } from "./store.svelte";
  import Timeline from "./Timeline.svelte";

  interface Props {
    onimport?: () => void;
    store?: LibraryStore;
    selection?: Selection;
  }

  let {
    onimport,
    store = new LibraryStore(api),
    selection = new Selection(),
  }: Props = $props();

  const filters = new Filters();
  const analysis = new AnalysisProgress(api);
  let showFilters = $state(true);

  // Reload whenever the filters change; indices change, so selection resets.
  let lastFilter: string | null = null;
  $effect(() => {
    const query = filters.toQuery();
    const key = JSON.stringify(query);
    if (key === lastFilter) return;
    const first = lastFilter === null;
    lastFilter = key;
    filters.save();
    if (!first) {
      selection.clear();
      void store.setFilter(query);
    }
  });

  /** Index of the photo open in the loupe. */
  let loupe = $state<number | null>(null);
  let layout = $state<Layout>();
  const culling = $derived(new Culling(api, store));

  const FLAGS: Record<string, "pick" | "reject" | "unflagged"> = {
    p: "pick",
    x: "reject",
    u: "unflagged",
  };
  const ARROWS: Record<string, Direction> = {
    ArrowLeft: "left",
    ArrowRight: "right",
    ArrowUp: "up",
    ArrowDown: "down",
  };

  /** The photos a culling key applies to: the loupe photo, else the selection. */
  function targets(): number[] {
    if (loupe !== null) return [loupe];
    if (selection.size > 0) return [...selection.selected];
    return selection.focus === null ? [] : [selection.focus];
  }

  /** Handles culling keys; returns whether the key was used. */
  function cull(e: KeyboardEvent): boolean {
    const mod = e.ctrlKey || e.metaKey;
    const key = e.key.toLowerCase();
    if (mod && key === "z") {
      void (e.shiftKey ? culling.redo() : culling.undo());
      return true;
    }
    if (mod && key === "y") {
      void culling.redo();
      return true;
    }
    if (mod || e.altKey) return false;
    const flag = FLAGS[key];
    if (flag) {
      void culling.flag(targets(), flag);
      return true;
    }
    if (/^[0-5]$/.test(e.key)) {
      void culling.rate(targets(), Number(e.key));
      return true;
    }
    return false;
  }

  /** Moves through the grid; returns whether the key was used. */
  function navigate(e: KeyboardEvent): boolean {
    const total = store.total;
    if (total === 0) return false;
    const from = selection.focus ?? -1;
    const direction = ARROWS[e.key];
    let to: number | null = null;
    if (direction)
      to =
        from < 0
          ? 0
          : layout
            ? neighbour(layout.rows, from, direction, total)
            : from;
    else if (e.key === "Home") to = 0;
    else if (e.key === "End") to = total - 1;
    else if (e.key === "Escape") {
      selection.clear();
      return true;
    }
    if (to === null) return false;
    void store.ensureLoaded(to);
    selection.moveTo(to, total, e.shiftKey);
    return true;
  }

  let seriesError = $state<string | null>(null);

  /** `s` / `j`: splits the series before the target photo, or joins it to
   * the photo taken before it; again removes the correction. */
  async function correctSeries(edit: api.SeriesEditView) {
    const [index] = targets();
    const photo = index === undefined ? undefined : store.photos[index];
    if (!photo) return;
    seriesError = null;
    try {
      const current = await api.photoSeries(photo.id);
      await api.editSeries(photo.id, current.edit === edit ? null : edit);
      await store.refresh();
    } catch (e) {
      seriesError = api.ApiError.from(e).message;
    }
  }

  /** The open comparison. */
  let compare = $state<{
    ids: number[];
    activeId?: number;
    scores?: Record<number, number | null>;
  } | null>(null);

  /** `c`: compares the selected photos, or else the series of the target
   * photo (best first). */
  async function openCompare() {
    seriesError = null;
    const focusId =
      (loupe !== null ? store.photos[loupe]?.id : undefined) ??
      (selection.focus !== null
        ? store.photos[selection.focus]?.id
        : undefined);
    if (loupe === null && selection.size >= 2) {
      const ids = [...selection.selected]
        .sort((a, b) => a - b)
        .map((i) => store.photos[i]?.id)
        .filter((id): id is number => id !== undefined);
      compare = { ids, activeId: focusId };
      return;
    }
    const [index] = targets();
    const photo = index === undefined ? undefined : store.photos[index];
    if (!photo || photo.series_id === null) {
      seriesError =
        "Select two or more photos, or a photo in a series, to compare.";
      return;
    }
    try {
      const series = await api.photoSeries(photo.id);
      if (series.ranking.length < 2) return;
      compare = {
        ids: series.ranking.map((r) => r.photo_id),
        activeId: photo.id,
        scores: Object.fromEntries(
          series.ranking.map((r) => [r.photo_id, r.score]),
        ),
      };
      loupe = null;
    } catch (e) {
      seriesError = api.ApiError.from(e).message;
    }
  }

  const SERIES_KEYS: Record<string, () => Promise<void>> = {
    b: chooseBest,
    c: openCompare,
    s: () => correctSeries("split"),
    j: () => correctSeries("join"),
  };

  function closeCompare(activeId: number | undefined) {
    compare = null;
    const index = store.photos.findIndex((p) => p.id === activeId);
    if (index >= 0) selection.click(index);
  }

  /** `b`: makes the target photo the best of its series, or withdraws
   * the choice when it already is. */
  async function chooseBest() {
    const [index] = targets();
    const photo = index === undefined ? undefined : store.photos[index];
    if (!photo || photo.series_id === null) return;
    seriesError = null;
    try {
      const series = await api.chooseBest(photo.id, !photo.best_in_series);
      // Recomputed synchronously: show it without waiting for the event.
      if (series.series_id !== null) await store.refresh();
    } catch (e) {
      seriesError = api.ApiError.from(e).message;
      return;
    }
  }

  function open(index: number) {
    selection.click(index);
    loupe = index;
  }

  function onkeydown(e: KeyboardEvent) {
    const target = e.target as HTMLElement | null;
    if (
      e.defaultPrevented ||
      target?.closest("input, select, textarea, [contenteditable]")
    )
      return;
    // The library is hidden while the Import tab is shown; the compare
    // view handles its own keys.
    if (!section?.offsetParent || compare) return;
    let used = cull(e);
    const seriesAction = SERIES_KEYS[e.key.toLowerCase()];
    if (!used && !e.ctrlKey && !e.metaKey && !e.altKey && seriesAction) {
      void seriesAction();
      used = true;
    }
    if (!used && loupe === null) {
      if (e.key === "Enter" && selection.focus !== null) {
        open(selection.focus);
        used = true;
      } else {
        used = navigate(e);
      }
    }
    if (used) e.preventDefault();
  }

  let section: HTMLElement | undefined = $state();

  $effect(() => {
    void store.setFilter(filters.toQuery());
    void analysis.load();
    // New photos appear when an import completes.
    const unsubscribe = api.onAppEvent((event) => {
      if (event.type === "import_progress" && event.state === "completed")
        void store.load();
      else if (event.type === "series_changed") void store.refresh();
      analysis.handle(event);
    });
    return () => {
      unsubscribe();
      analysis.dispose();
    };
  });
</script>

<svelte:window {onkeydown} />

<section class="library" aria-label="Library" bind:this={section}>
  {#if store.error}
    <p class="error-text" role="alert">{store.error}</p>
  {:else if store.facets && store.total === 0 && !filters.active}
    <div class="empty card">
      <h2>No photos yet</h2>
      <p class="muted">Import a folder to start building your library.</p>
      <button class="primary" onclick={() => onimport?.()}>Import photos</button
      >
    </div>
  {:else if store.facets}
    <div class="bar">
      <button
        class="small"
        aria-pressed={showFilters}
        onclick={() => (showFilters = !showFilters)}
        data-testid="toggle-filters">Filters</button
      >
      <span data-testid="library-count"
        >{store.total} {store.total === 1 ? "photo" : "photos"}</span
      >
      {#each filters.chips as chip (chip.label)}
        <span class="chip"
          >{chip.label}
          <button
            aria-label={`Remove filter ${chip.label}`}
            onclick={chip.clear}>✕</button
          ></span
        >
      {/each}
      {#if filters.active}
        <button
          class="link"
          onclick={() => filters.clear()}
          data-testid="clear-filters">Clear all</button
        >
      {/if}
      {#if selection.size > 0}
        <span class="muted" data-testid="selection-count"
          >{selection.size} selected</span
        >
      {/if}
      <select
        class="small"
        bind:value={filters.sort}
        aria-label="Sort"
        data-testid="sort"
      >
        <option value="captured_desc">Newest first</option>
        <option value="captured_asc">Oldest first</option>
        <option value="imported_desc">Recently imported</option>
        <option value="quality_desc">Best quality first</option>
      </select>
      <button
        class="small"
        aria-pressed={filters.stacked}
        onclick={() => {
          filters.stacked = !filters.stacked;
          filters.expanded.clear();
        }}
        title="Show each series as its best photo"
        data-testid="toggle-stack">Stack series</button
      >
      <button
        class="small"
        onclick={() => openCompare()}
        disabled={selection.focus === null && selection.size === 0}
        title="Compare the selected photos, or the selected photo's series (C)"
        data-testid="open-compare">Compare</button
      >
      <span class="spacer"></span>
      {#if analysis.status && analysis.remaining > 0}
        <span
          class="muted"
          data-testid="analysis-progress"
          title="Scoring photos in the background; photos on screen go first"
          >Analyzing {analysis.status.analyzed} / {analysis.status.total}</span
        >
      {/if}
      {#if seriesError}
        <span class="error-text" role="alert">{seriesError}</span>
      {/if}
      {#if culling.error}
        <span class="error-text" role="alert">{culling.error}</span>
      {/if}
      <button
        class="small"
        onclick={() => culling.undo()}
        disabled={!culling.canUndo}
        title="Undo (Ctrl+Z)">Undo</button
      >
      <button
        class="small"
        onclick={() => culling.redo()}
        disabled={!culling.canRedo}
        title="Redo (Ctrl+Shift+Z)">Redo</button
      >
    </div>
    <div class="body">
      {#if showFilters}
        <aside class="filters">
          <FilterPanel {filters} facets={store.facets} />
        </aside>
      {/if}
      <div class="grid">
        {#if store.total === 0}
          <p class="muted no-match" data-testid="no-match">
            No photos match these filters.
          </p>
        {:else}
          <Timeline
            {store}
            {selection}
            onopen={open}
            onlayout={(l) => (layout = l)}
            onvisible={(ids) => analysis.visible(ids)}
            stacked={filters.stacked}
            expanded={filters.expanded}
            onstack={(id) => filters.toggleExpanded(id)}
            showQuality={filters.sort === "quality_desc"}
          />
        {/if}
      </div>
    </div>
    {#if compare}
      <Compare
        ids={compare.ids}
        activeId={compare.activeId}
        scores={compare.scores}
        {culling}
        onclose={closeCompare}
      />
    {/if}
    {#if loupe !== null}
      <Loupe
        {store}
        index={loupe}
        onnavigate={(i) => {
          loupe = i;
          selection.click(i);
        }}
        onclose={() => (loupe = null)}
      />
    {/if}
  {/if}
</section>

<style>
  .library {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 1rem;
    padding-bottom: 0.4rem;
    font-size: 0.9rem;
  }

  .spacer {
    flex: 1;
  }

  .small {
    padding: 0.2rem 0.6rem;
    font-size: 0.85rem;
  }

  .body {
    flex: 1;
    min-height: 0;
    display: flex;
    gap: 1.2rem;
  }

  .filters {
    width: 15rem;
    flex-shrink: 0;
    overflow-y: auto;
    padding-right: 0.2rem;
  }

  .grid {
    flex: 1;
    min-width: 0;
    min-height: 0;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 0.2rem;
    padding: 0.1rem 0.2rem 0.1rem 0.6rem;
    border-radius: 999px;
    background: var(--surface-2);
    font-size: 0.82rem;
  }

  .chip button,
  .link {
    border: none;
    background: none;
    padding: 0.1rem 0.35rem;
    color: var(--muted);
    font-size: 0.82rem;
  }

  .link {
    text-decoration: underline;
  }

  .no-match {
    margin-top: 3rem;
    text-align: center;
  }

  .empty {
    max-width: 28rem;
    margin: 3rem auto;
    text-align: center;
    display: flex;
    flex-direction: column;
    gap: 0.8rem;
    align-items: center;
  }

  .empty p {
    margin: 0;
  }
</style>
