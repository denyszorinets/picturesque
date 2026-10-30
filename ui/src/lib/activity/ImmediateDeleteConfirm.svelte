<script lang="ts">
  interface Props {
    /** Where it applies, for the text: a folder path, or all folders. */
    scope?: string | null;
    onconfirm: () => void;
    oncancel: () => void;
  }

  let { scope = null, onconfirm, oncancel }: Props = $props();

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") oncancel();
  }
</script>

<svelte:window {onkeydown} />

<div class="backdrop" role="presentation">
  <div
    class="dialog card"
    role="dialog"
    aria-modal="true"
    aria-label="Delete originals right after importing"
    data-testid="immediate-dialog"
  >
    <h2>Delete originals right after importing?</h2>
    <p>
      {#if scope}Photos imported from <strong>{scope}</strong>{:else}Photos
        imported from any folder that follows this setting{/if}
      are deleted from there as soon as their copy in your library is verified, one
      by one, while the import is still running. This frees the space as you go.
    </p>
    <ul>
      <li>
        Deleting is <strong>permanent</strong>: there is no trash and no undo.
      </li>
      <li>
        Each original is deleted only if it still matches the verified library
        copy byte for byte, and it has not changed since it was imported.
        Anything else stays and is listed in the import log.
      </li>
      <li>
        Your library disk is the only copy afterwards. Keep a backup of it.
      </li>
    </ul>
    <div class="actions">
      <button onclick={oncancel} data-testid="immediate-cancel">Cancel</button>
      <button class="danger" onclick={onconfirm} data-testid="immediate-confirm"
        >Delete originals right away</button
      >
    </div>
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

  .danger {
    background: var(--danger);
    border-color: var(--danger);
    color: #fff;
  }
</style>
