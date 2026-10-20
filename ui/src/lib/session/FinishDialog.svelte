<script lang="ts">
  import * as api from "../api";
  import type { ColorLabel, FinishResult, FinishStatus } from "../api";

  interface Props {
    /** The auto-pick label from the settings; `null` writes none. */
    autoLabel: ColorLabel | null;
    onclose: () => void;
    /** Next steps after finishing. */
    onexport?: () => void;
    onnextfolder?: () => void;
  }

  let { autoLabel, onclose, onexport, onnextfolder }: Props = $props();

  let status = $state<FinishStatus | null>(null);
  let result = $state<FinishResult | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);

  $effect(() => {
    api
      .finishStatus()
      .then((s) => (status = s))
      .catch((e: unknown) => (error = api.ApiError.from(e).message));
  });

  async function finish() {
    busy = true;
    error = null;
    try {
      result = await api.finishFolder();
    } catch (e) {
      error = api.ApiError.from(e).message;
    } finally {
      busy = false;
    }
  }

  function plural(n: number, word: string): string {
    return `${n} ${word}${n === 1 ? "" : "s"}`;
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
    aria-label="Finish folder"
    data-testid="finish-dialog"
  >
    <h2>Finish folder</h2>
    {#if result}
      <p data-testid="finish-result">
        {#if result.failed > 0}
          <span class="error-text"
            >{plural(result.failed, "sidecar")} could not be written: {result.error}</span
          >
        {:else}
          Done: every decision is in the XMP sidecars{autoLabel &&
          result.auto_labeled > 0
            ? `, and ${plural(result.auto_labeled, "auto-picked photo")} ${result.auto_labeled === 1 ? "has" : "have"} the ${autoLabel} label`
            : ""}.
        {/if}
      </p>
      <p class="muted">What next?</p>
      <div class="next">
        {#if onexport}
          <button
            onclick={() => {
              onclose();
              onexport?.();
            }}
            data-testid="finish-export">Export keepers…</button
          >
        {/if}
        {#if onnextfolder}
          <button
            onclick={() => {
              onclose();
              onnextfolder?.();
            }}
            data-testid="finish-next-folder">Open the next folder…</button
          >
        {/if}
        <button class="primary" onclick={onclose}>Done</button>
      </div>
      {#if status && status.rejects > 0}
        <p class="muted small">
          {plural(status.rejects, "rejected photo")} stay where they are; use
          <strong>Move rejects…</strong> in the toolbar to move them aside.
        </p>
      {/if}
    {:else if status}
      <ul data-testid="finish-summary">
        <li>
          {plural(status.picks, "pick")}, {plural(status.rejects, "reject")}
        </li>
        <li>
          {plural(status.auto_picked, "auto-picked photo")}
          {#if autoLabel && status.auto_picked > 0}
            get the <strong>{autoLabel}</strong> colour label, so darktable and other
            tools show them
          {:else if !autoLabel}
            (no label: none is chosen in Settings)
          {/if}
        </li>
        <li>Every decision not yet in its sidecar is written now.</li>
      </ul>
      <div class="next">
        <button onclick={onclose} disabled={busy}>Cancel</button>
        <button
          class="primary"
          onclick={() => void finish()}
          disabled={busy}
          data-testid="finish-go">{busy ? "Finishing…" : "Finish"}</button
        >
      </div>
    {:else if !error}
      <p class="muted">Loading…</p>
    {/if}
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

  ul {
    margin: 0;
    padding-left: 1.1rem;
  }

  li + li {
    margin-top: 0.3rem;
  }

  .next {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 0.5rem;
  }

  .small {
    font-size: 0.85rem;
  }
</style>
