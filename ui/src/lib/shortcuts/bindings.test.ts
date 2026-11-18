import { afterEach, describe, expect, it, vi } from "vitest";
import {
  ACTIONS,
  DEFAULTS,
  ONE_HAND,
  STORAGE_KEY,
  bindingError,
  readBindings,
  resolveKey,
} from "./bindings";
import {
  saveBindings,
  shortcuts,
  shortcutLabel,
  ignorePhotoKey,
} from "./preferences.svelte";

afterEach(() => {
  vi.restoreAllMocks();
  localStorage.clear();
  shortcuts.bindings = { ...DEFAULTS };
});

describe("keyboard preferences", () => {
  it("retains defaults and handles swaps without leaving the old key active", () => {
    for (const { key } of ACTIONS)
      expect(resolveKey(key.toUpperCase(), DEFAULTS)).toBe(key);
    const swapped = { ...DEFAULTS, pick: "z", zoom: "p" };
    expect(bindingError(swapped)).toBeNull();
    expect(resolveKey("Z", swapped)).toBe("p");
    expect(resolveKey("p", swapped)).toBe("z");
    expect(resolveKey("p", ONE_HAND)).toBe("");
    expect(resolveKey("ArrowUp", ONE_HAND)).toBe("ArrowUp");
  });

  it("rejects duplicates and reserved or multi-key bindings", () => {
    expect(bindingError({ ...DEFAULTS, pick: "X" })).toContain(
      "both Pick and Reject",
    );
    for (const key of ["", "1", "ArrowUp", "Ctrl+a", "ab", "?"])
      expect(bindingError({ ...DEFAULTS, pick: key })).toContain(
        "single letter",
      );
    expect(bindingError({ ...DEFAULTS, pick: "å" })).toBeNull();
  });

  it("persists normalized preferences and recovers invalid or partial storage", () => {
    expect(saveBindings({ ...ONE_HAND, pick: "A" })).toBeNull();
    expect(readBindings(localStorage)).toEqual(ONE_HAND);
    expect(shortcutLabel("P")).toBe("A");
    for (const raw of [
      "bad json",
      "null",
      "[]",
      '{"pick":3}',
      '{"pick":"x"}',
    ]) {
      localStorage.setItem(STORAGE_KEY, raw);
      expect(readBindings(localStorage)).toEqual(DEFAULTS);
    }
    localStorage.setItem(STORAGE_KEY, '{"pick":"A"}');
    expect(readBindings(localStorage)).toEqual({ ...DEFAULTS, pick: "a" });
  });

  it("keeps active bindings unchanged when storage fails", () => {
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("full");
    });
    expect(saveBindings(ONE_HAND)).toContain("Could not save");
    expect(shortcuts.bindings).toEqual(DEFAULTS);
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("denied");
    });
    expect(readBindings(localStorage)).toEqual(DEFAULTS);
  });

  it("ignores composition and photo keys behind dialogs", () => {
    expect(
      ignorePhotoKey(
        new KeyboardEvent("keydown", { key: "a", isComposing: true }),
      ),
    ).toBe(true);
    const dialog = document.createElement("div");
    dialog.setAttribute("role", "dialog");
    document.body.append(dialog);
    try {
      expect(ignorePhotoKey(new KeyboardEvent("keydown", { key: "a" }))).toBe(
        true,
      );
    } finally {
      dialog.remove();
    }
  });
});
