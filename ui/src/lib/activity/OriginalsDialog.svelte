<script lang="ts">
  import * as api from "../api";
  import type { OriginalsPlan, OriginalsStatus } from "../api";
  import { formatSize } from "../ai/models.svelte";

  interface Props {
    /** The folder whose originals would be deleted. */
    source: { id: number; path: string };
    onclose: () => void;
  }

  let { source, onclose }: Props = $props();

  /** How often progress is read while deleting (events may be missed). */
  const POLL_MS = 500;

  let plan = $state<OriginalsPlan | null>(null);
  let status = $state<OriginalsStatus | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);

  const running = $derived(status?.state === "running");
  const finished = $derived(!!status && status.state !== "running");

  $effect(() => {
    api
      .originalsPlan(source.id)
      .then((p) => (plan = p))
      .catch((e: unknown) => (error = api.ApiError.from(e).message));
  });

  // Follows a run that is going, also one started before the dialog opened.
  $effect(() => {
    if (!running) return;
    const timer = setInterval(() => {
      api
        .originalsStatus()
        .then((s) => (status = s))
        .catch(() => {});
    }, POLL_MS);
    return () => clearInterval(timer);
  });

  async function start() {
    if (!plan) return;
    busy = true;
    error = null;
    try {
      status = await api.deleteOriginals(source.id, plan.through_job_id);
    } catch (e) {
      error = api.ApiError.from(e).message;
    } finally {
      busy = false;
    }
  }

  async function stop() {
    try {
      status = await api.cancelDeleteOriginals();
    } catch (e) {
      error = api.ApiError.from(e).message;
    }
  }

  function plural(n: number, word: string): string {
    return `${n} ${word}${n === 1 ? "" : "s"}`;
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && !running && !busy) {
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
    aria-label="Delete originals"
    data-testid="originals-dialog"
  >
    <h2>Delete originals</h2>
    {#if finished && status}
      <p data-testid="originals-result">
        {status.state === "cancelled" ? "Stopped. " : ""}Deleted the originals
        of
        {plural(status.deleted, "photo")} ({plural(status.files, "file")},
        {formatSize(status.bytes)}). Your library was not changed.
        {#if status.gone > 0}
          {plural(status.gone, "photo")} had no originals left.
        {/if}
      </p>
      {#if status.kept.length > 0}
        <div class="error-text" data-testid="originals-kept">
          <p>{plural(status.kept.length, "photo")} kept:</p>
          <ul>
            {#each status.kept.slice(0, 5) as kept (kept.photo_id)}
              <li>{kept.reason}</li>
            {/each}
            {#if status.kept.length > 5}
              <li>and {status.kept.length - 5} more</li>
            {/if}
          </ul>
        </div>
      {/if}
      <div class="actions">
        <button class="primary" onclick={onclose}>Done</button>
      </div>
    {:else if running && status}
      <p data-testid="originals-progress">
        Checking and deleting… {status.done} of {status.total}
      </p>
      <progress max={Math.max(status.total, 1)} value={status.done}></progress>
      <div class="actions">
        <button onclick={() => void stop()} data-testid="originals-stop"
          >Stop</button
        >
      </div>
    {:else if plan}
      {#if plan.photos === 0}
        <p data-testid="originals-nothing">
          Nothing to delete{plan.changed > 0
            ? `: ${plural(plan.changed, "photo")} changed since they were imported and stay`
            : ""}.
        </p>
      {:else}
        <p data-testid="originals-summary">
          Deletes the originals of {plural(plan.photos, "photo")} ({plural(
            plan.files,
            "file",
          )}, {formatSize(plan.bytes)}) from <code>{source.path}</code>:
        </p>
        <ul>
          <li>
            Each photo is checked first: its original files must be unchanged
            since the import and match the copies in your library, byte for
            byte, and those copies must be intact. Sidecars (XMP) must be
            unchanged since the import.
          </li>
          {#if plan.duplicates > 0}
            <li>
              {plural(plan.duplicates, "photo")} were already in your library; their
              originals go once an identical file is found there.
            </li>
          {/if}
          {#if plan.changed > 0}
            <li>
              {plural(plan.changed, "photo")} changed since they were imported and
              stay.
            </li>
          {/if}
          <li>
            <strong>Deleting is permanent</strong>: the files do not go to the
            trash. Your library keeps its copies.
          </li>
        </ul>
      {/if}
      <div class="actions">
        <button onclick={onclose} disabled={busy}>Cancel</button>
        {#if plan.photos > 0}
          <button
            class="danger"
            onclick={() => void start()}
            disabled={busy}
            data-testid="originals-go"
            >{busy
              ? "Starting…"
              : `Delete ${plural(plan.photos, "original")}`}</button
          >
        {/if}
      </div>
    {:else if !error}
      <p class="muted">Loading…</p>
    {/if}
    {#if error}
      <p class="error-text" role="alert">{error}</p>
      {#if !plan}
        <div class="actions"><button onclick={onclose}>Close</button></div>
      {/if}
    {/if}
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

  progress {
    width: 100%;
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
</style>
