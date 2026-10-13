<script lang="ts">
  import { imageUrl, type PhotoSummary } from "../api";
  import type { CompareState } from "./compare.svelte";

  interface Props {
    photo: PhotoSummary | undefined;
    compare: CompareState;
    active: boolean;
    /** 1-based position among the compared photos. */
    position: number;
    /** Ranking score in the series (0–1), when known. */
    score: number | null;
    onactivate: () => void;
  }

  let { photo, compare, active, position, score, onactivate }: Props = $props();

  let scroller: HTMLDivElement | undefined = $state();
  let full: HTMLImageElement | undefined = $state();
  let loaded = $state(false);
  /** This pane is being dragged or scrolled: it leads the others. */
  let leading = false;
  let wheelTimer: ReturnType<typeof setTimeout> | undefined;
  let drag: { x: number; y: number; left: number; top: number } | null = null;

  // A different photo, or zooming out, needs the full image loaded again.
  $effect(() => {
    void photo?.id;
    if (!compare.zoomed) loaded = false;
  });

  // Follow the shared centre unless this pane is leading.
  $effect(() => {
    const { x, y } = compare.center;
    if (!loaded || !scroller || !full || leading) return;
    scroller.scrollLeft = full.naturalWidth * x - scroller.clientWidth / 2;
    scroller.scrollTop = full.naturalHeight * y - scroller.clientHeight / 2;
  });

  function onscroll() {
    if (!leading || !scroller || !full || !full.naturalWidth) return;
    compare.center = {
      x: (scroller.scrollLeft + scroller.clientWidth / 2) / full.naturalWidth,
      y: (scroller.scrollTop + scroller.clientHeight / 2) / full.naturalHeight,
    };
  }

  function onwheel() {
    leading = true;
    clearTimeout(wheelTimer);
    wheelTimer = setTimeout(() => (leading = false), 200);
  }

  function onpointerdown(e: PointerEvent) {
    onactivate();
    if (!scroller) return;
    leading = true;
    drag = {
      x: e.clientX,
      y: e.clientY,
      left: scroller.scrollLeft,
      top: scroller.scrollTop,
    };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function onpointermove(e: PointerEvent) {
    if (!drag || !scroller) return;
    scroller.scrollLeft = drag.left - (e.clientX - drag.x);
    scroller.scrollTop = drag.top - (e.clientY - drag.y);
  }

  function onpointerup() {
    drag = null;
    leading = false;
  }

  function ondblclick(e: MouseEvent) {
    const img = e.currentTarget as HTMLImageElement;
    const rect = img.getBoundingClientRect();
    compare.toggleZoom({
      x: (e.clientX - rect.left) / rect.width,
      y: (e.clientY - rect.top) / rect.height,
    });
  }
</script>

<div
  class="pane"
  class:active
  class:rejected={photo?.flag === "reject"}
  data-testid="compare-pane"
  data-photo-id={photo?.id}
  data-active={active}
>
  <div class="view">
    {#if !photo}
      <p class="muted">Loading…</p>
    {:else if compare.zoomed}
      <div
        class="scroller"
        bind:this={scroller}
        role="presentation"
        {onscroll}
        {onwheel}
        {onpointerdown}
        {onpointermove}
        {onpointerup}
        data-testid="compare-zoomed"
      >
        {#if !loaded}<p class="muted">Loading full size…</p>{/if}
        <img
          class="full"
          class:hidden={!loaded}
          bind:this={full}
          src={imageUrl(photo.id, "full")}
          alt={photo.file_name}
          draggable="false"
          onload={() => (loaded = true)}
        />
      </div>
    {:else}
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
      <img
        class="fitted"
        src={imageUrl(photo.id, "preview")}
        alt={photo.file_name}
        draggable="false"
        onclick={onactivate}
        {ondblclick}
      />
    {/if}
  </div>
  {#if photo}
    <div class="caption">
      <span class="position">{position}</span>
      <span class="name" title={photo.file_name}>{photo.file_name}</span>
      {#if score !== null}
        <span class="muted" title="Ranking score in the series"
          >{Math.round(score * 100)}%</span
        >
      {/if}
      {#if photo.rating > 0}<span class="stars">{"★".repeat(photo.rating)}</span
        >{/if}
      {#if photo.flag === "pick"}<span class="badge pick">✓ Pick</span>{/if}
      {#if photo.flag === "reject"}<span class="badge reject">✕ Rejected</span
        >{/if}
      {#if photo.best_in_series}<span
          class="badge best"
          data-testid="compare-best">★ Best</span
        >{/if}
    </div>
  {/if}
</div>

<style>
  .pane {
    min-width: 0;
    min-height: 0;
    display: grid;
    grid-template-rows: minmax(0, 1fr) auto;
    border: 2px solid transparent;
    border-radius: 6px;
    overflow: hidden;
    background: #151514;
  }

  .pane.active {
    border-color: #ececea;
  }

  .view {
    position: relative;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .fitted {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    user-select: none;
  }

  .pane.rejected .fitted,
  .pane.rejected .full {
    opacity: 0.4;
  }

  .scroller {
    position: absolute;
    inset: 0;
    overflow: auto;
    cursor: grab;
    display: grid;
    place-items: center;
  }

  .scroller:active {
    cursor: grabbing;
  }

  .full {
    max-width: none;
    user-select: none;
  }

  .full.hidden {
    visibility: hidden;
    position: absolute;
  }

  .caption {
    height: 2.2rem;
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0 0.6rem;
    background: rgb(20 20 19 / 0.9);
    font-size: 0.8rem;
  }

  .position {
    font-weight: 700;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .muted {
    color: #9a9a94;
  }

  .stars {
    color: #ffd25e;
  }

  .badge {
    padding: 1px 6px;
    border-radius: 4px;
    font-weight: 700;
    font-size: 0.72rem;
    white-space: nowrap;
  }

  .badge.pick,
  .badge.best {
    background: var(--accent);
    color: var(--accent-fg);
  }

  .badge.reject {
    background: var(--danger);
    color: #fff;
  }
</style>
