<script lang="ts">
  import { onMount } from "svelte";
  import * as api from "../api";
  let dialog: HTMLDivElement;
  onMount(() => {
    const previous = document.activeElement;
    dialog.focus();
    return () => {
      if (previous instanceof HTMLElement && previous.isConnected)
        previous.focus();
    };
  });
  interface Props {
    query: api.PhotoQuery;
    description: string;
    folder: string;
    onclose: () => void;
    onchanged: () => void;
  }
  let { query, description, folder, onclose, onchanged }: Props = $props();
  let scope = $state("view");
  let plan = $state<api.ReviewPlan | null>(null);
  let busy = $state(false);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let result = $state<number | null>(null);
  let applied = $state(true);
  const dated = $derived(
    Boolean(query.from && query.to && query.from <= query.to),
  );
  $effect(() => {
    const target =
      scope === "dates" ? { from: query.from, to: query.to } : query;
    let cancelled = false;
    loading = true;
    plan = null;
    error = null;
    void api
      .reviewPlan(target)
      .then((value) => {
        if (!cancelled) plan = value;
      })
      .catch((e: unknown) => {
        if (!cancelled) error = api.ApiError.from(e).message;
      })
      .finally(() => {
        if (!cancelled) loading = false;
      });
    return () => {
      cancelled = true;
    };
  });
  async function apply(reviewed: boolean) {
    if (!plan || busy || loading) return;
    busy = true;
    error = null;
    try {
      result = await api.applyReview(plan.token, reviewed);
      applied = reviewed;
      onchanged();
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
    if (e.key !== "Tab") return;
    const controls = [
      ...dialog.querySelectorAll<HTMLElement>(
        'button:not(:disabled), input:not(:disabled), select:not(:disabled), [tabindex="0"]',
      ),
    ];
    const first = controls[0],
      last = controls.at(-1);
    if (
      e.shiftKey &&
      (document.activeElement === first || document.activeElement === dialog)
    ) {
      e.preventDefault();
      last?.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first?.focus();
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="backdrop" role="presentation">
  <div
    class="dialog card"
    bind:this={dialog}
    role="dialog"
    aria-modal="true"
    aria-label="Review filtered photos"
    tabindex="-1"
  >
    <h2>Review filtered photos</h2>
    <p class="folder">{folder}</p>
    {#if result !== null}
      <p role="status">
        {result} photos marked {applied ? "reviewed" : "not reviewed"}.
      </p>
      <p>
        Choose “{applied ? "Not reviewed" : "Reviewed"}” in Review progress to
        continue. You can reverse this status with the same action.
      </p>
      <button class="primary" onclick={onclose}>Done</button>
    {:else}
      <fieldset disabled={busy}>
        <legend>Photos to mark</legend>
        <label
          ><input type="radio" bind:group={scope} value="view" /> Current filtered
          view</label
        >
        <p class="muted">{description}</p>
        {#if dated}
          <label
            ><input type="radio" bind:group={scope} value="dates" /> All photos
            from {query.from} through {query.to}</label
          >
          <p class="muted">
            This option includes photos hidden by flags, AI suggestions, review
            status, other filters and stacked series within these dates.
          </p>
        {/if}
      </fieldset>
      {#if loading}<p>Counting photos…</p>
      {:else if plan}
        <p data-testid="review-count">
          <strong>{plan.count} photos</strong> in this folder/library will be marked.
          RAW+JPEG pairs count as one photo.
        </p>
        {#if plan.first_day}<p>
            Capture dates: {plan.first_day} – {plan.last_day}
          </p>{/if}
        {#if plan.undated}<p>
            {plan.undated} photos without a capture date.
          </p>{/if}
      {/if}
      <p>
        Review progress is saved separately. Picks, AI suggestions, ratings,
        exports and XMP stay unchanged. Folder catalogs with reviewed photos are
        retained beyond 30 days.
      </p>
      <div class="actions">
        <button disabled={busy} onclick={onclose}>Cancel</button>
        <button
          disabled={busy || loading || !plan?.count}
          onclick={() => void apply(false)}>Mark not reviewed</button
        >
        <button
          class="primary"
          disabled={busy || loading || !plan?.count}
          onclick={() => void apply(true)}>Mark reviewed</button
        >
      </div>
    {/if}
    {#if error}<p role="alert" class="error-text">{error}</p>{/if}
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
    width: min(38rem, calc(100vw - 2rem));
    max-height: calc(100vh - 2rem);
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    background: var(--bg);
  }
  h2 {
    font-size: 1.1rem;
  }
  p {
    margin: 0;
  }
  .folder {
    overflow-wrap: anywhere;
    font-weight: 600;
  }
  fieldset {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  label {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 0.5rem;
  }
</style>
