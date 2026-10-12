<script lang="ts">
  import { imageUrl, type PhotoSummary } from "../api";

  interface Props {
    photo: PhotoSummary;
    size: number;
    selected: boolean;
    focused: boolean;
    onclick: (e: MouseEvent) => void;
    ondblclick: () => void;
  }

  let { photo, size, selected, focused, onclick, ondblclick }: Props = $props();
  let failed = $state(false);

  const title = $derived(
    [
      photo.file_name,
      photo.captured_at?.replace("T", " "),
      photo.camera,
      photo.city,
    ]
      .filter(Boolean)
      .join(" · "),
  );
</script>

<!-- Series frames (M3) wrap tiles; the tile itself only shows one photo. -->
<button
  class="tile"
  class:selected
  class:focused
  class:rejected={photo.flag === "reject"}
  style:width={`${size}px`}
  style:height={`${size}px`}
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
  </span>
  {#if photo.rating > 0}
    <span class="stars" aria-label={`${photo.rating} stars`}
      >{"★".repeat(photo.rating)}</span
    >
  {/if}
</button>

<style>
  .tile {
    position: relative;
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

  .stars {
    position: absolute;
    bottom: 4px;
    right: 6px;
    color: #ffd25e;
    font-size: 0.8rem;
    text-shadow: 0 0 3px rgb(0 0 0 / 0.8);
  }
</style>
