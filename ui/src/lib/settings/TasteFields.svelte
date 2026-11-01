<script lang="ts">
  import type { TasteStatus } from "../api";
  import { validPercent } from "./percent";

  interface Props {
    /** Share of the personal taste score in the aesthetic score, in percent. */
    percent: number | null;
    /** `null` while unknown. */
    taste: TasteStatus | null;
  }

  let { percent = $bindable(), taste }: Props = $props();
</script>

<label class="inline">
  <span class="label">Your taste</span>
  <input
    type="number"
    min="0"
    max="100"
    step="10"
    bind:value={percent}
    data-testid="taste-weight"
  />
  % of the aesthetic score
</label>
{#if !validPercent(percent)}
  <p class="hint error-text" data-testid="taste-weight-error">
    Enter a percentage from 0 to 100.
  </p>
{/if}
<p class="hint muted">
  Picturesque learns what you like from your picks and rejects, and blends it
  with the general aesthetic score: 0% uses only the general score, 100% only
  yours.
</p>
{#if taste}
  <p class="hint muted" data-testid="taste-status">
    {#if taste.learned && taste.agreement !== null}
      Learned from {taste.picks} picks and {taste.rejects} rejects; it scores your
      picks above your rejects {Math.round(taste.agreement * 100)}% of the time.
    {:else}
      Pick and reject at least {taste.needed} analyzed photos each to teach it (now
      {taste.picks}
      {taste.picks === 1 ? "pick" : "picks"} and {taste.rejects}
      {taste.rejects === 1 ? "reject" : "rejects"}).
    {/if}
  </p>
{/if}
