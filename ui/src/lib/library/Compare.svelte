<script lang="ts">
  import * as api from "../api";
  import type { PhotoSummary } from "../api";
  import { CompareState } from "./compare.svelte";
  import ComparePane from "./ComparePane.svelte";
  import type { Culling } from "./culling.svelte";

  interface Props {
    /** The photos to compare, in order. */
    ids: number[];
    /** The photo to start on. */
    activeId?: number;
    /** Ranking scores by photo id, when comparing a series. */
    scores?: Record<number, number | null>;
    culling: Culling;
    /** Closes the view; gets the photo that was active. */
    onclose: (activeId: number | undefined) => void;
  }

  let { ids, activeId, scores = {}, culling, onclose }: Props = $props();

  // The view is recreated for each comparison, so the initial props suffice.
  // svelte-ignore state_referenced_locally
  const compare = new CompareState(ids, activeId);
  let photos = $state<Record<number, PhotoSummary>>({});
  let error = $state<string | null>(null);

  const PAN = 0.1;
  const FLAGS: Record<string, PhotoSummary["flag"]> = {
    p: "pick",
    x: "reject",
    u: "unflagged",
  };

  async function load() {
    try {
      const page = await api.queryPhotos({ photo_ids: ids, limit: 1000 });
      photos = Object.fromEntries(page.photos.map((p) => [p.id, p]));
    } catch (e) {
      error = api.ApiError.from(e).message;
    }
  }

  $effect(() => {
    void load();
    return culling.subscribe((decisions) => {
      const next = { ...photos };
      for (const d of decisions) {
        const p = next[d.photo_id];
        if (p) next[d.photo_id] = { ...p, flag: d.flag, rating: d.rating };
      }
      photos = next;
    });
  });

  const active = $derived(
    compare.activeId === undefined ? undefined : photos[compare.activeId],
  );

  async function chooseBest() {
    if (!active || active.series_id === null) return;
    error = null;
    try {
      await api.chooseBest(active.id, !active.best_in_series);
      await load();
    } catch (e) {
      error = api.ApiError.from(e).message;
    }
  }

  function onkeydown(e: KeyboardEvent) {
    const key = e.key.toLowerCase();
    const mod = e.ctrlKey || e.metaKey;
    let action: (() => unknown) | undefined;
    if (mod && key === "z")
      action = () => (e.shiftKey ? culling.redo() : culling.undo());
    else if (mod && key === "y") action = () => culling.redo();
    else if (mod || e.altKey) return;
    else if (e.shiftKey && compare.zoomed && e.key.startsWith("Arrow")) {
      const [dx, dy] = {
        ArrowLeft: [-PAN, 0],
        ArrowRight: [PAN, 0],
        ArrowUp: [0, -PAN],
        ArrowDown: [0, PAN],
      }[e.key] ?? [0, 0];
      action = () => compare.pan(dx!, dy!);
    } else if (e.key === "ArrowLeft") action = () => compare.move(-1);
    else if (e.key === "ArrowRight") action = () => compare.move(1);
    else if (e.key === "Home") action = () => compare.moveTo(0);
    else if (e.key === "End") action = () => compare.moveTo(ids.length - 1);
    else if (e.key === "Escape")
      action = () =>
        compare.zoomed ? compare.toggleZoom() : onclose(compare.activeId);
    else if (key === "l") action = () => compare.togglePanes();
    else if (key === "z") action = () => compare.toggleZoom();
    else if (key === "b") action = chooseBest;
    else if (FLAGS[key] && active)
      action = () => culling.flagPhotos([active], FLAGS[key]!);
    else if (/^[0-5]$/.test(e.key) && active)
      action = () => culling.ratePhotos([active], Number(e.key));
    if (!action) return;
    e.preventDefault();
    e.stopPropagation();
    void action();
  }
</script>

<svelte:window {onkeydown} />

<div
  class="compare"
  role="dialog"
  aria-modal="true"
  aria-label="Compare photos"
  data-testid="compare"
>
  <div
    class="panes"
    class:four={compare.panes === 4}
    data-panes={compare.panes}
  >
    {#each compare.visible as index (ids[index])}
      {@const id = ids[index]!}
      <ComparePane
        photo={photos[id]}
        {compare}
        active={index === compare.active}
        position={index + 1}
        score={scores[id] ?? null}
        onactivate={() => compare.moveTo(index)}
      />
    {/each}
  </div>
  <div class="toolbar">
    <button
      onclick={() => compare.move(-1)}
      disabled={compare.active === 0}
      aria-label="Previous photo">←</button
    >
    <span data-testid="compare-position"
      >{compare.active + 1} / {ids.length}</span
    >
    <button
      onclick={() => compare.move(1)}
      disabled={compare.active >= ids.length - 1}
      aria-label="Next photo">→</button
    >
    <button
      onclick={() => compare.togglePanes()}
      data-testid="compare-layout"
      title="Switch between 2-up and 4-up (L)">{compare.panes}-up</button
    >
    <button
      onclick={() => compare.toggleZoom()}
      aria-pressed={compare.zoomed}
      title="Zoom every photo to 100% (Z)"
      >{compare.zoomed ? "Fit" : "100%"}</button
    >
    {#if error ?? culling.error}
      <span class="error" role="alert">{error ?? culling.error}</span>
    {/if}
    <span class="keys"
      >← → photo · P X U pick/reject · 0–5 stars · B best · L layout · Z zoom ·
      Shift+arrows pan · Esc close</span
    >
    <button onclick={() => onclose(compare.activeId)} aria-label="Close compare"
      >✕</button
    >
  </div>
</div>

<style>
  .compare {
    position: fixed;
    inset: 0;
    z-index: 10;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 0.5rem;
    background: #0d0d0c;
    color: #ececea;
  }

  .panes {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 1fr 1fr;
    grid-template-rows: 1fr;
    gap: 0.5rem;
  }

  .panes.four {
    grid-template-rows: 1fr 1fr;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.85rem;
  }

  .toolbar button {
    background: transparent;
    color: inherit;
    border-color: #3a3a36;
    padding: 0.25rem 0.6rem;
  }

  .toolbar button[aria-pressed="true"] {
    background: #3a3a36;
  }

  .keys {
    flex: 1;
    text-align: right;
    color: #9a9a94;
    font-size: 0.78rem;
  }

  .error {
    color: var(--danger);
  }
</style>
