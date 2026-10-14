<script lang="ts">
  import * as api from "../api";
  import type { RejectMovePlan, RejectMoveResult } from "../api";
  import { formatSize } from "../ai/models.svelte";

  interface Props {
    /** Rejected photos in the library (from the facets). */
    rejected: number;
    /** Called after files moved, so the view can refresh. */
    onmoved?: () => void;
  }

  let { rejected, onmoved }: Props = $props();

  let plan = $state<RejectMovePlan | null>(null);
  let result = $state<RejectMoveResult | null>(null);
  let undone = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);

  async function run<T>(call: () => Promise<T>): Promise<T | undefined> {
    busy = true;
    error = null;
    try {
      return await call();
    } catch (e) {
      error = api.ApiError.from(e).message;
    } finally {
      busy = false;
    }
  }

  async function ask() {
    result = null;
    undone = false;
    plan = (await run(api.planRejectMove)) ?? null;
  }

  async function move() {
    const moved = await run(api.moveRejects);
    plan = null;
    if (moved) {
      result = moved;
      onmoved?.();
    }
  }

  async function undo() {
    if (result?.batch == null) return;
    const back = await run(() => api.undoRejectMove(result!.batch!));
    if (back) {
      undone = true;
      onmoved?.();
    }
  }
</script>

{#if plan}
  <span class="confirm" data-testid="rejects-confirm">
    {#if plan.photos === 0}
      No rejected photos to move.
      <button class="small" onclick={() => (plan = null)}>OK</button>
    {:else}
      Move {plan.photos} rejected {plan.photos === 1 ? "photo" : "photos"}
      ({plan.files} files, {formatSize(plan.bytes)}) to <code>_Rejected</code>?
      <button
        class="small primary"
        onclick={move}
        disabled={busy}
        data-testid="rejects-move">Move</button
      >
      <button class="small" onclick={() => (plan = null)}>Cancel</button>
    {/if}
  </span>
{:else if result}
  <span class="confirm" data-testid="rejects-result">
    {#if undone}
      Moved {result.moved} back.
    {:else}
      Moved {result.moved}
      {result.moved === 1 ? "photo" : "photos"} to
      <code>_Rejected</code>{result.failed.length > 0
        ? `; ${result.failed.length} could not be moved and stayed in place`
        : ""}.
      {#if result.batch !== null}
        <button
          class="small"
          onclick={undo}
          disabled={busy}
          data-testid="rejects-undo">Undo</button
        >
      {/if}
    {/if}
    <button
      class="small link"
      onclick={() => (result = null)}
      aria-label="Dismiss">✕</button
    >
  </span>
{:else if rejected > 0}
  <button
    class="small"
    onclick={ask}
    disabled={busy}
    title="Move rejected photos into the library's _Rejected folder"
    data-testid="rejects-open">Move rejects…</button
  >
{/if}
{#if error}
  <span class="error-text" role="alert">{error}</span>
{/if}
{#if result && result.failed.length > 0 && !undone}
  <span class="error-text" title={result.failed.map((f) => f.error).join("\n")}
    >{result.failed[0]!.error}</span
  >
{/if}

<style>
  .confirm {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.2rem 0.6rem;
    border-radius: 6px;
    background: var(--surface-2);
    font-size: 0.85rem;
  }

  .small {
    padding: 0.2rem 0.6rem;
    font-size: 0.85rem;
  }

  .link {
    border: none;
    background: none;
    color: var(--muted);
  }
</style>
