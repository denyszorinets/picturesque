<script lang="ts">
  import type { BackgroundCritiqueStatus, CriticIntensity } from "../api";
  import { formatTimeLeft } from "./duration";
  import { validPercent } from "../settings/percent";

  interface Props {
    background: boolean;
    intensity: CriticIntensity;
    /** Share of critiques in the aesthetic score, in percent. */
    scorePercent: number | null;
    /** `null` while unknown. */
    progress: BackgroundCritiqueStatus | null;
    /** Whether the critic fields differ from the saved settings. */
    unsaved: boolean;
  }

  let {
    background = $bindable(),
    intensity = $bindable(),
    scorePercent = $bindable(),
    progress,
    unsaved,
  }: Props = $props();

  const INTENSITIES: { id: CriticIntensity; label: string }[] = [
    {
      id: "gentle",
      label:
        "Gentle: low priority, half the processor, waits while importing, analyzing or on battery",
    },
    {
      id: "when_idle",
      label:
        "When idle: full speed after a few minutes without using Picturesque",
    },
    { id: "full", label: "Full speed" },
  ];

  const done = $derived(
    progress !== null && progress.missing === 0 && progress.stale === 0,
  );
</script>

<div class="background">
  <label class="inline">
    <input
      type="checkbox"
      bind:checked={background}
      data-testid="critic-background"
    />
    Critique the whole library in the background
  </label>
  <p class="hint muted">
    Slow on purpose: about 10-20 s per photo, so thousands of photos take days.
    It works through your best photos first; rejects are skipped.
  </p>
  {#if background}
    <label class="inline">
      How hard may it work
      <select bind:value={intensity} data-testid="critic-intensity">
        {#each INTENSITIES as option (option.id)}
          <option value={option.id}>{option.label}</option>
        {/each}
      </select>
    </label>
  {/if}
  <label class="inline">
    Critique counts for
    <input
      type="number"
      min="0"
      max="100"
      step="5"
      bind:value={scorePercent}
      data-testid="critic-score-weight"
    />
    % of the aesthetic score
  </label>
  {#if !validPercent(scorePercent)}
    <p class="hint error-text" data-testid="critic-score-weight-error">
      Enter a percentage from 0 to 100.
    </p>
  {/if}
  <p class="hint muted">
    The model's ratings are compared within your library, so a 7 means better
    than most of your critiqued photos.
  </p>
  {#if background}
    <p class="hint muted" data-testid="critic-progress">
      {#if unsaved}
        Save to use these settings.
      {:else if progress && progress.enabled}
        {progress.critiqued} of {progress.total} photos critiqued.
        {#if progress.stale > 0}
          {progress.stale} to redo with the new model or rubric.
        {/if}
        {#if progress.seconds_left !== null && !done}
          {formatTimeLeft(progress.seconds_left)} left.
        {/if}
        {#if done}
          Done: nothing left to critique.
        {:else if progress.paused}
          Paused: {progress.paused}
        {:else if progress.running}
          Critiquing now…
        {/if}
      {/if}
    </p>
  {/if}
</div>

<style>
  .background {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
  }

  .inline {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.9rem;
    flex-wrap: wrap;
  }

  input[type="number"] {
    width: 4.5rem;
  }

  .hint {
    margin: 0;
    font-size: 0.85rem;
  }
</style>
