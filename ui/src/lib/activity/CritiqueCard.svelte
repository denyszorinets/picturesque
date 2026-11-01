<script lang="ts">
  import type { BackgroundCritiqueStatus } from "../api";
  import {
    critiqueExtraLines,
    critiqueFailedLine,
    critiqueHeadline,
    critiqueLeft,
  } from "./critique-ring";

  interface Props {
    status: BackgroundCritiqueStatus | null;
    onsettings?: () => void;
  }

  let { status, onsettings }: Props = $props();

  const extra = $derived(status ? critiqueExtraLines(status) : []);
  const failed = $derived(status ? critiqueFailedLine(status) : null);
  const percent = $derived(
    status && status.total > 0
      ? Math.round((status.critiqued / status.total) * 100)
      : 0,
  );
  const phase = $derived(
    !status
      ? ""
      : status.paused && !status.running
        ? "paused"
        : status.running
          ? "running"
          : critiqueLeft(status) > 0
            ? "waiting"
            : "done",
  );
</script>

<section class="card critique" aria-labelledby="critique-title">
  <div class="heading">
    <h2 id="critique-title">Critique</h2>
    {#if status?.enabled}
      <span class="muted" data-testid="activity-critique-state">{phase}</span>
    {/if}
  </div>
  {#if !status?.enabled}
    <p class="muted" data-testid="activity-critique-off">
      The background critic is off.
      {#if onsettings}
        <button class="link" onclick={onsettings}>Open Settings</button>
      {/if}
    </p>
  {:else}
    <p data-testid="activity-critique-count">
      {status.critiqued} / {status.total} photos critiqued
    </p>
    <progress max="100" value={percent} aria-label="Critique progress"
    ></progress>
    <ul data-testid="activity-critique-details">
      {#if status.paused && !status.running}
        <li>{critiqueHeadline(status)}</li>
      {/if}
      {#each extra as line (line)}
        <li>{line}</li>
      {/each}
      {#if failed}
        <li class="error-text">{failed}</li>
      {/if}
    </ul>
  {/if}
</section>

<style>
  section {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  ul {
    margin: 0;
    padding-left: 1rem;
    font-size: 0.85rem;
  }

  progress {
    width: 100%;
  }

  .link {
    border: none;
    background: none;
    padding: 0;
    color: var(--muted);
    text-decoration: underline;
  }
</style>
