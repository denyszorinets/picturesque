import type { PhotoSummary } from "../api";

type Flag = PhotoSummary["flag"];

/** Where a key is pressed: arrows navigate the grid, but not up and down
 * in the loupe or compare, where they pick and reject instead. */
export type KeyView = "grid" | "loupe" | "compare";

const LETTERS: Record<string, Flag> = {
  p: "pick",
  x: "reject",
  u: "unflagged",
};

const ARROWS: Record<string, Flag> = {
  ArrowUp: "pick",
  ArrowDown: "reject",
};

/** The flag a key sets, if any (`key` is `KeyboardEvent.key`). */
export function flagForKey(key: string, view: KeyView): Flag | undefined {
  const letter = LETTERS[key.toLowerCase()];
  if (letter) return letter;
  return view === "grid" ? undefined : ARROWS[key];
}
