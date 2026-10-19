<script lang="ts">
  import ExternalLink from "../components/ExternalLink.svelte";
  import { DARKTABLE_GUIDE } from "../help/shortcuts";

  const KEY = "picturesque.darktable-note-dismissed";

  function dismissed(): boolean {
    try {
      return localStorage.getItem(KEY) === "1";
    } catch {
      return false;
    }
  }

  let hidden = $state(dismissed());

  function dismiss() {
    hidden = true;
    try {
      localStorage.setItem(KEY, "1");
    } catch {
      // Shown again next time; harmless.
    }
  }
</script>

{#if !hidden}
  <aside class="note" data-testid="darktable-note">
    <p>
      <strong>Using darktable?</strong> If it already knows this folder, close
      it while you cull, and turn on
      <em>look for updated XMP files on startup</em>
      in its preferences so it takes your picks.
      <ExternalLink href={DARKTABLE_GUIDE}>How it works</ExternalLink>
    </p>
    <button
      class="link"
      onclick={dismiss}
      aria-label="Hide this note"
      data-testid="darktable-note-dismiss">Got it</button
    >
  </aside>
{/if}

<style>
  .note {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.5rem 0.9rem;
    border-radius: var(--radius);
    background: var(--surface-2);
    font-size: 0.88rem;
  }

  .note p {
    margin: 0;
    flex: 1;
  }
</style>
