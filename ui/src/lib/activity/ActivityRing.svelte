<script lang="ts">
  import type { Ring } from "./ring";

  interface Props {
    ring: Ring | null;
    /** Opens the Activity tab. */
    onactivity?: () => void;
  }

  let { ring, onactivity }: Props = $props();

  let open = $state(false);
  let root: HTMLElement | undefined = $state();

  const RADIUS = 8;
  const CIRCUMFERENCE = 2 * Math.PI * RADIUS;
  /** An unknown amount shows as a spinning quarter. */
  const dash = $derived(
    ring?.progress === null || ring?.progress === undefined
      ? CIRCUMFERENCE * 0.25
      : CIRCUMFERENCE * Math.min(1, Math.max(0, ring.progress)),
  );

  function onwindowclick(e: MouseEvent) {
    // The path at dispatch: a clicked item may be gone from the page by now.
    if (open && root && !e.composedPath().includes(root)) open = false;
  }

  function onkeydown(e: KeyboardEvent) {
    if (open && e.key === "Escape") {
      e.stopPropagation();
      open = false;
    }
  }
</script>

<svelte:window onclick={onwindowclick} />

{#if ring}
  <div class="ring-root" bind:this={root}>
    <button
      class="ring-button {ring.tone}"
      onclick={() => (open = !open)}
      aria-expanded={open}
      aria-haspopup="dialog"
      title={ring.details.join("\n")}
      data-testid="activity-ring"
    >
      <svg
        class="ring"
        class:spinning={ring.progress === null}
        viewBox="0 0 20 20"
        width="18"
        height="18"
        aria-hidden="true"
      >
        <circle class="track" cx="10" cy="10" r={RADIUS} />
        <circle
          class="fill"
          cx="10"
          cy="10"
          r={RADIUS}
          stroke-dasharray={`${dash} ${CIRCUMFERENCE}`}
          transform="rotate(-90 10 10)"
        />
      </svg>
      <span class="label" data-testid="activity-ring-label">{ring.label}</span>
      {#if ring.failed > 0}
        <span class="failed">{ring.failed} failed</span>
      {/if}
    </button>
    {#if open}
      <div
        class="popover"
        role="dialog"
        aria-label="Background work"
        tabindex="-1"
        {onkeydown}
        data-testid="activity-popover"
      >
        <ul>
          {#each ring.details as line (line)}
            <li>{line}</li>
          {/each}
        </ul>
        <button
          class="small"
          onclick={() => {
            open = false;
            onactivity?.();
          }}>Open Activity</button
        >
      </div>
    {/if}
  </div>
{/if}

<style>
  .ring-root {
    position: relative;
  }

  .ring-button {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    padding: 0.25rem 0.7rem 0.25rem 0.45rem;
    border-radius: 999px;
    font-size: 0.85rem;
    background: var(--surface-2);
    border-color: transparent;
    white-space: nowrap;
  }

  .ring-button.paused {
    background: color-mix(in srgb, var(--warn) 18%, transparent);
  }

  .track {
    fill: none;
    stroke: var(--border);
    stroke-width: 2.5;
  }

  .fill {
    fill: none;
    stroke: var(--accent);
    stroke-width: 2.5;
    stroke-linecap: round;
    transition: stroke-dasharray 0.4s ease;
  }

  .paused .fill {
    stroke: var(--warn);
  }

  .spinning {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spinning {
      animation: none;
    }

    .fill {
      transition: none;
    }
  }

  .done .label {
    color: var(--muted);
  }

  .failed {
    color: var(--danger);
    font-weight: 600;
  }

  .popover {
    position: absolute;
    right: 0;
    top: calc(100% + 0.4rem);
    z-index: 20;
    min-width: 18rem;
    max-width: 26rem;
    padding: 0.75rem 0.9rem;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    box-shadow: 0 6px 24px rgb(0 0 0 / 18%);
    font-size: 0.85rem;
  }

  .popover ul {
    margin: 0 0 0.6rem;
    padding-left: 1rem;
  }

  .popover li + li {
    margin-top: 0.3rem;
  }
</style>
