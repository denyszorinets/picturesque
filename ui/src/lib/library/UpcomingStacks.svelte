<script lang="ts">
  import { imageUrl, type PhotoSummary } from "../api";
  import ResizeHandle from "../components/ResizeHandle.svelte";
  import type { LibraryStore } from "./store.svelte";

  interface Props {
    store: LibraryStore;
    /** The list entry open in the loupe. */
    index: number;
    /** Opens another entry. */
    onselect: (index: number) => void;
    /** Species names of a photo, when known (Bird ID). */
    species?: (photo: PhotoSummary) => string | null;
  }

  let { store, index, onselect, species }: Props = $props();

  /** Stacks shown before and after the open one. */
  const BEHIND = 30;
  const AHEAD = 30;
  /** Width of the pane, resizable; thumbnails grow with it. */
  let width = $state(240);

  $effect(() => {
    void store.ensureLoaded(Math.min(store.total - 1, index + AHEAD));
  });

  const first = $derived(Math.max(0, index - BEHIND));
  const entries = $derived(
    Array.from(
      { length: Math.max(0, Math.min(store.total, index + AHEAD + 1) - first) },
      (_, k) => ({ at: first + k, photo: store.photos[first + k] }),
    ).filter((e): e is { at: number; photo: PhotoSummary } => !!e.photo),
  );
  const remaining = $derived(Math.max(0, store.total - index - 1));
  /** Wide enough for the thumbnail above its caption. */
  const stacked = $derived(width >= 300);

  function time(p: PhotoSummary): string {
    return p.captured_at ? p.captured_at.slice(11, 19) : "";
  }

  // Keeps the open stack in view, with the ones before it above.
  let list: HTMLElement | undefined = $state();
  $effect(() => {
    void index;
    void entries.length;
    list
      ?.querySelector<HTMLElement>("[aria-current=true]")
      ?.scrollIntoView?.({ block: "center" });
  });
</script>

<aside
  class="upnext"
  style:width="{width}px"
  aria-label="Stacks"
  data-testid="upcoming-stacks"
>
  <header>
    <h2>Stacks</h2>
    <p data-testid="upcoming-remaining">
      {index + 1} of {store.total} · {remaining === 0
        ? "last stack"
        : `${remaining} more`}
    </p>
  </header>
  <ol bind:this={list} class:stacked>
    {#each entries as { at, photo } (photo.id)}
      {@const name = species?.(photo) ?? null}
      <li>
        <button
          class:current={at === index}
          class:seen={at < index}
          aria-current={at === index}
          onclick={(e) => {
            e.currentTarget.blur();
            if (at !== index) onselect(at);
          }}
          title={photo.file_name}
          data-testid="upcoming-entry"
        >
          <span class="thumb">
            <img src={imageUrl(photo.id, "grid")} alt="" draggable="false" />
            {#if photo.series_size > 1}
              <span class="count" data-testid="upcoming-count"
                >×{photo.series_size}</span
              >
            {/if}
          </span>
          <span class="meta">
            <span class="title"
              >{name ??
                (photo.series_size > 1
                  ? `${photo.series_size} frames`
                  : "Single")}</span
            >
            <span class="sub"
              >{time(photo)}{#if photo.flag === "pick"}<span
                  class="pick"
                  aria-label="Pick"
                >
                  ✓</span
                >{:else if photo.flag === "reject"}<span
                  class="reject"
                  aria-label="Reject"
                >
                  ✕</span
                >{/if}</span
            >
          </span>
        </button>
      </li>
    {/each}
  </ol>
  <ResizeHandle
    bind:width
    edge="right"
    min={180}
    max={640}
    storageKey="picturesque.stacksWidth"
    label="Resize the stacks pane"
  />
</aside>

<style>
  .upnext {
    position: relative;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    background: #171716;
    border-right: 1px solid #2a2a27;
    min-height: 0;
  }

  header {
    padding: 0.9rem 0.9rem 0.5rem;
  }

  h2 {
    font-size: 0.8rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: #9a9a94;
    margin: 0 0 0.2rem;
  }

  header p {
    margin: 0;
    font-size: 0.85rem;
    font-variant-numeric: tabular-nums;
  }

  ol {
    list-style: none;
    margin: 0;
    padding: 0 0.5rem 0.8rem;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  button {
    display: flex;
    gap: 0.6rem;
    align-items: center;
    width: 100%;
    padding: 0.3rem;
    border: 1px solid transparent;
    border-radius: 6px;
    background: transparent;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }

  .stacked button {
    flex-direction: column;
    align-items: stretch;
    gap: 0.3rem;
  }

  button:hover {
    background: #22221f;
  }

  button.current {
    border-color: #ececea;
    background: #22221f;
  }

  /* Stacks already looked at stay, a little quieter. */
  button.seen:not(:hover) {
    opacity: 0.6;
  }

  /* Grows with the pane: about 40% of it, or all of it when stacked. */
  .thumb {
    position: relative;
    width: 42%;
    aspect-ratio: 3 / 2;
    flex-shrink: 0;
    border-radius: 4px;
    overflow: hidden;
    background: #0d0d0c;
  }

  .stacked .thumb {
    width: 100%;
  }

  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .count {
    position: absolute;
    right: 2px;
    bottom: 2px;
    padding: 0 0.25rem;
    border-radius: 3px;
    background: rgb(13 13 12 / 0.8);
    font-size: 0.7rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }

  .meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .title {
    font-size: 0.82rem;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sub {
    font-size: 0.75rem;
    color: #9a9a94;
    font-variant-numeric: tabular-nums;
  }

  .pick {
    color: #8ee3ad;
  }

  .reject {
    color: #ef6a5b;
  }

  @media (max-width: 700px) {
    .upnext {
      display: none;
    }
  }
</style>
