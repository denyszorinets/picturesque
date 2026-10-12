<script lang="ts">
  import * as api from "../api";
  import PlanTable from "./PlanTable.svelte";
  import ProgressStep from "./ProgressStep.svelte";
  import SourcesStep from "./SourcesStep.svelte";
  import { ImportFlow } from "./flow.svelte";

  interface Props {
    hasLibrary: boolean;
    flow?: ImportFlow;
  }

  let { hasLibrary, flow = new ImportFlow(api) }: Props = $props();

  // Progress events, with polling as a fallback while a phase runs.
  $effect(() => {
    const stop = api.onAppEvent((event) => {
      if (event.type === "import_progress") void flow.onStatus(event);
    });
    const timer = setInterval(() => void flow.refresh(), 1000);
    return () => {
      stop();
      clearInterval(timer);
    };
  });

  function bytes(n: number): string {
    const units = ["B", "KB", "MB", "GB", "TB"];
    let i = 0;
    while (n >= 1000 && i < units.length - 1) {
      n /= 1000;
      i += 1;
    }
    return `${n.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
  }

  const summary = $derived(flow.plan?.summary);
  const outcome = $derived(flow.plan?.state);
</script>

<section class="card import" aria-labelledby="import-title">
  <h2 id="import-title">Import</h2>

  {#if flow.error}
    <p class="error-text" role="alert" data-testid="import-error">
      {flow.error}
    </p>
  {/if}

  {#if flow.step === "choose"}
    <SourcesStep {flow} {hasLibrary} />
  {:else if flow.step === "planning" || flow.step === "copying"}
    <ProgressStep {flow} />
  {:else if flow.plan && summary}
    <div class="stats" data-testid="import-summary">
      {#if flow.step === "review"}
        <div class="stat">
          <strong>{summary.to_import}</strong><span>to import</span>
        </div>
      {:else}
        <div class="stat good">
          <strong>{summary.imported}</strong><span>imported</span>
        </div>
        {#if summary.to_import > 0}
          <div class="stat">
            <strong>{summary.to_import}</strong><span>not yet copied</span>
          </div>
        {/if}
      {/if}
      <div class="stat">
        <strong>{summary.duplicates}</strong><span>duplicates skipped</span>
      </div>
      <div class="stat" class:bad={summary.failed > 0}>
        <strong>{summary.failed}</strong><span>failed</span>
      </div>
      <div class="stat">
        <strong>{bytes(summary.bytes)}</strong><span>{summary.files} files</span
        >
      </div>
    </div>

    {#if flow.step === "done"}
      <p data-testid="import-outcome">
        {#if outcome === "completed"}
          Import finished. Photos are in <code>{flow.plan.library_root}</code>.
        {:else if outcome === "cancelled"}
          Import cancelled. Already imported photos stay; you can resume the
          rest.
        {:else}
          The import stopped: {flow.plan.error}
        {/if}
      </p>
    {/if}

    {#if flow.plan.problems.length > 0}
      <details class="problems">
        <summary
          >{flow.plan.problems.length} problem{flow.plan.problems.length === 1
            ? ""
            : "s"}</summary
        >
        <ul>
          {#each flow.plan.problems as problem (problem.path + problem.reason)}
            <li><code>{problem.path}</code> — {problem.reason}</li>
          {/each}
        </ul>
      </details>
    {/if}

    <PlanTable plan={flow.plan} />

    <div class="actions">
      {#if flow.step === "review"}
        <button
          class="primary"
          onclick={() => flow.startCopying()}
          disabled={summary.to_import === 0 || flow.busy}
          data-testid="start-import"
        >
          Import {summary.to_import} photo{summary.to_import === 1 ? "" : "s"}
        </button>
        <button onclick={() => flow.reset()}>Back</button>
      {:else}
        {#if summary.to_import > 0 && outcome !== "completed"}
          <button
            class="primary"
            onclick={() => flow.startCopying()}
            disabled={flow.busy}>Resume</button
          >
        {/if}
        <button onclick={() => flow.reset()} data-testid="new-import"
          >New import</button
        >
      {/if}
    </div>
  {/if}
</section>

<style>
  .import {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  p {
    margin: 0;
  }

  .stats {
    display: flex;
    flex-wrap: wrap;
    gap: 0.6rem;
  }

  .stat {
    display: flex;
    flex-direction: column;
    padding: 0.6rem 0.9rem;
    background: var(--surface-2);
    border-radius: 8px;
    min-width: 7rem;
  }

  .stat strong {
    font-size: 1.3rem;
  }

  .stat span {
    color: var(--muted);
    font-size: 0.8rem;
  }

  .stat.good strong {
    color: var(--accent);
  }

  .stat.bad strong {
    color: var(--danger);
  }

  .problems ul {
    margin: 0.5rem 0 0;
    padding-left: 1.2rem;
    font-size: 0.85rem;
    overflow-wrap: anywhere;
  }

  .actions {
    display: flex;
    gap: 0.6rem;
  }
</style>
