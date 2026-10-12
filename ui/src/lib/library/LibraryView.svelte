<script lang="ts">
  import * as api from "../api";
  import { Culling } from "./culling.svelte";
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
    // The library is hidden while the Import tab is shown.
    if (!section?.offsetParent) return;
    let used = cull(e);
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
    void store.load();
    // New photos appear when an import completes.
    return api.onAppEvent((event) => {
      if (event.type === "import_progress" && event.state === "completed")
        void store.load();
    });
  });
</script>

<svelte:window {onkeydown} />

<section class="library" aria-label="Library" bind:this={section}>
  {#if store.error}
    <p class="error-text" role="alert">{store.error}</p>
  {:else if store.facets && store.total === 0}
    <div class="empty card">
      <h2>No photos yet</h2>
      <p class="muted">Import a folder to start building your library.</p>
      <button class="primary" onclick={() => onimport?.()}>Import photos</button
      >
    </div>
  {:else if store.facets}
    <div class="bar">
      <span data-testid="library-count">{store.total} photos</span>
      {#if selection.size > 0}
        <span class="muted" data-testid="selection-count"
          >{selection.size} selected</span
        >
      {/if}
      <span class="spacer"></span>
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
    <div class="grid">
      <Timeline
        {store}
        {selection}
        onopen={open}
        onlayout={(l) => (layout = l)}
      />
    </div>
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

  .grid {
    flex: 1;
    min-height: 0;
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
