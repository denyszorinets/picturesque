import { describe, expect, it } from "vitest";
import { CompareState } from "./compare-state.svelte";

describe("CompareState", () => {
  it("starts on the given photo with 4 panes for more than two photos", () => {
    const c = new CompareState([10, 11, 12, 13, 14, 15], 14);
    expect(c.panes).toBe(4);
    expect(c.activeId).toBe(14);
    // The window scrolls only as far as needed.
    expect(c.visible).toEqual([1, 2, 3, 4]);
    expect(new CompareState([1, 2]).panes).toBe(2);
  });

  it("scrolls the window to follow the active photo", () => {
    const c = new CompareState([1, 2, 3, 4, 5], 1, 2);
    expect(c.visible).toEqual([0, 1]);
    c.move(1);
    expect(c.visible).toEqual([0, 1]);
    c.move(1);
    expect(c.visible).toEqual([1, 2]);
    c.moveTo(99);
    expect(c.activeId).toBe(5);
    expect(c.visible).toEqual([3, 4]);
    c.move(-2);
    expect(c.visible).toEqual([2, 3]);
    c.move(-10);
    expect(c.active).toBe(0);
    expect(c.visible).toEqual([0, 1]);
  });

  it("switches between 2 and 4 panes, keeping the active photo shown", () => {
    const c = new CompareState([1, 2, 3, 4, 5], 5, 2);
    expect(c.visible).toEqual([3, 4]);
    c.togglePanes();
    expect(c.visible).toEqual([1, 2, 3, 4]);
    c.moveTo(1);
    c.togglePanes();
    expect(c.visible).toEqual([1, 2]);
    expect(c.activeId).toBe(2);
  });

  it("zooms around a point and pans within the photo", () => {
    const c = new CompareState([1, 2]);
    c.toggleZoom({ x: 0.2, y: 0.9 });
    expect(c.zoomed).toBe(true);
    expect(c.center).toEqual({ x: 0.2, y: 0.9 });
    c.pan(-0.5, 0.5);
    expect(c.center).toEqual({ x: 0, y: 1 });
    c.toggleZoom();
    expect(c.zoomed).toBe(false);
  });

  it("handles an unknown photo and an empty list", () => {
    expect(new CompareState([1, 2], 9).activeId).toBe(1);
    const empty = new CompareState([]);
    empty.move(1);
    expect(empty.visible).toEqual([]);
    expect(empty.activeId).toBeUndefined();
  });
});
