<script lang="ts">
  import * as api from "../api";
  import { AnalysisProgress } from "./analysis.svelte";
  import { SidecarProgress } from "./sidecars.svelte";
  import { flagForKey } from "./keys";
  import Compare from "./Compare.svelte";
  import ExportDialog from "./ExportDialog.svelte";
  import Reanalyze from "../analysis/Reanalyze.svelte";
  import { waitingNote } from "../analysis/models";
  import Tour from "../tour/Tour.svelte";
  import { CULLING_TOUR } from "../tour/steps";
  import RejectsMove from "./RejectsMove.svelte";
  import { Culling } from "./culling.svelte";
  import FilterPanel from "./FilterPanel.svelte";
  import { Filters } from "./filters.svelte";
  import { neighbour, type Direction, type Layout } from "./layout";
  import Loupe from "./Loupe.svelte";
  import { Selection } from "./selection.svelte";
  import { LibraryStore } from "./store.svelte";
  import Timeline from "./Timeline.svelte";

  interface Props {
    /** Opens the import activity. */
    onimport?: () => void;
    /** Opens Settings (to download AI models). */
    onsettings?: () => void;
    /** Opens the setup wizard. */
    onsetup?: () => void;
    /** Whether an output (library) folder is set; importing waits for one. */
    hasLibrary?: boolean;
    /** Whether to show the culling tour (once photos are on screen). */
    tour?: boolean;
    /** Called when the tour is finished or skipped. */
    ontourend?: () => void;
    /** Whether photos are being imported. */
    importing?: boolean;
    store?: LibraryStore;
    selection?: Selection;
  }

  let {
    onimport,
    onsettings,
    onsetup,
    hasLibrary = true,
    tour = false,
    ontourend,
    importing = false,
    store = new LibraryStore(api),
    selection = new Selection(),
  }: Props = $props();

  const filters = new Filters();
  const analysis = new AnalysisProgress(api);
  const sidecars = new SidecarProgress(api);
  let showFilters = $state(true);

  /** The search box's text; searching starts on Enter. */
  let searchText = $state(filters.search);
  let searchInput: HTMLInputElement | undefined = $state();
  let searchError = $state<string | null>(null);
  let searching = $state(false);
  let searchRun = 0;

  /** Shows the photos matching the filters, or the search results. */
  async function apply(query: api.PhotoQuery, text: string) {
    const run = ++searchRun;
    if (!text) {
      searchError = null;
      searching = false;
      return store.setFilter(query);
    }
    searching = true;
    try {
      const found = await api.searchPhotos(text);
      if (run !== searchRun) return;
      searchError = null;
      await store.setFilter(query, {
        ids: found.map((f) => f.photo_id),
        label: `Matching “${text}”`,
      });
    } catch (e) {
      if (run === searchRun) searchError = api.ApiError.from(e).message;
    } finally {
      if (run === searchRun) searching = false;
    }
  }

  // Reload whenever the filters change; indices change, so selection resets.
  let lastFilter: string | null = null;
  $effect(() => {
    const query = filters.toQuery();
    const text = filters.search;
    const key = JSON.stringify([query, text]);
    if (key === lastFilter) return;
    const first = lastFilter === null;
    lastFilter = key;
    filters.save();
    searchText = text;
    if (!first) selection.clear();
    void apply(query, text);
  });

  /** Ids of the selected photos that are loaded. */
  const selectedIds = $derived(
    [...selection.selected]
      .map((i) => store.photos[i]?.id)
      .filter((id): id is number => id !== undefined),
  );

  /** Index of the photo open in the loupe. */
  let loupe = $state<number | null>(null);
  let layout = $state<Layout>();
  const culling = $derived(new Culling(api, store));
  // Flag and rating counts follow decisions.
  // A decision changes the counts; under a filter on decisions (flags,
  // auto-picked, rating) it also changes which photos match.
  $effect(() =>
    culling.subscribe(() => {
      const byDecision =
        filters.flags.length > 0 || filters.autoPicked || filters.minRating > 0;
      void (byDecision ? store.refresh() : store.refreshFacets());
    }),
  );

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

  const AUTO_ADVANCE_KEY = "picturesque.autoAdvance";
  /** In the loupe, move to the next photo after every decision. */
  let autoAdvance = $state(
    (() => {
      try {
        return localStorage.getItem(AUTO_ADVANCE_KEY) === "true";
      } catch {
        return false;
      }
    })(),
  );
  $effect(() => {
    try {
      localStorage.setItem(AUTO_ADVANCE_KEY, String(autoAdvance));
    } catch {
      // A convenience only.
    }
  });

  /** After a decision: the next photo in the loupe, or the next tile when
   * one photo is selected. */
  function advance() {
    const total = store.total;
    if (loupe !== null) {
      if (loupe + 1 < total) {
        loupe += 1;
        selection.click(loupe);
        void store.ensureLoaded(loupe);
      }
      return;
    }
    if (selection.size <= 1 && selection.focus !== null) {
      const to = Math.min(selection.focus + 1, total - 1);
      void store.ensureLoaded(to);
      selection.moveTo(to, total);
    }
  }

  function decide(change: () => Promise<unknown>, next: boolean) {
    void change();
    if (next) advance();
  }

  /** Turns every auto-picked photo into a pick, as one undoable step. */
  async function acceptAutoPicks() {
    const photos: api.PhotoSummary[] = [];
    let cursor: string | null = null;
    try {
      do {
        const page: api.PhotoPage = await api.queryPhotos({
          auto_picked: true,
          limit: 1000,
          cursor: cursor ?? undefined,
        });
        photos.push(...page.photos);
        cursor = page.next_cursor;
      } while (cursor !== null);
    } catch (e) {
      culling.error = api.ApiError.from(e).message;
      return;
    }
    await culling.flagPhotos(photos, "pick");
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
    // Shift moves on after the decision; in the loupe, so does
    // Auto-advance.
    const next = e.shiftKey || (loupe !== null && autoAdvance);
    const flag = flagForKey(e.key, loupe === null ? "grid" : "loupe");
    if (flag) {
      const photos = targets();
      decide(() => culling.flag(photos, flag), next);
      return true;
    }
    if (/^[0-5]$/.test(e.key)) {
      const photos = targets();
      decide(() => culling.rate(photos, Number(e.key)), next);
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
  let exporting = $state(false);

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
    const target = e.target instanceof Element ? e.target : null;
    if (
      e.defaultPrevented ||
      target?.closest("input, select, textarea, [contenteditable]")
    )
      return;
    // The library is hidden while the Import tab is shown; the compare
    // view handles its own keys.
    if (!section?.offsetParent || compare || exporting) return;
    if (e.key === "/" && loupe === null) {
      e.preventDefault();
      searchInput?.focus();
      return;
    }
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

  /** While importing, new photos show up at most this often. */
  const LIVE_REFRESH_MS = 1500;
  let lastImported: number | null = null;
  let liveTimer: ReturnType<typeof setTimeout> | undefined;

  function photosImported() {
    if (liveTimer) return;
    liveTimer = setTimeout(() => {
      liveTimer = undefined;
      void (store.total === 0 ? store.load() : store.refresh());
    }, LIVE_REFRESH_MS);
  }

  $effect(() => {
    void analysis.load();
    void sidecars.load();
    const unsubscribe = api.onAppEvent((event) => {
      if (event.type === "pipeline_progress") {
        if (event.imported !== lastImported) photosImported();
        lastImported = event.imported;
      } else if (event.type === "series_changed") void store.refresh();
      analysis.handle(event);
      sidecars.handle(event);
    });
    return () => {
      unsubscribe();
      clearTimeout(liveTimer);
      analysis.dispose();
    };
  });
</script>

<svelte:window {onkeydown} />

<section class="library" aria-label="Library" bind:this={section}>
  {#if store.error}
    <p class="error-text" role="alert">{store.error}</p>
  {:else if store.facets && store.total === 0 && !filters.active}
    <div class="empty card" data-testid="library-empty">
      {#if importing}
        <h2>Importing…</h2>
        <p class="muted">Photos appear here as soon as they are imported.</p>
        <button onclick={() => onimport?.()}>Show activity</button>
      {:else if !hasLibrary}
        <h2>Choose an output folder first</h2>
        <p class="muted">
          Picturesque copies your photos into a library folder and never changes
          the originals. Choose that folder, then the folders to import from.
        </p>
        <button class="primary" onclick={() => onsetup?.()}
          >Set up Picturesque</button
        >
      {:else}
        <h2>No photos yet</h2>
        <p class="muted">
          Add a folder to import from. New photos in it are imported
          automatically.
        </p>
        <button class="primary" onclick={() => onimport?.()}
          >Add import folder</button
        >
      {/if}
    </div>
  {:else if store.facets}
    <div class="bar">
      <button
        class="small"
        aria-pressed={showFilters}
        onclick={() => (showFilters = !showFilters)}
        data-testid="toggle-filters">Filters</button
      >
      <form
        class="search"
        role="search"
        onsubmit={(e) => {
          e.preventDefault();
          filters.search = searchText.trim();
        }}
      >
        <input
          type="search"
          bind:value={searchText}
          bind:this={searchInput}
          placeholder="Search photos… (/)"
          aria-label="Search photos by description"
          data-testid="search"
          oninput={() => {
            // Clearing the box (also with its ✕) ends the search.
            if (!searchText) filters.search = "";
          }}
          onkeydown={(e) => {
            if (e.key === "Escape") {
              searchText = "";
              filters.search = "";
              searchInput?.blur();
            }
          }}
        />
      </form>
      <span data-testid="library-count"
        >{searching
          ? "Searching…"
          : `${store.total} ${store.total === 1 ? "photo" : "photos"}`}</span
      >
      {#if searchError}
        <span class="error-text" role="alert" data-testid="search-error"
          >{searchError}</span
        >
      {/if}
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
      <button
        class="small"
        onclick={() => (exporting = true)}
        title="Copy picks, the photos in view or the selection to a folder"
        data-testid="open-export">Export…</button
      >
      <Reanalyze
        reanalyze={api.reanalyzePhotos}
        photoIds={selectedIds}
        small
        label="Re-analyze"
        disabled={selectedIds.length === 0}
        title="Analyze the selected photos again (quality, series, faces and AI scores)"
      />
      <RejectsMove
        rejected={store.facets.flags.find((f) => f.flag === "reject")?.count ??
          0}
        onmoved={() => void store.refresh()}
      />
      <span class="spacer"></span>
      {#if analysis.status?.paused && analysis.remaining > 0}
        <span
          class="paused"
          data-testid="analysis-paused"
          title={`Analysis paused: ${analysis.status.paused}`}
          >Analysis paused: {analysis.status.paused.split(";")[0]}</span
        >
      {:else if analysis.status && analysis.remaining > 0}
        <span
          class="muted"
          data-testid="analysis-progress"
          title="Scoring photos in the background; photos on screen go first"
          >Analyzing {analysis.status.analyzed} / {analysis.status.total}</span
        >
      {/if}
      {#if sidecars.label}
        {@const folder = sidecars.status?.folder}
        {#if api.canReveal() && folder}
          <button
            class="link sidecars {sidecars.label.tone}"
            onclick={() => void api.revealInFolder(folder)}
            title={`${sidecars.label.title}. Click to open the folder.`}
            data-testid="sidecar-status">{sidecars.label.text}</button
          >
        {:else}
          <span
            class="sidecars {sidecars.label.tone}"
            title={sidecars.label.title}
            data-testid="sidecar-status">{sidecars.label.text}</span
          >
        {/if}
      {/if}
      {#if analysis.status && analysis.status.waiting_for_models.length > 0}
        <button
          class="link"
          onclick={() => onsettings?.()}
          title={waitingNote(analysis.status.waiting_for_models)}
          data-testid="analysis-needs-models">AI analysis needs models</button
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
          <FilterPanel
            {filters}
            facets={store.facets}
            onacceptauto={() => void acceptAutoPicks()}
          />
        </aside>
      {/if}
      <div class="grid">
        {#if store.total === 0}
          <div class="no-match">
            <p class="muted" data-testid="no-match">
              {filters.search
                ? `No photos match “${filters.search}”.`
                : "No photos match these filters."}
            </p>
            {#if filters.active}
              <button
                onclick={() => filters.clear()}
                data-testid="no-match-clear">Clear filters</button
              >
            {/if}
          </div>
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
    {#if tour && store.total > 0 && loupe === null && !compare && !exporting}
      <Tour steps={CULLING_TOUR} onend={() => ontourend?.()} />
    {/if}
    {#if exporting}
      <ExportDialog
        view={filters.search
          ? { photo_ids: store.photos.map((p) => p.id) }
          : filters.toQuery()}
        viewCount={store.total}
        selected={selectedIds}
        picks={store.facets.flags.find((f) => f.flag === "pick")?.count ?? 0}
        onclose={() => (exporting = false)}
      />
    {/if}
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
        libraryFolder={sidecars.status?.folder ?? null}
        index={loupe}
        onnavigate={(i) => {
          loupe = i;
          selection.click(i);
        }}
        onclose={() => (loupe = null)}
        onflag={(flag) => {
          const photos = targets();
          decide(() => culling.flag(photos, flag), autoAdvance);
        }}
        onrate={(rating) => {
          const photos = targets();
          decide(() => culling.rate(photos, rating), autoAdvance);
        }}
        bind:autoAdvance
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

  .search input {
    width: 14rem;
    padding: 0.25rem 0.6rem;
    font-size: 0.88rem;
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
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.6rem;
  }

  .paused {
    color: var(--warn);
    font-size: 0.9rem;
  }

  .sidecars {
    font-size: 0.85rem;
    white-space: nowrap;
  }

  .sidecars.ok {
    color: var(--accent);
  }

  .sidecars.busy {
    color: var(--muted);
  }

  .sidecars.error {
    color: var(--danger);
    font-weight: 600;
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
