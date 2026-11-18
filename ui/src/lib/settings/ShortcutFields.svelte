<script lang="ts">
  import {
    ACTIONS,
    DEFAULTS,
    ONE_HAND,
    bindingError,
    type Bindings,
  } from "../shortcuts/bindings";
  import { shortcuts, saveBindings } from "../shortcuts/preferences.svelte";

  let draft = $state<Bindings>({ ...shortcuts.bindings });
  let notice = $state<string | null>(null);
  let storageError = $state<string | null>(null);
  const error = $derived(bindingError(draft));
  const dirty = $derived(
    ACTIONS.some(
      ({ id }) => draft[id].trim().toLowerCase() !== shortcuts.bindings[id],
    ),
  );

  function save() {
    storageError = saveBindings(draft);
    notice = storageError ? null : "Shortcuts saved.";
    if (!storageError) draft = { ...shortcuts.bindings };
  }

  function preset(bindings: Bindings) {
    draft = { ...bindings };
    notice = null;
    storageError = null;
  }
</script>

<fieldset class="shortcuts" data-testid="shortcut-settings">
  <legend>Keyboard shortcuts</legend>
  <p class="muted">
    Choose one letter for each action. Shift still advances after a decision.
    Arrow keys, ratings, Undo/Redo, Enter, Escape, / and ? keep their usual
    keys.
  </p>
  <div class="presets">
    <button class="small" onclick={() => preset(ONE_HAND)}
      >One-hand culling (A / D / F)</button
    >
    <button class="small" onclick={() => preset(DEFAULTS)}
      >Restore default shortcuts</button
    >
  </div>
  <div class="keys">
    {#each ACTIONS as action (action.id)}
      <label>
        <span>{action.label}</span>
        <input
          aria-label={`${action.label} shortcut`}
          type="text"
          maxlength="1"
          autocomplete="off"
          spellcheck="false"
          bind:value={draft[action.id]}
          oninput={() => {
            notice = null;
            storageError = null;
          }}
        />
      </label>
    {/each}
  </div>
  {#if error || storageError}<p class="error" role="alert">
      {error || storageError}
    </p>{/if}
  <div class="presets">
    <button class="primary" disabled={!dirty || !!error} onclick={save}
      >Save shortcuts</button
    >
    {#if notice}<span role="status">{notice}</span>{/if}
  </div>
  <p class="muted">
    Saved separately on this device, for all libraries and folders. Help and
    button hints show your chosen keys.
  </p>
</fieldset>

<style>
  .shortcuts {
    display: flex;
    flex-direction: column;
    gap: 0.8rem;
  }
  .shortcuts p {
    margin: 0;
    font-size: 0.85rem;
  }
  .presets {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.6rem;
  }
  .keys {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(15rem, 1fr));
    gap: 0.5rem 1.5rem;
  }
  .keys label {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }
  .keys input {
    width: 3rem;
    text-align: center;
    text-transform: uppercase;
  }
</style>
