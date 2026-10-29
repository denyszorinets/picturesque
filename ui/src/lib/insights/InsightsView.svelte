<script lang="ts">
  import { ApiError, imageUrl, type Issue, type LibraryInsights } from "../api";

  interface Props {
    /** Loads the insights (injectable for tests). */
    load: () => Promise<LibraryInsights>;
    /** Whether the view is on screen; it reloads each time it is shown. */
    visible: boolean;
    /** Goes to the settings, where critiques are switched on. */
    onsettings?: () => void;
    /** The thumbnail of an example photo. */
    imageSrc?: (photoId: number) => string;
  }

  let {
    load,
    visible,
    onsettings,
    imageSrc = (id) => imageUrl(id, "grid"),
  }: Props = $props();
  let insights = $state<LibraryInsights | null>(null);
  let error = $state<string | null>(null);

  /** Counts loads, so a slow older one cannot overwrite a newer one. */
  let latest = 0;

  $effect(() => {
    if (!visible) return;
    const mine = ++latest;
    load()
      .then((i) => {
        if (mine !== latest) return;
        insights = i;
        error = null;
      })
      .catch((e: unknown) => {
        if (mine === latest) error = ApiError.from(e).message;
      });
  });

  const ISSUES: Record<Issue, string> = {
    exposure: "Exposure",
    focus: "Focus",
    composition: "Composition",
    noise: "Noise",
    colour: "Colour",
    horizon: "Horizon",
    light: "Light",
    timing: "Timing",
    other: "Other problems",
    overexposed: "Overexposed",
    underexposed: "Underexposed",
    missed_focus: "Missed focus",
    motion_blur: "Motion blur",
    subject_placement: "Subject placement",
    subject_cut_off: "Subject cut off",
    busy_background: "Busy background",
  };

  const percent = (share: number) => `${Math.round(share * 100)}%`;
  const plural = (n: number, word: string) =>
    `${n.toLocaleString()} ${word}${n === 1 ? "" : "s"}`;
  const quarterLabel = (q: string) => q.replace("-", " ");

  /** In words, for a slope in rating points per year. */
  function slopeText(slope: number): string {
    const points = Math.abs(slope).toFixed(1);
    if (Math.abs(slope) < 0.1) return "Your ratings are steady.";
    return slope > 0
      ? `Your ratings are rising by about ${points} points a year.`
      : `Your ratings are falling by about ${points} points a year.`;
  }
</script>

<div class="insights" data-testid="insights">
  <h2>Insights</h2>
  <!-- A failure while drawing shows instead of a page stuck loading. -->
  <svelte:boundary>
    {#if error}
      <p class="error-text" role="alert">{error}</p>
    {:else if !insights}
      <p class="muted">Loading…</p>
    {:else if insights.critiques === 0}
      <div class="empty" data-testid="insights-empty">
        <p class="muted">
          No critiques yet. Insights summarize what the critic found across your
          photos.
        </p>
        {#if onsettings}
          <button class="primary" onclick={onsettings}>Open settings</button>
        {/if}
      </div>
    {:else}
      <div class="tiles">
        <div class="tile">
          <span class="label">Critiques</span>
          <span class="big">{insights.critiques.toLocaleString()}</span>
          {#if insights.as_of}
            <span class="muted">Up to {insights.as_of}</span>
          {/if}
        </div>
        <div class="tile">
          <span class="label">Average rating</span>
          <span class="big" data-testid="insights-rating">
            {insights.weighted_rating?.toFixed(1) ?? "–"}
          </span>
          <span class="muted"
            >Recent photos count more (plain average {insights.mean_rating?.toFixed(
              1,
            ) ?? "–"})</span
          >
        </div>
      </div>

      {#if insights.critiques < insights.min_critiques}
        <p class="muted note" data-testid="insights-few">
          Based on {plural(insights.critiques, "critique")}; {insights.min_critiques}
          or more give steadier insights.
        </p>
      {/if}

      {#if insights.with_issues === 0}
        <p class="muted note" data-testid="insights-no-issues">
          These critiques name no problem categories. Redo them from the loupe
          or in the background to see what recurs.
        </p>
      {:else}
        {#if insights.advice.length > 0}
          <section data-testid="insights-advice">
            <h3>What to work on</h3>
            <ul class="advice">
              {#each insights.advice as item (item.issue)}
                <li data-testid={`insights-advice-${item.issue}`}>
                  <h4>{item.title}</h4>
                  <p>{item.text}</p>
                  <p class="muted note">{item.evidence}</p>
                  <div class="examples">
                    {#each item.examples as id (id)}
                      <img
                        src={imageSrc(id)}
                        alt={`Example of ${ISSUES[item.issue].toLowerCase()}`}
                        loading="lazy"
                      />
                    {/each}
                  </div>
                </li>
              {/each}
            </ul>
          </section>
        {/if}

        <section>
          <h3>Recurring problems</h3>
          <p class="muted note">
            Share of {plural(insights.with_issues, "critique")} that name each problem.
            Critiques lose half their weight every {Math.round(
              insights.half_life_days / 30.4,
            )} months.
          </p>
          {#if insights.issues.length === 0}
            <p class="muted" data-testid="insights-none">
              No problem comes up in these critiques.
            </p>
          {/if}
          <ul class="issues">
            {#each insights.issues as issue (issue.issue)}
              <li data-testid={`insights-issue-${issue.issue}`}>
                <div class="head">
                  <span class="name">{ISSUES[issue.issue]}</span>
                  <div class="track" aria-hidden="true">
                    <span
                      class="bar"
                      style:width={`${Math.round(issue.weighted_share * 100)}%`}
                    ></span>
                  </div>
                  <span class="value"
                    >{percent(issue.weighted_share)}
                    <span class="muted">({issue.count.toLocaleString()})</span
                    ></span
                  >
                </div>
                <div class="examples">
                  {#each issue.examples as id (id)}
                    <img
                      src={imageSrc(id)}
                      alt={`Example of ${ISSUES[issue.issue].toLowerCase()}`}
                      loading="lazy"
                      data-testid="insights-example"
                    />
                  {/each}
                </div>
              </li>
            {/each}
          </ul>
        </section>

        {#if insights.habits.length > 0}
          <section>
            <h3>Settings habits</h3>
            <ul class="habits">
              {#each insights.habits as habit (`${habit.issue}-${habit.setting}-${habit.label}`)}
                <li data-testid="insights-habit">
                  {ISSUES[habit.issue]} shows up {habit.lift.toFixed(1)}× as
                  often at <strong>{habit.label}</strong>
                  <span class="muted"
                    >({percent(habit.share)} of {plural(
                      habit.critiques,
                      "photo",
                    )}, against {percent(habit.baseline)} overall)</span
                  >
                </li>
              {/each}
            </ul>
          </section>
        {/if}
      {/if}

      {#if insights.trend.length > 0}
        <section data-testid="insights-trend">
          <h3>Rating over time</h3>
          <div class="quarters">
            {#each insights.trend as q (q.quarter)}
              <div
                class="quarter"
                title={`${plural(q.critiques, "critique")}, average ${q.mean_rating.toFixed(1)}`}
              >
                <span class="value">{q.mean_rating.toFixed(1)}</span>
                <span class="column" aria-hidden="true">
                  <span class="fill" style:height={`${q.mean_rating * 10}%`}
                  ></span>
                </span>
                <span class="muted">{quarterLabel(q.quarter)}</span>
              </div>
            {/each}
          </div>
          {#if insights.trend_slope !== null}
            <p class="note" data-testid="insights-slope">
              {slopeText(insights.trend_slope)}
            </p>
          {:else}
            <p class="muted note">
              A trend appears once critiqued photos span three quarters.
            </p>
          {/if}
        </section>
      {/if}
    {/if}
    {#snippet failed(e)}
      <p class="error-text" role="alert">
        Could not show the insights: {ApiError.from(e).message}
      </p>
    {/snippet}
  </svelte:boundary>
</div>

<style>
  .insights {
    display: flex;
    flex-direction: column;
    gap: 1.2rem;
    max-width: 72rem;
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.6rem;
  }

  h2 {
    font-size: 1.05rem;
  }

  h3 {
    font-size: 0.95rem;
    margin-bottom: 0.4rem;
  }

  p {
    margin: 0;
  }

  .note {
    font-size: 0.85rem;
  }

  .advice li {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    padding: 0.8rem 1rem;
    border: 1px solid var(--border);
    border-left: 3px solid var(--accent);
    border-radius: var(--radius);
    background: var(--surface);
  }

  h4 {
    margin: 0;
    font-size: 0.95rem;
  }

  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(13rem, 1fr));
    gap: 0.8rem;
  }

  .tile {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    padding: 0.8rem 1rem;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    min-width: 0;
  }

  .label {
    font-size: 0.8rem;
    color: var(--muted);
  }

  .big {
    font-size: 1.5rem;
    font-weight: 600;
  }

  .tile .muted {
    font-size: 0.85rem;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 0.7rem;
  }

  .name {
    width: 9rem;
    flex: none;
    font-weight: 500;
  }

  .track {
    flex: 1;
    height: 0.6rem;
    min-width: 4rem;
    border-radius: 4px;
    background: var(--surface-2);
    overflow: hidden;
  }

  .bar {
    display: block;
    height: 100%;
    background: var(--accent);
  }

  .value {
    flex: none;
    min-width: 5rem;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .examples {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    margin-top: 0.5rem;
  }

  .examples img {
    width: 5.5rem;
    height: 5.5rem;
    object-fit: cover;
    border-radius: 4px;
    background: var(--surface-2);
  }

  .habits {
    gap: 0.5rem;
    font-size: 0.9rem;
  }

  .quarters {
    display: flex;
    align-items: flex-end;
    gap: 0.6rem;
    overflow-x: auto;
    padding-bottom: 0.3rem;
  }

  .quarter {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.25rem;
    min-width: 3.4rem;
    font-size: 0.8rem;
  }

  .quarter .value {
    min-width: 0;
    text-align: center;
  }

  .column {
    display: flex;
    align-items: flex-end;
    width: 1.6rem;
    height: 6rem;
    border-radius: 4px;
    background: var(--surface-2);
    overflow: hidden;
  }

  .fill {
    display: block;
    width: 100%;
    background: var(--accent);
  }

  @media (max-width: 40rem) {
    .name {
      width: 6rem;
    }
  }
</style>
