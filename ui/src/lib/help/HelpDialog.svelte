<script lang="ts">
  import ExternalLink from "../components/ExternalLink.svelte";
  import { GUIDES, SHORTCUTS } from "./shortcuts";

  interface Props {
    onclose: () => void;
  }

  let { onclose }: Props = $props();

  // Captured first, so dialogs below (the setup wizard) do not also close.
  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape" || e.key === "?") {
      e.preventDefault();
      e.stopImmediatePropagation();
      onclose();
    }
  }
</script>

<svelte:window onkeydowncapture={onkeydown} />

<div class="backdrop" role="presentation">
  <div
    class="dialog card"
    role="dialog"
    aria-modal="true"
    aria-labelledby="help-title"
    data-testid="help-dialog"
  >
    <header>
      <h2 id="help-title">Help</h2>
      <button class="close" aria-label="Close" onclick={onclose}>✕</button>
    </header>

    <nav class="guides" aria-label="Guides">
      {#each GUIDES as guide (guide.url)}
        <ExternalLink href={guide.url}>{guide.label}</ExternalLink>
      {/each}
    </nav>

    <h3 class="section">Keyboard shortcuts</h3>
    <div class="groups">
      {#each SHORTCUTS as group (group.title)}
        <section data-testid={`shortcuts-${group.title}`}>
          <h4>{group.title}</h4>
          {#if group.note}<p class="muted note">{group.note}</p>{/if}
          <dl>
            {#each group.items as item (item.keys.join("+") + item.action)}
              <dt>
                {#each item.keys as key, i (i)}
                  {#if i > 0}<span class="plus">+</span>{/if}<kbd>{key}</kbd>
                {/each}
              </dt>
              <dd>{item.action}</dd>
            {/each}
          </dl>
        </section>
      {/each}
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 30;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 0.4);
  }

  .dialog {
    width: min(52rem, calc(100vw - 2rem));
    max-height: calc(100vh - 2rem);
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.8rem;
    background: var(--bg);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  h2 {
    font-size: 1.2rem;
  }

  .close {
    border: none;
    background: none;
    color: var(--muted);
  }

  .guides {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem 1.2rem;
  }

  .section {
    font-size: 1rem;
  }

  .groups {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(15rem, 1fr));
    gap: 1rem 1.5rem;
  }

  h4 {
    font-size: 0.9rem;
    margin-bottom: 0.3rem;
  }

  .note {
    margin: 0 0 0.3rem;
    font-size: 0.8rem;
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.3rem 0.7rem;
    margin: 0;
    font-size: 0.85rem;
  }

  dt {
    white-space: nowrap;
  }

  dd {
    margin: 0;
  }

  kbd {
    display: inline-block;
    min-width: 1.4em;
    padding: 0 0.35em;
    border: 1px solid var(--border);
    border-bottom-width: 2px;
    border-radius: 4px;
    background: var(--surface-2);
    font-family: inherit;
    font-size: 0.8rem;
    text-align: center;
  }

  .plus {
    margin: 0 0.15em;
    color: var(--muted);
  }
</style>
