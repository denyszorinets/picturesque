<script lang="ts">
  import * as api from "../api";
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

  $effect(() => {
    void store.load();
    // New photos appear when an import completes.
    return api.onAppEvent((event) => {
      if (event.type === "import_progress" && event.state === "completed")
        void store.load();
    });
  });
</script>

<section class="library" aria-label="Library">
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
    </div>
    <div class="grid">
      <Timeline {store} {selection} />
    </div>
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
    gap: 1rem;
    padding-bottom: 0.4rem;
    font-size: 0.9rem;
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
