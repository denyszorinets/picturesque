<script lang="ts">
  import type { PhotoFacets } from "../api";
  import type { Filters } from "./filters.svelte";

  interface Props {
    filters: Filters;
    facets: PhotoFacets;
  }

  let { filters, facets }: Props = $props();

  const FLAGS = [
    { flag: "pick", label: "Picks" },
    { flag: "unflagged", label: "Unflagged" },
    { flag: "reject", label: "Rejects" },
  ] as const;

  const flagCount = (flag: string) =>
    facets.flags.find((f) => f.flag === flag)?.count ?? 0;
  /** Photos with at least `stars` stars. */
  const atLeast = (stars: number) =>
    facets.ratings.slice(stars).reduce((a, b) => a + b, 0);

  /** Countries with their total photo counts, most photos first. */
  const countries = $derived.by(() => {
    const byCode: Record<string, { country: string; count: number }> = {};
    for (const p of facets.places) {
      const entry = (byCode[p.value.country_code] ??= {
        country: p.value.country,
        count: 0,
      });
      entry.count += p.count;
    }
    return Object.entries(byCode).sort((a, b) => b[1].count - a[1].count);
  });

  const cameraValue = $derived(
    filters.camera ? `${filters.camera.make}\u0000${filters.camera.model}` : "",
  );
  const placeValue = $derived(
    filters.place
      ? `${filters.place.country_code}\u0000${filters.place.city ?? ""}`
      : "",
  );

  function setCamera(value: string) {
    const facet = facets.cameras.find(
      (c) => `${c.value.key.make}\u0000${c.value.key.model}` === value,
    );
    filters.camera = facet ? { ...facet.value.key } : null;
    filters.cameraLabel = facet?.value.label ?? "";
  }

  function setPlace(value: string) {
    if (!value) {
      filters.place = null;
      return;
    }
    const [code, city] = value.split("\u0000");
    const country = countries.find(([c]) => c === code)?.[1].country ?? code!;
    filters.place = city
      ? { country_code: code!, city, label: `${city}, ${country}` }
      : { country_code: code!, label: country };
  }
</script>

<div class="panel" data-testid="filter-panel">
  <section>
    <h3>Flag</h3>
    <div class="toggles">
      {#each FLAGS as { flag, label } (flag)}
        <button
          class:on={filters.flags.includes(flag)}
          aria-pressed={filters.flags.includes(flag)}
          onclick={() => filters.toggleFlag(flag)}
        >
          {label} <span class="count">{flagCount(flag)}</span>
        </button>
      {/each}
    </div>
  </section>

  <section>
    <h3>Rating</h3>
    <div class="toggles" role="radiogroup" aria-label="Minimum rating">
      {#each [0, 1, 2, 3, 4, 5] as stars (stars)}
        <button
          role="radio"
          aria-checked={filters.minRating === stars}
          class:on={filters.minRating === stars}
          onclick={() => (filters.minRating = stars)}
          title={stars === 0
            ? "Any rating"
            : `${stars} stars or more (${atLeast(stars)})`}
        >
          {stars === 0 ? "Any" : `${stars}★+`}
        </button>
      {/each}
    </div>
  </section>

  <section>
    <h3>Date</h3>
    <label
      >From <input
        type="date"
        bind:value={filters.from}
        min={facets.first_day}
        max={facets.last_day}
        data-testid="filter-from"
      /></label
    >
    <label
      >To <input
        type="date"
        bind:value={filters.to}
        min={facets.first_day}
        max={facets.last_day}
        data-testid="filter-to"
      /></label
    >
  </section>

  {#if facets.places.length > 0}
    <section>
      <h3>Place</h3>
      <select
        value={placeValue}
        onchange={(e) => setPlace(e.currentTarget.value)}
        aria-label="Place"
        data-testid="filter-place"
      >
        <option value="">Anywhere</option>
        {#each countries as [code, { country, count }] (code)}
          <optgroup label={country}>
            <option value={`${code}\u0000`}>All of {country} ({count})</option>
            {#each facets.places.filter((p) => p.value.country_code === code) as p (p.value.city)}
              <option value={`${code}\u0000${p.value.city}`}
                >{p.value.city} ({p.count})</option
              >
            {/each}
          </optgroup>
        {/each}
      </select>
    </section>
  {/if}

  {#if facets.cameras.length > 0}
    <section>
      <h3>Camera</h3>
      <select
        value={cameraValue}
        onchange={(e) => setCamera(e.currentTarget.value)}
        aria-label="Camera"
        data-testid="filter-camera"
      >
        <option value="">Any camera</option>
        {#each facets.cameras as c (c.value.label + c.value.key.make)}
          <option value={`${c.value.key.make}\u0000${c.value.key.model}`}
            >{c.value.label} ({c.count})</option
          >
        {/each}
      </select>
    </section>
  {/if}

  {#if facets.lenses.length > 0}
    <section>
      <h3>Lens</h3>
      <select
        value={filters.lens ?? ""}
        onchange={(e) => (filters.lens = e.currentTarget.value || null)}
        aria-label="Lens"
        data-testid="filter-lens"
      >
        <option value="">Any lens</option>
        {#each facets.lenses as l (l.value)}
          <option value={l.value}>{l.value} ({l.count})</option>
        {/each}
      </select>
    </section>
  {/if}
</div>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    gap: 1.1rem;
    font-size: 0.88rem;
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  h3 {
    font-size: 0.78rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
  }

  .toggles {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
  }

  .toggles button {
    padding: 0.2rem 0.55rem;
    font-size: 0.82rem;
    border-radius: 999px;
  }

  .toggles button.on {
    background: var(--fg);
    color: var(--bg);
    border-color: var(--fg);
  }

  .count {
    opacity: 0.6;
  }

  label {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  select {
    width: 100%;
  }
</style>
