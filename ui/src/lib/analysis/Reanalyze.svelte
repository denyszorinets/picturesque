<script lang="ts">
  import { ApiError, type ReanalyzeResult } from "../api";

  interface Props {
    /** Queues photos for analysis again (all of them without ids). */
    reanalyze: (photoIds?: number[]) => Promise<ReanalyzeResult>;
    /** The photos to analyze again; every photo when absent. */
    photoIds?: number[];
    label: string;
    title?: string;
    disabled?: boolean;
    /** Ask before starting (for the whole library). */
    confirm?: string;
    /** Smaller button without a message, for toolbars (the toolbar shows
     * analysis progress). */
    small?: boolean;
  }

  let {
    reanalyze,
    photoIds,
    label,
    title,
    disabled = false,
    confirm,
    small = false,
  }: Props = $props();

  let asking = $state(false);
  let busy = $state(false);
  let message = $state<string | null>(null);
  let error = $state<string | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;

  /** How long the confirmation stays. */
  const MESSAGE_MS = 5000;

  async function start() {
    asking = false;
    busy = true;
    error = null;
    try {
      const { photos } = await reanalyze(photoIds);
      message =
        photos === 0
          ? "No photos to analyze."
          : `Analyzing ${photos} ${photos === 1 ? "photo" : "photos"} again.`;
      clearTimeout(timer);
      timer = setTimeout(() => (message = null), MESSAGE_MS);
    } catch (e) {
      error = ApiError.from(e).message;
    } finally {
      busy = false;
    }
  }

  $effect(() => () => clearTimeout(timer));
</script>

<span class="reanalyze">
  {#if asking}
    <span class="question" data-testid="reanalyze-confirm">{confirm}</span>
    <button
      class="primary"
      class:small
      onclick={start}
      data-testid="reanalyze-start">Re-analyze</button
    >
    <button class:small onclick={() => (asking = false)}>Cancel</button>
  {:else}
    <button
      class:small
      onclick={() => (confirm ? (asking = true) : start())}
      disabled={disabled || busy}
      {title}
      data-testid="reanalyze">{label}</button
    >
  {/if}
  {#if error}
    <span class="error-text" role="alert">{error}</span>
  {:else if message && !small}
    <span class="muted" role="status" data-testid="reanalyze-message"
      >{message}</span
    >
  {/if}
</span>

<style>
  .reanalyze {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.85rem;
  }

  .small {
    padding: 0.2rem 0.6rem;
    font-size: 0.85rem;
  }

  .question {
    flex-basis: 100%;
  }
</style>
