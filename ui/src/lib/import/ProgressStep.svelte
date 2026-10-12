<script lang="ts">
  import type { ImportFlow } from "./flow.svelte";

  interface Props {
    flow: ImportFlow;
  }

  let { flow }: Props = $props();

  const label = $derived.by(() => {
    const s = flow.status;
    switch (s?.phase) {
      case "scanning":
        return "Looking for photos…";
      case "reading":
        return `Reading photos: ${s.done} of ${s.total}`;
      case "copying":
        return `Copying photos: ${s.done} of ${s.total}`;
      default:
        return flow.step === "planning" ? "Preparing…" : "Starting…";
    }
  });
</script>

<div class="progress" aria-live="polite">
  <p data-testid="progress-label">{label}</p>
  <div
    class="bar"
    role="progressbar"
    aria-valuemin="0"
    aria-valuemax="100"
    aria-valuenow={flow.fraction === null
      ? undefined
      : Math.round(flow.fraction * 100)}
  >
    <div
      class="fill"
      class:indeterminate={flow.fraction === null}
      style:width={`${(flow.fraction ?? 0.3) * 100}%`}
    ></div>
  </div>
  <div>
    <button
      onclick={() => flow.cancel()}
      disabled={flow.busy}
      data-testid="cancel-import">Cancel</button
    >
    {#if flow.step === "copying"}
      <span class="muted"
        >Stops after the photo being copied; you can resume later.</span
      >
    {/if}
  </div>
</div>

<style>
  .progress {
    display: flex;
    flex-direction: column;
    gap: 0.8rem;
  }

  p {
    margin: 0;
  }

  .bar {
    height: 10px;
    background: var(--surface-2);
    border-radius: 999px;
    overflow: hidden;
  }

  .fill {
    height: 100%;
    background: var(--accent);
    border-radius: 999px;
    transition: width 0.3s ease;
  }

  .fill.indeterminate {
    animation: slide 1.4s ease-in-out infinite;
  }

  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(340%);
    }
  }
</style>
