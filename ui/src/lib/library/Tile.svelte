<script lang="ts">
  import { imageUrl, type PhotoSummary } from "../api";

  interface Props {
    photo: PhotoSummary;
    size: number;
    selected: boolean;
    focused: boolean;
    onclick: (e: MouseEvent) => void;
    ondblclick: () => void;
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
    stack = null,
    onstack,
    showQuality = false,
  }: Props = $props();

  const quality = $derived(
    photo.quality === null ? null : `${Math.round(photo.quality * 100)}%`,
  );
  let failed = $state(false);

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
      {#if photo.flag === "pick"}<span class="badge pick" title="Pick">✓</span
        >{/if}
      {#if photo.flag === "reject"}<span class="badge reject" title="Rejected"
          >✕</span
        >{/if}
      {#if photo.has_raw}<span class="badge raw">RAW</span>{/if}
      {#if showQuality && quality}<span
          class="badge"
          title="Overall technical quality"
          data-testid="tile-quality">{quality}</span
        >{/if}
    </span>
    {#if photo.best_in_series}
      <span class="best-mark" title="Best of the series" data-testid="best-mark"
        >★</span
      >
    {/if}
    {#if photo.rating > 0}
      <span class="stars" aria-label={`${photo.rating} stars`}
        >{"★".repeat(photo.rating)}</span
      >
    {/if}
  </button>
  {#if stack}
    <button
      class="stack"
      data-testid="stack-toggle"
      aria-expanded={stack.expanded}
      title={stack.expanded
        ? "Stack this series"
        : `Show all ${stack.count} photos of this series`}
      onclick={() => onstack?.()}
      >{stack.expanded ? "−" : `⧉ ${stack.count}`}</button
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
    border-radius: 6px;
    background: var(--surface-2);
    overflow: hidden;
    cursor: default;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .tile:hover {
    border-color: var(--border);
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
    top: 4px;
    left: 4px;
    display: flex;
    gap: 3px;
  }

  .badge {
    font-size: 0.65rem;
    font-weight: 700;
    padding: 1px 5px;
    border-radius: 4px;
    background: rgb(0 0 0 / 0.55);
    color: #fff;
  }

  .badge.pick {
    background: var(--accent);
  }

  .badge.reject {
    background: var(--danger);
  }

  .best-mark {
    position: absolute;
    top: 4px;
    right: 4px;
    width: 1.3rem;
    height: 1.3rem;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: var(--accent);
    color: var(--accent-fg);
    font-size: 0.75rem;
  }

  .stack {
    position: absolute;
    bottom: 4px;
    left: 4px;
    padding: 1px 6px;
    border: none;
    border-radius: 4px;
    background: rgb(0 0 0 / 0.65);
    color: #fff;
    font-size: 0.72rem;
    font-weight: 700;
  }

  .stack:hover {
    background: rgb(0 0 0 / 0.85);
  }

  .stars {
    position: absolute;
    bottom: 4px;
    right: 6px;
    color: #ffd25e;
    font-size: 0.8rem;
    text-shadow: 0 0 3px rgb(0 0 0 / 0.8);
  }
</style>
