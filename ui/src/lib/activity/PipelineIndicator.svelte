<script lang="ts">
  import { indicatorLabel, type Pipeline } from "./pipeline.svelte";

  interface Props {
    pipeline: Pipeline;
    onclick?: () => void;
  }

  let { pipeline, onclick }: Props = $props();

  const label = $derived(indicatorLabel(pipeline.status));
  const failed = $derived(pipeline.status?.failed ?? 0);
  const title = $derived(
    pipeline.status?.paused ??
      (pipeline.status?.active
        ? `${pipeline.status.active.stage === "reading" ? "Reading" : "Copying"} ${pipeline.status.active.file_name}`
        : "Import activity"),
  );
</script>

{#if label || failed > 0}
  <button
    class="indicator"
    class:paused={!!pipeline.status?.paused}
    {onclick}
    {title}
    data-testid="pipeline-indicator"
  >
    {#if label}
      {#if !pipeline.status?.paused}
        <span class="spinner" aria-hidden="true"></span>
      {/if}
      <span>{label}</span>
    {/if}
    {#if failed > 0}
      <span class="failed">{failed} failed</span>
    {/if}
  </button>
{/if}

<style>
  .indicator {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    padding: 0.25rem 0.7rem;
    border-radius: 999px;
    font-size: 0.85rem;
    background: var(--surface-2);
    border-color: transparent;
    white-space: nowrap;
  }

  .indicator.paused {
    background: color-mix(in srgb, var(--warn) 18%, transparent);
  }

  .spinner {
    width: 0.75rem;
    height: 0.75rem;
    border-radius: 50%;
    border: 2px solid var(--border);
    border-top-color: var(--info);
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spinner {
      animation: none;
    }
  }

  .failed {
    color: var(--danger);
    font-weight: 600;
  }
</style>
