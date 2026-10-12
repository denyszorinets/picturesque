import { SvelteSet } from "svelte/reactivity";

/**
 * Which photos are selected, by position in the timeline list. Click selects
 * one, Shift extends a range from the anchor, Ctrl/Cmd toggles.
 */
export class Selection {
  /** Selected list indices. */
  readonly selected = new SvelteSet<number>();
  /** The photo keyboard actions apply to (and ranges extend from). */
  focus = $state<number | null>(null);
  private anchor: number | null = null;

  has(index: number): boolean {
    return this.selected.has(index);
  }

  get size(): number {
    return this.selected.size;
  }

  click(index: number, mods: { shift?: boolean; toggle?: boolean } = {}): void {
    if (mods.shift && this.anchor !== null) {
      const [a, b] =
        this.anchor < index ? [this.anchor, index] : [index, this.anchor];
      if (!mods.toggle) this.selected.clear();
      for (let i = a; i <= b; i++) this.selected.add(i);
    } else if (mods.toggle) {
      if (this.selected.has(index)) this.selected.delete(index);
      else this.selected.add(index);
      this.anchor = index;
    } else {
      this.selected.clear();
      this.selected.add(index);
      this.anchor = index;
    }
    this.focus = index;
  }

  /** Moves the focus (and a single selection) to `index`, within [0, count). */
  moveTo(index: number, count: number, extend = false): void {
    if (count === 0) return;
    const clamped = Math.max(0, Math.min(count - 1, index));
    this.click(clamped, { shift: extend });
  }

  clear(): void {
    this.selected.clear();
    this.focus = null;
    this.anchor = null;
  }
}
