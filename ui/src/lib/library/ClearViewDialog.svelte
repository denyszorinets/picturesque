<script lang="ts">
  import * as api from "../api";

  interface Props {
    /** The photos in view (the filters), or `null` when nothing filters. */
    query: api.PhotoQuery | null;
    /** How many photos the view shows; `null` while stacked (the view
     * counts stacks, not photos). */
    count: number | null;
    /** Whether a folder is culled in place (else it is the library). */
    inFolder: boolean;
    onclose: () => void;
    /** Called after photos were removed, so the view can refresh. */
    oncleared?: (removed: number) => void;
  }

  let { query, count, inFolder, onclose, oncleared }: Props = $props();

  let busy = $state(false);
  let error = $state<string | null>(null);

  const what = $derived(
    query
      ? count === null
        ? "the photos matching the filters"
        : `the ${count === 1 ? "photo" : `${count} photos`} matching the filters`
      : "every photo",
  );

  async function clear() {
    busy = true;
    error = null;
    try {
      const result = await api.clearView(query ?? {});
      oncleared?.(result.removed);
      onclose();
    } catch (e) {
      error = api.ApiError.from(e).message;
    } finally {
      busy = false;
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && !busy) {
      e.preventDefault();
      e.stopPropagation();
      onclose();
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="backdrop" role="presentation">
  <div
    class="dialog card"
    role="dialog"
    aria-modal="true"
    aria-label="Clear view"
    data-testid="clear-view-dialog"
  >
    <h2>Clear view</h2>
    <p data-testid="clear-view-summary">
      Removes {what} from Picturesque. <strong>No file is deleted</strong>: they
      stay {inFolder ? "in this folder" : "in your library folder"}, with their
      sidecars.
    </p>
    <ul>
      <li>
        Their picks, ratings and scores in Picturesque are forgotten; what is
        already in their sidecars stays there.
      </li>
      <li>
        They do not come back while their files are unchanged. New photos{inFolder
          ? " on the card"
          : " in your import folders"} appear as usual.
      </li>
      {#if inFolder}
        <li class="muted">
          Photos whose files are gone (a wiped card) leave the view by
          themselves whenever the folder is checked.
        </li>
      {/if}
    </ul>
    {#if query}
      <p class="muted small">
        To clear everything, clear the filters first. Stacked series count with
        all their photos.
      </p>
    {/if}
    <div class="actions">
      <button onclick={onclose} disabled={busy}>Cancel</button>
      <button
        class="primary"
        onclick={() => void clear()}
        disabled={busy}
        data-testid="clear-view-go"
        >{busy
          ? "Clearing…"
          : query
            ? "Clear these photos"
            : "Clear view"}</button
      >
    </div>
    {#if error}<p class="error-text" role="alert">{error}</p>{/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 20;
    display: grid;
    place-items: center;
    background: var(--scrim);
  }

  .dialog {
    width: min(32rem, calc(100vw - 2rem));
    display: flex;
    flex-direction: column;
    gap: 0.8rem;
    background: var(--bg);
  }

  h2 {
    font-size: 1.1rem;
  }

  p {
    margin: 0;
  }

  ul {
    margin: 0;
    padding-left: 1.1rem;
  }

  li + li {
    margin-top: 0.3rem;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }

  .small {
    font-size: 0.85rem;
  }
</style>
