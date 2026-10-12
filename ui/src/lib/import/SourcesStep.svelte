<script lang="ts">
  import PathInput from "../components/PathInput.svelte";
  import type { ImportFlow } from "./flow.svelte";

  interface Props {
    flow: ImportFlow;
    /** Whether a library folder is set; planning needs one. */
    hasLibrary: boolean;
  }

  let { flow, hasLibrary }: Props = $props();
  let draft = $state("");

  function add(path: string) {
    flow.addSource(path);
    draft = "";
  }
</script>

<div class="sources">
  <p class="muted">
    Choose the folders to import from. Subfolders are included; the library
    folder itself is skipped.
  </p>

  {#if flow.sources.length > 0}
    <ul class="list" data-testid="sources">
      {#each flow.sources as source (source)}
        <li>
          <code>{source}</code>
          <button
            class="remove"
            aria-label={`Remove ${source}`}
            onclick={() => flow.removeSource(source)}>✕</button
          >
        </li>
      {/each}
    </ul>
  {/if}

  <div class="add">
    <PathInput
      label="Folder to import from"
      bind:value={draft}
      multiple
      onpick={(paths) => paths.forEach(add)}
      onsubmit={add}
      testid="source-path"
    />
    <button
      onclick={() => add(draft)}
      disabled={!draft.trim()}
      data-testid="add-source">Add</button
    >
  </div>

  <div class="actions">
    <button
      class="primary"
      onclick={() => flow.startPlanning()}
      disabled={flow.sources.length === 0 || !hasLibrary || flow.busy}
      data-testid="plan-import"
    >
      Plan import
    </button>
    {#if !hasLibrary}
      <span class="muted">Set a library folder first.</span>
    {/if}
  </div>
</div>

<style>
  .sources {
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
  }

  p {
    margin: 0;
  }

  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .list li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    padding: 0.4rem 0.6rem;
    background: var(--surface-2);
    border-radius: 8px;
    overflow-wrap: anywhere;
  }

  .remove {
    border: none;
    background: none;
    padding: 0.1rem 0.4rem;
    color: var(--muted);
  }

  .add {
    display: flex;
    gap: 0.5rem;
  }

  .add :global(.path-input) {
    flex: 1;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 0.8rem;
  }
</style>
