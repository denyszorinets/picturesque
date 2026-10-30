<script lang="ts">
  import { imageUrl, type PhotoSummary } from "../api";

  interface Props {
    /** The series, in capture order. */
    members: PhotoSummary[];
    currentId: number;
    bestId: number | null;
    /** Ids that can be opened (they are in the list the loupe pages through). */
    reachable: (id: number) => boolean;
    onselect: (id: number) => void;
  }

  let { members, currentId, bestId, reachable, onselect }: Props = $props();

  const position = $derived(members.findIndex((p) => p.id === currentId) + 1);

  // Keeps the shown photo in view when the series is longer than the screen.
  let strip: HTMLElement | undefined = $state();
  $effect(() => {
    void currentId;
    strip
      ?.querySelector<HTMLElement>("[aria-current=true]")
      ?.scrollIntoView?.({ block: "nearest", inline: "center" });
  });
</script>

<nav
  class="ribbon"
  aria-label="Photos of this series"
  data-testid="series-ribbon"
>
  <span class="count" data-testid="series-ribbon-count"
    >{position} of {members.length}</span
  >
  <div class="strip" bind:this={strip}>
    {#each members as p (p.id)}
      {@const open = reachable(p.id)}
      <button
        class="frame"
        class:current={p.id === currentId}
        class:rejected={p.flag === "reject"}
        aria-current={p.id === currentId}
        disabled={!open}
        title={open
          ? `${p.file_name}${p.id === bestId ? " (best)" : ""}`
          : `${p.file_name}: not in the current view (stacked or filtered out)`}
        onclick={(e) => {
          e.currentTarget.blur();
          onselect(p.id);
        }}
        data-testid="series-ribbon-frame"
      >
        <img src={imageUrl(p.id, "grid")} alt={p.file_name} draggable="false" />
        {#if p.id === bestId}<span
            class="badge"
            data-testid="series-ribbon-best">Best</span
          >{/if}
        {#if p.flag === "pick"}<span class="mark pick" aria-label="Pick">✓</span
          >{/if}
      </button>
    {/each}
  </div>
</nav>

<style>
  .ribbon {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    display: flex;
    align-items: center;
    gap: 0.8rem;
    height: var(--ribbon-height);
    padding: 0 1rem;
    background: rgb(23 23 22 / 0.95);
    border-top: 1px solid #2a2a27;
    box-sizing: border-box;
  }

  .count {
    flex-shrink: 0;
    color: #9a9a94;
    font-size: 0.85rem;
  }

  .strip {
    display: flex;
    gap: 0.4rem;
    overflow-x: auto;
    padding: 0.3rem 0;
    min-width: 0;
  }

  .frame {
    position: relative;
    flex-shrink: 0;
    height: calc(var(--ribbon-height) - 1.2rem);
    aspect-ratio: 3 / 2;
    padding: 0;
    border: 2px solid transparent;
    border-radius: 4px;
    background: #0d0d0c;
    overflow: hidden;
    cursor: pointer;
  }

  .frame:disabled {
    cursor: default;
    opacity: 0.45;
  }

  .frame.current {
    border-color: #fff;
  }

  .frame.rejected img {
    opacity: 0.35;
  }

  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .badge {
    position: absolute;
    left: 2px;
    top: 2px;
    padding: 0 0.3rem;
    border-radius: 3px;
    background: var(--accent);
    color: var(--accent-fg);
    font-size: 0.65rem;
    font-weight: 700;
  }

  .mark {
    position: absolute;
    right: 3px;
    bottom: 2px;
    font-size: 0.8rem;
    font-weight: 700;
    color: #8ee3ad;
  }
</style>
