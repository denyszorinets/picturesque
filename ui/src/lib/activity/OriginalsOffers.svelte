<script lang="ts">
  import * as api from "../api";
  import type { OriginalsOffer } from "../api";
  import OriginalsDialog from "./OriginalsDialog.svelte";

  interface Props {
    /** Whether the view is on screen; offers load only then. */
    visible: boolean;
    /** Changes when the import pipeline did something: offers reload. */
    revision?: unknown;
  }

  let { visible, revision }: Props = $props();

  let offers = $state<OriginalsOffer[]>([]);
  let open = $state<OriginalsOffer | null>(null);

  async function load() {
    try {
      offers = await api.originalsOffers();
    } catch {
      // Informational; keep what was shown.
    }
  }

  $effect(() => {
    void revision;
    if (visible) void load();
  });

  function close() {
    open = null;
    void load();
  }

  function plural(n: number, word: string): string {
    return `${n} ${word}${n === 1 ? "" : "s"}`;
  }
</script>

{#if offers.length > 0}
  <ul class="offers" data-testid="originals-offers">
    {#each offers as offer (offer.source_id)}
      <li class="notice">
        <span>
          <code>{offer.path}</code>: {plural(offer.photos, "photo")} imported. Their
          originals can be deleted after a check.
        </span>
        <button
          onclick={() => (open = offer)}
          data-testid={`originals-offer-${offer.source_id}`}
          >Delete originals…</button
        >
      </li>
    {/each}
  </ul>
{/if}

{#if open}
  <OriginalsDialog
    source={{ id: open.source_id, path: open.path }}
    onclose={close}
  />
{/if}

<style>
  .offers {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .notice {
    padding: 0.5rem 0.7rem;
    border-radius: 8px;
    background: color-mix(in srgb, var(--warn) 15%, transparent);
    font-size: 0.9rem;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.8rem;
    overflow-wrap: anywhere;
  }
</style>
