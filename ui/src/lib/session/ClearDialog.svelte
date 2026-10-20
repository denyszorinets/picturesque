<script lang="ts">
  import * as api from "../api";
  import type { ClearPlan, ClearResult } from "../api";
  import { formatSize } from "../ai/models.svelte";

  interface Props {
    /** The folder being cleared, for the confirmation. */
    folder: string;
    onclose: () => void;
    /** Called after photos were deleted, so the view can refresh. */
    oncleared?: () => void;
  }

  let { folder, onclose, oncleared }: Props = $props();

  let plan = $state<ClearPlan | null>(null);
  let result = $state<ClearResult | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  /** Rejects have no copy: deleting them needs its own yes. */
  let rejectsConfirmed = $state(false);

  $effect(() => {
    api
      .clearPlan()
      .then((p) => (plan = p))
      .catch((e: unknown) => (error = api.ApiError.from(e).message));
  });

  const copied = $derived(plan ? plan.photos - plan.rejects : 0);
  const canDelete = $derived(
    !!plan &&
      plan.photos > 0 &&
      plan.mode !== "never" &&
      (plan.rejects === 0 || rejectsConfirmed),
  );

  async function clear() {
    busy = true;
    error = null;
    try {
      result = await api.clearFolder();
      if (result.deleted > 0) oncleared?.();
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
    aria-label="Clear folder"
    data-testid="clear-dialog"
  >
    <h2>Clear folder</h2>
    {#if result}
      <p data-testid="clear-result">
        Deleted {plural(result.deleted, "photo")} ({plural(
          result.files,
          "file",
        )}, {formatSize(result.bytes)}).
      </p>
      {#if result.kept.length > 0}
        <p class="error-text" data-testid="clear-kept">
          {plural(result.kept.length, "photo")} kept: {result.kept[0]!
            .reason}{result.kept.length > 1 ? ", …" : ""}
        </p>
      {/if}
      <div class="actions">
        <button class="primary" onclick={onclose}>Done</button>
      </div>
    {:else if plan}
      {#if plan.mode === "never"}
        <p>
          Deleting originals is turned off in <strong
            >Settings → After copying keepers</strong
          >.
        </p>
      {:else if plan.photos === 0}
        <p data-testid="clear-nothing">
          Nothing to delete: no photo in this folder has a verified copy yet.
          Copy the keepers first.
        </p>
      {:else}
        <p data-testid="clear-summary">
          Deletes {plural(plan.photos, "photo")} ({plural(plan.files, "file")},
          {formatSize(plan.bytes)}) from <code>{folder}</code>:
        </p>
        <ul>
          {#if copied > 0}
            <li>
              {plural(copied, "photo")} you copied; each copy is checked again before
              its original is deleted.
            </li>
          {/if}
          {#if plan.rejects > 0}
            <li class="reject">
              {plural(plan.rejects, "rejected photo")}
              <strong>without a copy</strong>.
            </li>
          {/if}
          <li>Other photos stay. Deleting cannot be undone.</li>
        </ul>
        {#if plan.partly_copied > 0}
          <p class="muted small">
            {plural(plan.partly_copied, "photo")} were copied only in part (for example
            without the RAW file) and stay.
          </p>
        {/if}
        {#if plan.rejects > 0}
          <label class="confirm">
            <input
              type="checkbox"
              bind:checked={rejectsConfirmed}
              data-testid="clear-rejects-confirm"
            />
            Also delete the {plural(plan.rejects, "rejected photo")} for good
          </label>
        {/if}
      {/if}
      <div class="actions">
        <button onclick={onclose} disabled={busy}>Cancel</button>
        {#if plan.mode !== "never" && plan.photos > 0}
          <button
            class="danger"
            onclick={() => void clear()}
            disabled={busy || !canDelete}
            data-testid="clear-go"
            >{busy
              ? "Deleting…"
              : `Delete ${plural(plan.photos, "photo")}`}</button
          >
        {/if}
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

  .reject {
    color: var(--danger);
  }

  .confirm {
    display: flex;
    gap: 0.5rem;
    align-items: center;
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

  .danger:disabled {
    opacity: 0.5;
  }

  .small {
    font-size: 0.85rem;
  }
</style>
