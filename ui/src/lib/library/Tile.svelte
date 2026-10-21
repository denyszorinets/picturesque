<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import { imageUrl, type PhotoSummary } from "../api";

  interface Props {
    photo: PhotoSummary;
    size: number;
    selected: boolean;
    focused: boolean;
    onclick: (e: MouseEvent) => void;
    ondblclick: () => void;
    /** Adds the photo to the selection, or takes it out. */
    ontoggle?: () => void;
    /** Whether several photos are being selected (checkboxes stay visible). */
    selecting?: boolean;
    /** A stacked series' size and whether it is expanded (stacking only). */
    stack?: { count: number; expanded: boolean } | null;
    onstack?: () => void;
    /** Show the quality score on the tile (when sorting by it). */
    showQuality?: boolean;
  }

  let {
    photo,
    size,
    selected,
    focused,
    onclick,
    ondblclick,
    ontoggle,
    selecting = false,
    stack = null,
    onstack,
    showQuality = false,
  }: Props = $props();

  const quality = $derived(
    photo.quality === null ? null : `${Math.round(photo.quality * 100)}%`,
  );
  let failed = $state(false);

  /** Picks get a green ring, the app's suggestions a light-green one. */
  const ring = $derived(
    photo.flag === "pick"
      ? "pick"
      : photo.keeper && photo.flag === "unflagged"
        ? "auto"
        : null,
  );

  const title = $derived(
    [
      photo.file_name,
      photo.captured_at?.replace("T", " "),
      photo.camera,
      photo.city,
      quality && `Quality ${quality}`,
    ]
      .filter(Boolean)
      .join(" · "),
  );
</script>

<!-- The timeline draws series frames behind tiles. -->
<div class="cell" style:width={`${size}px`} style:height={`${size}px`}>
  <button
    class="tile"
    class:selected
    class:focused
    class:best={photo.best_in_series}
    class:rejected={photo.flag === "reject"}
    {title}
    aria-label={photo.file_name}
    aria-pressed={selected}
    data-photo-id={photo.id}
    {onclick}
    {ondblclick}
  >
    {#if failed}
      <span class="broken">Can't show {photo.file_name}</span>
    {:else}
      <img
        src={imageUrl(photo.id, "grid")}
        alt={photo.file_name}
        loading="lazy"
        decoding="async"
        draggable="false"
        onerror={() => (failed = true)}
      />
    {/if}
    <span class="badges">
      {#if photo.flag === "pick"}<span class="badge pick" title="Pick"
          ><Icon name="check" /></span
        >{/if}
      {#if photo.keeper && photo.flag === "unflagged"}<span
          class="badge auto"
          title="Auto-picked: the app suggests keeping this photo. P makes it a pick, X rejects it."
          data-testid="tile-auto-picked"><Icon name="check" /> Auto</span
        >{/if}
      {#if photo.flag === "reject"}<span class="badge reject" title="Rejected"
          ><Icon name="x-mark" /></span
        >{/if}
      {#if photo.has_raw}<span class="badge raw">RAW</span>{/if}
      {#if photo.eyes_closed}<span
          class="badge eyes"
          title="Someone has closed eyes"
          data-testid="tile-eyes-closed">Eyes closed</span
        >{/if}
      {#if showQuality && quality}<span
          class="badge"
          title="Overall technical quality"
          data-testid="tile-quality">{quality}</span
        >{/if}
    </span>
    {#if ring}
      <span
        class="ring {ring}"
        aria-hidden="true"
        data-ring={ring}
        data-testid="tile-ring"
      ></span>
    {/if}
    {#if photo.best_in_series}
      <span class="best-mark" title="Best of the series" data-testid="best-mark"
        ><Icon name="star" /></span
      >
    {/if}
    {#if photo.rating > 0}
      <span class="stars" role="img" aria-label={`${photo.rating} stars`}
        >{#each { length: photo.rating }, i (i)}<Icon
            name="star"
          />{/each}</span
      >
    {/if}
  </button>
  {#if ontoggle}
    <button
      class="check"
      class:visible={selecting || selected}
      class:shifted={photo.best_in_series}
      role="checkbox"
      aria-checked={selected}
      aria-label={`Select ${photo.file_name}`}
      title={selected ? "Remove from the selection" : "Add to the selection"}
      onclick={() => ontoggle()}
      data-testid="tile-check"><Icon name="check" /></button
    >
  {/if}
  {#if stack}
    <button
      class="stack"
      data-testid="stack-toggle"
      aria-expanded={stack.expanded}
      title={stack.expanded
        ? "Stack this series"
        : `Show all ${stack.count} photos of this series`}
      aria-label={stack.expanded
        ? "Stack this series"
        : `Show all ${stack.count} photos of this series`}
      onclick={() => onstack?.()}
      ><Icon name="rectangle-stack" />{stack.expanded
        ? ""
        : stack.count}</button
    >
  {/if}
</div>

<style>
  .cell {
    position: relative;
    z-index: 1;
    flex-shrink: 0;
  }

  .tile {
    position: relative;
    width: 100%;
    height: 100%;
    padding: 0;
    border: 2px solid transparent;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    overflow: hidden;
    cursor: default;
    display: flex;
    align-items: center;
    justify-content: center;
    transition:
      border-color var(--dur-fast) var(--ease-out),
      box-shadow var(--dur-fast) var(--ease-out);
  }

  .tile:hover {
    border-color: var(--muted);
  }

  .tile.selected {
    border-color: var(--info);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--info) 35%, transparent);
  }

  .tile.best {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }

  .tile.best.selected {
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--info) 45%, transparent);
  }

  .tile.focused {
    outline: 2px solid var(--fg);
    outline-offset: 1px;
  }

  /* Inside the border, so selection and best-of-series stay visible. */
  .ring {
    position: absolute;
    inset: 0;
    border-radius: 4px;
    pointer-events: none;
  }

  .ring.pick {
    box-shadow: inset 0 0 0 3px var(--pick);
  }

  /* Quieter than a pick: often every photo of a folder is suggested. */
  .ring.auto {
    box-shadow: inset 0 0 0 2px var(--auto-pick);
  }

  .tile.rejected img {
    opacity: 0.35;
  }

  img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    user-select: none;
  }

  .broken {
    font-size: 0.75rem;
    color: var(--muted);
    padding: 0.5rem;
    overflow-wrap: anywhere;
  }

  .badges {
    position: absolute;
    top: 6px;
    left: 6px;
    display: flex;
    gap: 3px;
  }

  .badge {
    display: inline-flex;
    align-items: center;
    gap: 0.2em;
    --icon-size: 0.85rem;
    font-size: 0.7rem;
    font-weight: 700;
    line-height: 1.35;
    padding: 1px 6px;
    border-radius: 4px;
    background: rgb(0 0 0 / 0.62);
    color: #fff;
  }

  .badge.pick {
    background: var(--accent);
  }

  .badge.auto {
    color: #8ee3ad;
  }

  .badge.reject {
    background: var(--danger);
  }

  .badge.eyes {
    background: color-mix(in srgb, var(--danger) 80%, black);
  }

  .best-mark {
    position: absolute;
    top: 6px;
    right: 6px;
    width: 1.3rem;
    height: 1.3rem;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: var(--accent);
    color: var(--accent-fg);
    --icon-size: 0.8rem;
  }

  .check {
    position: absolute;
    top: 6px;
    right: 6px;
    width: 1.3rem;
    height: 1.3rem;
    padding: 0;
    display: grid;
    place-items: center;
    border-radius: 50%;
    border: 2px solid #fff;
    background: rgb(0 0 0 / 0.35);
    color: transparent;
    --icon-size: 0.8rem;
    opacity: 0;
    box-shadow: 0 0 3px rgb(0 0 0 / 0.6);
    transition: opacity var(--dur-fast) var(--ease-out);
  }

  .check:hover:not([aria-checked="true"]) {
    background: rgb(0 0 0 / 0.55);
  }

  .check.shifted {
    right: 1.9rem;
  }

  .cell:hover .check,
  .check.visible,
  .check:focus-visible {
    opacity: 1;
  }

  .check[aria-checked="true"] {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }

  .stack {
    position: absolute;
    bottom: 6px;
    left: 6px;
    padding: 1px 6px;
    border: none;
    border-radius: 4px;
    background: rgb(0 0 0 / 0.65);
    color: #fff;
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    --icon-size: 0.85rem;
    font-size: 0.72rem;
    font-weight: 700;
  }

  .stack:hover {
    background: rgb(0 0 0 / 0.85);
  }

  /* A dark pill like the other badges keeps the stars readable on any
     photo. No CSS filters on tiles: WebKitGTK repaints every filtered
     tile on any change, which froze the app with many photos (#192). */
  .stars {
    position: absolute;
    bottom: 6px;
    right: 6px;
    display: flex;
    padding: 2px 4px;
    border-radius: 4px;
    background: rgb(0 0 0 / 0.62);
    color: #ffd25e;
    --icon-size: 0.8rem;
  }
</style>
