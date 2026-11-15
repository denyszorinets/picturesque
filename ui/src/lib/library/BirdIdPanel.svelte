<script lang="ts" module>
  import type { BoxView, SpeciesView, SuggestionView } from "../api";

  /** A bird in the photo: found by the bird finder, or boxed by hand. */
  export interface Bird {
    box: BoxView;
    /** How sure the finder is it is a bird; `null` when drawn by hand. */
    score: number | null;
    suggestions: SuggestionView[];
    elsewhere: SpeciesView | null;
  }
</script>

<script lang="ts">
  import Icon from "../components/Icon.svelte";

  interface Props {
    birds: Bird[];
    /** The bird whose suggestions are shown in full. */
    selected: number | null;
    onselect: (index: number) => void;
    /** Waiting for the user to draw a box. */
    drawing: boolean;
    /** A model is looking. */
    busy: boolean;
    error: string | null;
    /** The region suggestions are limited to, if any. */
    region: string | null;
    /** The species chosen for the shown photo. */
    current: SpeciesView[];
    /** Photos in the shown photo's stack (0 or 1 when not in one). */
    stackSize: number;
    /** Adds a species, to the photo or its whole stack. */
    onadd: (species: SpeciesView, wholeStack: boolean) => void;
    /** Removes a species, from the photo or its whole stack. */
    onremove: (code: string, wholeStack: boolean) => void;
    onfind: () => void;
    ondraw: () => void;
    onclose: () => void;
  }

  let {
    birds,
    selected,
    onselect,
    drawing,
    busy,
    error,
    region,
    current,
    stackSize,
    onadd,
    onremove,
    onfind,
    ondraw,
    onclose,
  }: Props = $props();

  /** Naming one frame of a stack names the whole stack. */
  let wholeStack = $state(true);
  const whole = $derived(stackSize > 1 && wholeStack);
  const chosen = (code: string) => current.some((s) => s.code === code);

  const percent = (v: number) =>
    v >= 0.995 ? ">99%" : v < 0.005 ? "<1%" : `${Math.round(v * 100)}%`;

  function act(e: Event, action: () => void) {
    (e.currentTarget as HTMLElement).blur();
    action();
  }
</script>

<section class="birdid" aria-label="Bird ID" data-testid="bird-id">
  <header>
    <h2>Bird ID</h2>
    <button
      class="icon"
      onclick={(e) => act(e, onclose)}
      aria-label="Close Bird ID"><Icon name="x-mark" /></button
    >
  </header>

  <div class="actions">
    <button
      onclick={(e) => act(e, onfind)}
      disabled={busy}
      title="Find every bird in the photo (F)"
      data-testid="bird-id-find"><Icon name="sparkles" /> Find birds</button
    >
    <button
      onclick={(e) => act(e, ondraw)}
      aria-pressed={drawing}
      title="Draw a box around a bird (D)"
      data-testid="bird-id-draw-button"
      ><Icon name="viewfinder-circle" /> Draw a box</button
    >
  </div>

  {#if current.length > 0}
    <ul class="chosen" data-testid="bird-id-current">
      {#each current as s (s.code)}
        <li>
          <span><strong>{s.common}</strong> <em>{s.scientific}</em></span>
          <button
            class="icon"
            onclick={(e) => act(e, () => onremove(s.code, whole))}
            title={whole ? "Remove from every photo of the stack" : "Remove"}
            aria-label={`Remove ${s.common}`}
            data-testid="bird-id-forget"><Icon name="x-mark" /></button
          >
        </li>
      {/each}
    </ul>
  {/if}

  {#if drawing}
    <p class="hint" data-testid="bird-id-draw">
      Drag a box around a bird on the photo. Esc cancels.
    </p>
  {/if}
  {#if busy}
    <p class="hint" data-testid="bird-id-busy">Looking…</p>
  {/if}
  {#if error}
    <p class="error-text" role="alert">{error}</p>
  {/if}

  {#if birds.length > 0}
    {#if region}<p class="hint">Birds of {region}</p>{/if}
    <ol class="birds" data-testid="bird-id-birds">
      {#each birds as bird, i (i)}
        {@const top = bird.suggestions[0]}
        <li class:open={selected === i}>
          <button
            class="bird"
            onclick={(e) => act(e, () => onselect(i))}
            aria-expanded={selected === i}
            data-testid="bird-id-bird"
          >
            <span class="num">{i + 1}</span>
            <span class="names">
              <span class="common">{top?.species.common ?? "?"}</span>
              <span class="sci"
                >{bird.score === null
                  ? "drawn by hand"
                  : `bird ${percent(bird.score)} sure`}</span
              >
            </span>
            {#if top}<span class="confidence">{percent(top.confidence)}</span
              >{/if}
          </button>
          {#if selected === i}
            <ol class="suggestions" data-testid="bird-id-suggestions">
              {#each bird.suggestions as s (s.species.code)}
                <li>
                  <button
                    class="suggestion"
                    class:chosen={chosen(s.species.code)}
                    onclick={(e) =>
                      act(e, () =>
                        chosen(s.species.code)
                          ? onremove(s.species.code, whole)
                          : onadd(s.species, whole),
                      )}
                    title={chosen(s.species.code)
                      ? "Chosen; click to remove"
                      : "Add this species to the photo"}
                    data-testid="bird-id-suggestion"
                  >
                    <span class="names">
                      <span class="common">{s.species.common}</span>
                      <span class="sci">{s.species.scientific}</span>
                    </span>
                    <span class="confidence"
                      >{chosen(s.species.code)
                        ? "✓"
                        : percent(s.confidence)}</span
                    >
                  </button>
                </li>
              {/each}
            </ol>
            {#if bird.elsewhere}
              <p class="hint" data-testid="bird-id-elsewhere">
                Outside {region}, the closest match is
                <button
                  class="link"
                  onclick={() => bird.elsewhere && onadd(bird.elsewhere, whole)}
                  >{bird.elsewhere.common}</button
                >.
              </p>
            {/if}
          {/if}
        </li>
      {/each}
    </ol>
    {#if stackSize > 1}
      <label class="stack">
        <input
          type="checkbox"
          bind:checked={wholeStack}
          onchange={(e) => e.currentTarget.blur()}
          data-testid="bird-id-whole-stack"
        />
        Name all {stackSize} photos of the stack
      </label>
    {/if}
    <p class="hint small">
      The identifier always names a bird, even for a box without one.
    </p>
  {:else if !drawing && !busy && !error}
    <p class="hint">
      Find every bird with <kbd>F</kbd>, or draw a box around one with
      <kbd>D</kbd>.
    </p>
  {/if}
</section>

<style>
  .birdid {
    position: absolute;
    top: 4.2rem;
    right: 1rem;
    z-index: 2;
    width: 18rem;
    max-height: calc(100% - 10rem);
    overflow-y: auto;
    padding: 0.8rem;
    background: rgb(23 23 22 / 0.95);
    border: 1px solid #2a2a27;
    border-radius: 10px;
    font-size: 0.85rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  h2 {
    font-size: 0.8rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: #9a9a94;
    margin: 0;
  }

  p {
    margin: 0;
  }

  .hint {
    color: #b5b5ae;
  }

  .small {
    font-size: 0.78rem;
  }

  .actions {
    display: flex;
    gap: 0.4rem;
  }

  .actions button {
    flex: 1;
    background: transparent;
    color: inherit;
    border: 1px solid #3a3a36;
    padding: 0.3rem 0.5rem;
  }

  .actions button:hover:not(:disabled),
  .actions button[aria-pressed="true"] {
    background: #2a2a27;
  }

  .chosen {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .chosen li {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.4rem;
    padding: 0.2rem 0.3rem 0.2rem 0.5rem;
    border-left: 3px solid #8ee3ad;
    background: #1f241f;
    border-radius: 4px;
  }

  .chosen em,
  .sci {
    font-style: italic;
    color: #9a9a94;
    font-size: 0.78rem;
  }

  ol {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .bird,
  .suggestion {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.35rem 0.5rem;
    background: transparent;
    color: inherit;
    border: 1px solid #3a3a36;
    border-radius: 6px;
    text-align: left;
  }

  .bird:hover,
  .suggestion:hover {
    background: #22221f;
  }

  .open > .bird {
    border-color: #f2c94c;
  }

  .num {
    flex-shrink: 0;
    width: 1.3rem;
    height: 1.3rem;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: #f2c94c;
    color: #0d0d0c;
    font-weight: 700;
    font-size: 0.75rem;
  }

  .suggestions {
    margin: 0.25rem 0 0.2rem 1.8rem;
  }

  .suggestion.chosen {
    border-color: #8ee3ad;
  }

  .names {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }

  .common {
    font-weight: 600;
  }

  .confidence {
    font-variant-numeric: tabular-nums;
    color: #b5b5ae;
    flex-shrink: 0;
  }

  .stack {
    display: flex;
    gap: 0.4rem;
    align-items: center;
  }

  .icon,
  .link {
    border: none;
    background: none;
    color: #b5b5ae;
    padding: 0.1rem 0.2rem;
  }

  .link {
    text-decoration: underline;
    text-underline-offset: 2px;
    font-size: inherit;
  }

  .icon:hover,
  .link:hover {
    color: #ececea;
  }
</style>
