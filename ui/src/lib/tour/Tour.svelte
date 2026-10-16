<script lang="ts">
  import type { TourStep } from "./steps";

  interface Props {
    steps: TourStep[];
    /** Called when the tour is finished or skipped. */
    onend: () => void;
  }

  let { steps, onend }: Props = $props();

  let index = $state(0);
  /** Where the described control is, or null when it is not on screen. */
  let rect = $state<DOMRect | null>(null);
  let viewport = $state({ width: 0, height: 0 });

  const step = $derived(steps[index]);
  const last = $derived(index === steps.length - 1);

  /** Space around the highlighted control, and between it and the card. */
  const PAD = 6;
  const GAP = 12;
  const CARD_WIDTH = 340;

  function measure() {
    viewport = { width: window.innerWidth, height: window.innerHeight };
    const el = step ? document.querySelector(step.target) : null;
    const r = el?.getBoundingClientRect();
    rect = r && r.width > 0 && r.height > 0 ? r : null;
  }

  $effect(() => {
    void index;
    const el = step ? document.querySelector(step.target) : null;
    el?.scrollIntoView?.({ block: "nearest" });
    measure();
    window.addEventListener("resize", measure);
    window.addEventListener("scroll", measure, true);
    return () => {
      window.removeEventListener("resize", measure);
      window.removeEventListener("scroll", measure, true);
    };
  });

  /** The card's height, once shown. */
  let cardHeight = $state(0);

  /** Below the control, else above it, else over it; always on screen. */
  const position = $derived.by(() => {
    if (!rect) return "left: 50%; top: 50%; transform: translate(-50%, -50%);";
    const h = cardHeight || 200;
    const clamp = (v: number, max: number) => Math.max(GAP, Math.min(v, max));
    const left = clamp(rect.left, viewport.width - CARD_WIDTH - GAP);
    const below = rect.bottom + PAD + GAP;
    const above = rect.top - PAD - GAP - h;
    let top: number;
    if (below + h + GAP <= viewport.height) top = below;
    else if (above >= GAP) top = above;
    else top = rect.top + GAP;
    return `left: ${left}px; top: ${clamp(top, viewport.height - h - GAP)}px;`;
  });

  function next() {
    if (last) onend();
    else index += 1;
  }

  function back() {
    if (index > 0) index -= 1;
  }

  // Captured first, so the library does not also act on these keys.
  function onkeydown(e: KeyboardEvent) {
    const actions: Record<string, () => void> = {
      Escape: onend,
      ArrowRight: next,
      Enter: next,
      ArrowLeft: back,
    };
    const action = actions[e.key];
    if (!action) return;
    e.preventDefault();
    e.stopPropagation();
    action();
  }
</script>

<svelte:window onkeydowncapture={onkeydown} />

{#if step}
  {#if rect}
    <div
      class="highlight"
      data-testid="tour-highlight"
      style:left={`${rect.left - PAD}px`}
      style:top={`${rect.top - PAD}px`}
      style:width={`${rect.width + 2 * PAD}px`}
      style:height={`${rect.height + 2 * PAD}px`}
    ></div>
  {:else}
    <div class="shade"></div>
  {/if}
  <div
    class="card tour"
    role="dialog"
    aria-label={step.title}
    data-testid="tour"
    style={position}
    style:width={`${CARD_WIDTH}px`}
    bind:offsetHeight={cardHeight}
  >
    <p class="count muted">{index + 1} of {steps.length}</p>
    <h3>{step.title}</h3>
    <p>{step.text}</p>
    <footer>
      <button class="link" onclick={onend}>Skip tour</button>
      <span class="spacer"></span>
      {#if index > 0}
        <button onclick={back}>Back</button>
      {/if}
      <button class="primary" onclick={next} data-testid="tour-next"
        >{last ? "Done" : "Next"}</button
      >
    </footer>
  </div>
{/if}

<style>
  .highlight,
  .shade {
    position: fixed;
    z-index: 40;
    pointer-events: none;
  }

  .highlight {
    border-radius: 8px;
    outline: 2px solid var(--accent);
    box-shadow: 0 0 0 100vmax rgb(0 0 0 / 0.35);
    transition:
      left 0.15s,
      top 0.15s,
      width 0.15s,
      height 0.15s;
  }

  .shade {
    inset: 0;
    background: rgb(0 0 0 / 0.35);
  }

  .tour {
    position: fixed;
    z-index: 41;
    max-width: calc(100vw - 2rem);
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    background: var(--bg);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.25);
  }

  .tour p {
    margin: 0;
  }

  .count {
    font-size: 0.8rem;
  }

  h3 {
    font-size: 1rem;
  }

  footer {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-top: 0.4rem;
  }

  .spacer {
    flex: 1;
  }

  .link {
    border: none;
    background: none;
    padding: 0.2rem 0;
    color: var(--muted);
  }
</style>
