import { describe, expect, it } from "vitest";
import { Selection } from "./selection.svelte";

const list = (s: Selection) => [...s.selected].sort((a, b) => a - b);

describe("Selection", () => {
  it("selects one photo on a plain click", () => {
    const s = new Selection();
    s.click(3);
    s.click(5);

    expect(list(s)).toEqual([5]);
    expect(s.focus).toBe(5);
  });

  it("extends a range from the anchor with shift", () => {
    const s = new Selection();
    s.click(4);
    s.click(7, { shift: true });
    expect(list(s)).toEqual([4, 5, 6, 7]);

    s.click(2, { shift: true });
    expect(list(s)).toEqual([2, 3, 4]);
  });

  it("toggles with ctrl/cmd and adds ranges to the selection", () => {
    const s = new Selection();
    s.click(1);
    s.click(3, { toggle: true });
    s.click(1, { toggle: true });
    expect(list(s)).toEqual([3]);

    // The last ctrl/cmd-clicked photo is the anchor, as in file managers.
    s.click(5, { shift: true, toggle: true });
    expect(list(s)).toEqual([1, 2, 3, 4, 5]);
  });

  it("moves within bounds and can extend", () => {
    const s = new Selection();
    s.moveTo(-3, 10);
    expect(s.focus).toBe(0);
    s.moveTo(2, 10, true);
    expect(list(s)).toEqual([0, 1, 2]);
    s.moveTo(99, 10);
    expect(list(s)).toEqual([9]);
  });

  it("clears", () => {
    const s = new Selection();
    s.click(1);
    s.clear();

    expect(s.size).toBe(0);
    expect(s.focus).toBeNull();
  });
});
