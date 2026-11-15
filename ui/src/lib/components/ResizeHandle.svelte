<script lang="ts">
  import { onMount } from "svelte";

  interface Props {
    /** Width of the pane it resizes, in pixels. */
    width: number;
    /** Which edge of the pane the handle sits on: dragging away from the
     * pane widens it. */
    edge: "left" | "right";
    min?: number;
    max?: number;
    /** Remembered between sessions under this key (localStorage). */
    storageKey?: string;
    label: string;
  }

  let {
    width = $bindable(),
    edge,
    min = 160,
    max = 640,
    storageKey,
    label,
  }: Props = $props();

  // A remembered width, once.
  onMount(() => {
    if (!storageKey) return;
    try {
      const saved = Number(localStorage.getItem(storageKey));
      if (saved >= min && saved <= max) width = saved;
    } catch {
      // A convenience only.
    }
  });

  let start: { x: number; width: number } | null = null;
  const clamp = (w: number) => Math.round(Math.min(max, Math.max(min, w)));

  function save() {
    if (!storageKey) return;
    try {
      localStorage.setItem(storageKey, String(width));
    } catch {
      // A convenience only.
    }
  }

  function down(e: PointerEvent) {
    start = { x: e.clientX, width };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    e.preventDefault();
  }

  function move(e: PointerEvent) {
    if (!start) return;
    const dx = e.clientX - start.x;
    width = clamp(start.width + (edge === "right" ? dx : -dx));
  }

  function up() {
    if (!start) return;
    start = null;
    save();
  }

  function key(e: KeyboardEvent) {
    const step = e.shiftKey ? 64 : 16;
    const grow = edge === "right" ? "ArrowRight" : "ArrowLeft";
    const shrink = edge === "right" ? "ArrowLeft" : "ArrowRight";
    if (e.key !== grow && e.key !== shrink) return;
    e.preventDefault();
    e.stopPropagation();
    width = clamp(width + (e.key === grow ? step : -step));
    save();
  }
</script>

<!-- A focusable separator with a value is ARIA's window splitter, which
     is interactive. -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_no_noninteractive_tabindex -->
<div
  class="handle {edge}"
  role="separator"
  aria-orientation="vertical"
  aria-label={label}
  aria-valuenow={width}
  aria-valuemin={min}
  aria-valuemax={max}
  tabindex="0"
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
  onkeydown={key}
  data-testid="resize-handle"
></div>

<style>
  .handle {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 8px;
    cursor: col-resize;
    z-index: 3;
    touch-action: none;
  }

  .handle.right {
    right: -4px;
  }

  .handle.left {
    left: -4px;
  }

  .handle:hover,
  .handle:focus-visible {
    background: rgb(236 236 234 / 0.15);
    outline: none;
  }
</style>
