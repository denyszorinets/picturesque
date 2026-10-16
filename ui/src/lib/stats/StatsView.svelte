<script lang="ts">
  import { ApiError, type GearStats, type LibraryStats } from "../api";

  interface Props {
    /** Loads the statistics (injectable for tests). */
    load: () => Promise<LibraryStats>;
    /** Whether the view is on screen; it reloads each time it is shown. */
    visible: boolean;
    /** Goes to where photos are imported (or set up first). */
    onimport?: () => void;
  }

  let { load, visible, onimport }: Props = $props();
  let stats = $state<LibraryStats | null>(null);
  let error = $state<string | null>(null);

  /** Focal lengths listed per camera. */
  const FOCAL_SHOWN = 5;

  $effect(() => {
    if (!visible) return;
    load()
      .then((s) => {
        stats = s;
        error = null;
      })
      .catch((e: unknown) => (error = ApiError.from(e).message));
  });

  const percent = (part: number, whole: number) =>
    whole === 0 ? "0%" : `${Math.round((part / whole) * 100)}%`;
  const share = (part: number, whole: number) =>
    whole === 0 ? 0 : (part / whole) * 100;
  const plural = (n: number, word: string) =>
    `${n.toLocaleString()} ${word}${n === 1 ? "" : "s"}`;

  /** The most successful of a list, unless all have few photos. */
  const leader = (gear: GearStats[]) =>
    gear[0] && !gear[0].few_photos ? gear[0] : null;
  const bestCamera = $derived(stats ? leader(stats.cameras) : null);
  const bestLens = $derived(stats ? leader(stats.lenses) : null);
  const withFocal = $derived(
    stats?.cameras.filter((c) => c.focal_lengths.length > 0) ?? [],
  );
</script>

{#snippet rate(g: GearStats)}
  <div
    class="rate"
    title={`${percent(g.counts.picks, g.counts.photos)} picked, ${percent(g.counts.auto_picked, g.counts.photos)} auto-picked`}
  >
    <div class="track" aria-hidden="true">
      {#if g.counts.picks > 0}
        <span
          class="bar picked"
          style:width={`${share(g.counts.picks, g.counts.photos)}%`}
        ></span>
      {/if}
      {#if g.counts.auto_picked > 0}
        <span
          class="bar auto"
          style:width={`${share(g.counts.auto_picked, g.counts.photos)}%`}
        ></span>
      {/if}
    </div>
    <span class="value" data-testid="keeper-rate"
      >{percent(g.counts.picks + g.counts.auto_picked, g.counts.photos)}</span
    >
  </div>
{/snippet}

{#snippet table(gear: GearStats[], kind: string, testid: string)}
  <table data-testid={testid}>
    <thead>
      <tr>
        <th scope="col">{kind}</th>
        <th scope="col" class="num">Photos</th>
        <th scope="col" class="rate-col">Keepers</th>
        <th scope="col" class="num">Picked</th>
        <th scope="col" class="num">Auto-picked</th>
        <th scope="col" class="num">Rejected</th>
      </tr>
    </thead>
    <tbody>
      {#each gear as g, i (g.label)}
        {#if g.few_photos && (i === 0 || !gear[i - 1]!.few_photos)}
          <tr class="divider">
            <td colspan="6"
              >Fewer than {stats!.min_photos} photos: too few to compare</td
            >
          </tr>
        {/if}
        <tr class:few={g.few_photos} data-testid="gear-row">
          <th scope="row" class="name">{g.label}</th>
          <td class="num">{g.counts.photos.toLocaleString()}</td>
          <td>{@render rate(g)}</td>
          <td class="num">{g.counts.picks.toLocaleString()}</td>
          <td class="num">{g.counts.auto_picked.toLocaleString()}</td>
          <td class="num">{g.counts.rejects.toLocaleString()}</td>
        </tr>
      {/each}
    </tbody>
  </table>
{/snippet}

<div class="stats" data-testid="stats">
  <h2>Statistics</h2>
  <!-- A failure while drawing shows instead of a page stuck loading. -->
  <svelte:boundary>
    {#if error}
      <p class="error-text" role="alert">{error}</p>
    {:else if !stats}
      <p class="muted">Loading…</p>
    {:else if stats.all.photos === 0}
      <div class="empty" data-testid="stats-empty">
        <p class="muted">
          No photos yet. Statistics appear once photos are imported.
        </p>
        {#if onimport}
          <button class="primary" onclick={onimport}>Import photos</button>
        {/if}
      </div>
    {:else}
      <div class="tiles">
        <div class="tile">
          <span class="label">Photos</span>
          <span class="big">{stats.all.photos.toLocaleString()}</span>
          <span class="muted">{plural(stats.all.rejects, "reject")}</span>
        </div>
        <div class="tile">
          <span class="label">Keepers</span>
          <span class="big" data-testid="overall-rate"
            >{percent(
              stats.all.picks + stats.all.auto_picked,
              stats.all.photos,
            )}</span
          >
          <span class="muted"
            >{stats.all.picks.toLocaleString()} picked, {stats.all.auto_picked.toLocaleString()}
            auto-picked</span
          >
        </div>
        <div class="tile">
          <span class="label">Most successful camera</span>
          {#if bestCamera}
            <span class="big name" data-testid="best-camera"
              >{bestCamera.label}</span
            >
            <span class="muted"
              >{percent(
                bestCamera.counts.picks + bestCamera.counts.auto_picked,
                bestCamera.counts.photos,
              )} keepers of {plural(bestCamera.counts.photos, "photo")}</span
            >
          {:else}
            <span class="muted">Not enough photos with camera information.</span
            >
          {/if}
        </div>
        <div class="tile">
          <span class="label">Most successful lens</span>
          {#if bestLens}
            <span class="big name" data-testid="best-lens"
              >{bestLens.label}</span
            >
            <span class="muted"
              >{percent(
                bestLens.counts.picks + bestLens.counts.auto_picked,
                bestLens.counts.photos,
              )} keepers of {plural(bestLens.counts.photos, "photo")}</span
            >
          {:else}
            <span class="muted">Not enough photos with lens information.</span>
          {/if}
        </div>
      </div>

      <p class="muted note">
        <strong>Keepers</strong> are photos you picked and photos auto-pick
        suggests keeping (not yet decided). Gear is ranked by the share of its
        photos that are keepers.
        <span class="legend">
          <span class="swatch picked"></span>Picked
          <span class="swatch auto"></span>Auto-picked
        </span>
      </p>

      <section>
        <h3>Cameras</h3>
        {#if stats.cameras.length === 0}
          <p class="muted">No photos record their camera.</p>
        {:else}
          {@render table(stats.cameras, "Camera", "camera-stats")}
        {/if}
      </section>

      <section>
        <h3>Lenses</h3>
        {#if stats.lenses.length === 0}
          <p class="muted">No photos record their lens.</p>
        {:else}
          {@render table(stats.lenses, "Lens", "lens-stats")}
        {/if}
      </section>

      {#if withFocal.length > 0}
        <section>
          <h3>Most used focal lengths</h3>
          <div class="focal-grid">
            {#each withFocal as c (c.label)}
              {@const recorded = c.focal_lengths.reduce(
                (n, f) => n + f.count,
                0,
              )}
              <div class="focal" data-testid="focal-lengths">
                <h4>{c.label}</h4>
                <ol>
                  {#each c.focal_lengths.slice(0, FOCAL_SHOWN) as f (f.value)}
                    <li
                      title={`${plural(f.count, "photo")} at ${f.value} mm (${percent(f.count, recorded)})`}
                    >
                      <span class="mm">{f.value} mm</span>
                      <span class="track" aria-hidden="true"
                        ><span
                          class="bar focal-bar"
                          style:width={`${share(f.count, c.focal_lengths[0]!.count)}%`}
                        ></span></span
                      >
                      <span class="value">{percent(f.count, recorded)}</span>
                    </li>
                  {/each}
                </ol>
                {#if c.focal_lengths.length > FOCAL_SHOWN}
                  <p class="muted more">
                    and {plural(c.focal_lengths.length - FOCAL_SHOWN, "other")}
                  </p>
                {/if}
              </div>
            {/each}
          </div>
        </section>
      {/if}
    {/if}
    {#snippet failed(e)}
      <p class="error-text" role="alert" data-testid="stats-failed">
        Statistics could not be shown: {e instanceof Error
          ? e.message
          : String(e)}
      </p>
    {/snippet}
  </svelte:boundary>
</div>

<style>
  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.6rem;
  }

  .stats {
    display: flex;
    flex-direction: column;
    gap: 1.2rem;
    max-width: 72rem;
  }

  h2 {
    font-size: 1.05rem;
  }

  h3 {
    font-size: 0.95rem;
    margin-bottom: 0.5rem;
  }

  h4 {
    font-size: 0.9rem;
    margin: 0 0 0.5rem;
  }

  p {
    margin: 0;
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

  .big.name {
    font-size: 1.1rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tile .muted {
    font-size: 0.85rem;
  }

  .note {
    font-size: 0.85rem;
  }

  .legend {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    margin-left: 0.6rem;
    color: var(--fg);
  }

  .swatch {
    display: inline-block;
    width: 0.8rem;
    height: 0.8rem;
    border-radius: 3px;
  }

  .swatch.auto {
    margin-left: 0.5rem;
  }

  section {
    min-width: 0;
    overflow-x: auto;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.9rem;
  }

  th,
  td {
    padding: 0.45rem 0.6rem;
    border-bottom: 1px solid var(--border);
    text-align: left;
    font-weight: normal;
  }

  thead th {
    font-size: 0.8rem;
    color: var(--muted);
  }

  th.name {
    font-weight: 500;
  }

  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .rate-col {
    width: 32%;
  }

  tr.few {
    color: var(--muted);
  }

  tr.divider td {
    padding-top: 0.9rem;
    font-size: 0.8rem;
    color: var(--muted);
  }

  .rate {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  .track {
    flex: 1;
    display: flex;
    gap: 2px;
    height: 0.6rem;
    min-width: 4rem;
    border-radius: 4px;
    background: var(--surface-2);
    overflow: hidden;
  }

  .bar {
    display: block;
    height: 100%;
  }

  .bar:last-child {
    border-radius: 0 4px 4px 0;
  }

  .picked {
    background: var(--accent);
  }

  .auto {
    background: color-mix(in srgb, var(--accent) 40%, var(--surface-2));
  }

  .value {
    min-width: 2.6rem;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .focal-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(17rem, 1fr));
    gap: 0.8rem;
  }

  .focal {
    padding: 0.8rem 1rem;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
  }

  ol {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  li {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    font-size: 0.85rem;
  }

  .mm {
    min-width: 3.6rem;
    font-variant-numeric: tabular-nums;
  }

  .focal-bar {
    background: var(--info);
    border-radius: 0 4px 4px 0;
  }

  .more {
    margin-top: 0.4rem;
    font-size: 0.8rem;
  }
</style>
