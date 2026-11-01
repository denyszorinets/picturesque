<script lang="ts">
  import type { CritiqueView, Fixable, Issue } from "../api";

  interface Props {
    critique: CritiqueView;
  }

  let { critique }: Props = $props();

  const ISSUES: Record<Issue, string> = {
    exposure: "Exposure",
    focus: "Focus",
    composition: "Composition",
    noise: "Noise",
    colour: "Colour",
    horizon: "Horizon",
    light: "Light",
    timing: "Timing",
    other: "Other",
    overexposed: "Overexposed",
    underexposed: "Underexposed",
    missed_focus: "Missed focus",
    motion_blur: "Motion blur",
    subject_placement: "Subject placement",
    subject_cut_off: "Subject cut off",
    busy_background: "Busy background",
  };

  const FIXABLE: Record<Fixable, string> = {
    not_needed: "Nothing to fix in editing",
    easy: "Easy to improve with editing",
    needs_skill: "Can be improved with editing skill",
    not_fixable: "Not fixable in editing",
  };

  const reach = $derived(
    critique.potential !== null && critique.potential > critique.rating + 0.4
      ? critique.potential
      : null,
  );
  const hasRaw = $derived(
    critique.fixable !== null || critique.fix_steps.length > 0,
  );
  const hasAdvice = $derived(
    critique.improvements.length > 0 ||
      critique.camera_tips.length > 0 ||
      hasRaw,
  );
</script>

{#if critique.issues.length > 0}
  <ul class="tags" data-testid="critique-issues" aria-label="Issues">
    {#each critique.issues as issue (issue)}
      <li>{ISSUES[issue]}</li>
    {/each}
  </ul>
{/if}
{#if hasAdvice}
  <details class="advice" data-testid="critique-advice">
    <summary>Advice</summary>
    {#if critique.improvements.length > 0}
      <h4>How to improve</h4>
      <ul data-testid="critique-improvements">
        {#each critique.improvements as point, i (i)}
          <li>{point}</li>
        {/each}
      </ul>
    {/if}
    {#if critique.camera_tips.length > 0}
      <h4>Camera settings</h4>
      <ul data-testid="critique-camera">
        {#each critique.camera_tips as point, i (i)}
          <li>{point}</li>
        {/each}
      </ul>
    {/if}
    {#if hasRaw}
      <div data-testid="critique-raw">
        <h4>Editing</h4>
        {#if critique.fixable}
          <p>
            {FIXABLE[critique.fixable]}{#if reach !== null}
              <span data-testid="critique-potential"
                >, could reach {reach.toFixed(1)}/10</span
              >{/if}
          </p>
        {/if}
        {#if critique.fix_steps.length > 0}
          <ul data-testid="critique-fix-steps">
            {#each critique.fix_steps as point, i (i)}
              <li>{point}</li>
            {/each}
          </ul>
        {/if}
      </div>
    {/if}
  </details>
{/if}

<style>
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    list-style: none;
    margin: 0 0 0.4rem;
    padding: 0;
  }

  .tags li {
    padding: 0.05rem 0.5rem;
    border: 1px solid var(--border, #444);
    border-radius: 999px;
    font-size: 0.75rem;
    color: var(--muted);
  }

  .advice {
    margin: 0 0 0.4rem;
  }

  summary {
    cursor: pointer;
    color: var(--muted);
    font-size: 0.85rem;
  }

  h4 {
    margin: 0.5rem 0 0.2rem;
    font-size: 0.8rem;
    color: var(--muted);
  }

  .advice ul {
    margin: 0;
    padding-left: 1.2rem;
  }

  .advice p {
    margin: 0;
  }
</style>
