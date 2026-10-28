<script lang="ts">
  import { ApiError, type CriticStatus, type CritiqueView } from "../api";

  interface Props {
    photoId: number;
    /** `null` while unknown; nothing is shown while critiques are off. */
    status: CriticStatus | null;
    /** The stored critique, if any. */
    load: (photoId: number) => Promise<CritiqueView | null>;
    /** Asks the model (again, with `refresh`). */
    run: (photoId: number, refresh: boolean) => Promise<CritiqueView>;
  }

  let { photoId, status, load, run }: Props = $props();

  let critique = $state<CritiqueView | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);

  $effect(() => {
    const id = photoId;
    critique = null;
    error = null;
    busy = false;
    if (!status?.enabled) return;
    load(id)
      .then((c) => {
        if (id === photoId) critique = c;
      })
      .catch((err: unknown) => {
        if (id === photoId)
          error = `Could not load the critique: ${ApiError.from(err).message}`;
      });
  });

  async function ask(e: Event, refresh: boolean) {
    (e.currentTarget as HTMLElement).blur();
    const id = photoId;
    busy = true;
    error = null;
    try {
      const c = await run(id, refresh);
      if (id === photoId) critique = c;
    } catch (err) {
      if (id === photoId)
        error = err instanceof ApiError ? err.message : String(err);
    } finally {
      if (id === photoId) busy = false;
    }
  }
</script>

{#if status?.enabled}
  <section class="critique" data-testid="critique">
    <h3 class="why">Critique</h3>
    {#if critique}
      <p class="summary" data-testid="critique-summary">
        <strong title="The model's rating, from 1 to 10"
          >{critique.rating.toFixed(1)}/10</strong
        >
        {critique.summary}
      </p>
      {#if critique.strengths.length > 0}
        <ul class="points good" data-testid="critique-strengths">
          {#each critique.strengths as point, i (i)}
            <li>{point}</li>
          {/each}
        </ul>
      {/if}
      {#if critique.weaknesses.length > 0}
        <ul class="points bad" data-testid="critique-weaknesses">
          {#each critique.weaknesses as point, i (i)}
            <li>{point}</li>
          {/each}
        </ul>
      {/if}
      <p class="muted by">
        By {critique.model} ·
        {#if critique.outdated}
          <span data-testid="critique-outdated"
            >Written by an earlier model or prompt ·
          </span>
        {/if}
        <button
          class="link"
          disabled={busy}
          onclick={(e) => void ask(e, true)}
          data-testid="critique-again"
          >{critique.outdated ? "Critique again" : "Ask again"}</button
        >
      </p>
    {:else}
      <button
        disabled={busy}
        onclick={(e) => void ask(e, false)}
        data-testid="critique-run"
        title="Ask {status.model} what works and what does not"
        >{busy ? "Critiquing…" : "Critique this photo"}</button
      >
      {#if busy}
        <p class="muted">Can take a minute on a computer without a GPU.</p>
      {:else if status.problem}
        <p class="muted" data-testid="critique-problem">{status.problem}</p>
      {/if}
    {/if}
    {#if error}
      <p class="error-text" data-testid="critique-error">{error}</p>
    {/if}
  </section>
{/if}

<style>
  .why {
    margin: 1.2rem 0 0.4rem;
    font-size: 0.8rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: #9a9a94;
  }

  .summary {
    margin: 0 0 0.4rem;
  }

  .points {
    margin: 0 0 0.4rem;
    padding-left: 1.2rem;
  }

  .points.good li::marker {
    content: "+ ";
    color: #3fb36b;
  }

  .points.bad li::marker {
    content: "− ";
    color: #ef6a5b;
  }

  .by {
    font-size: 0.8rem;
  }

  .link {
    border: none;
    background: none;
    padding: 0;
    color: var(--muted);
    font-size: inherit;
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .link:hover {
    background: none;
    color: var(--fg);
  }
</style>
