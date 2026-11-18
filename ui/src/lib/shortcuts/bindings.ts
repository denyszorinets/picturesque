/** Letter shortcuts share one map across the grid, loupe and comparison. */
export const ACTIONS = [
  { id: "pick", label: "Pick", key: "p" },
  { id: "reject", label: "Reject", key: "x" },
  { id: "unflag", label: "Unflag", key: "u" },
  { id: "best", label: "Best in series", key: "b" },
  { id: "compare", label: "Compare photos", key: "c" },
  { id: "split", label: "Split series", key: "s" },
  { id: "join", label: "Join series", key: "j" },
  { id: "zoom", label: "Zoom", key: "z" },
  { id: "info", label: "Photo details", key: "i" },
  { id: "raw", label: "Switch RAW / JPEG", key: "r" },
  { id: "compareBest", label: "Compare with best", key: "v" },
  { id: "panes", label: "Compare layout (2 / 4)", key: "l" },
] as const;

export type ShortcutAction = (typeof ACTIONS)[number]["id"];
export type Bindings = Record<ShortcutAction, string>;
export const DEFAULTS = Object.fromEntries(
  ACTIONS.map(({ id, key }) => [id, key]),
) as Bindings;
export const ONE_HAND: Bindings = {
  ...DEFAULTS,
  pick: "a",
  reject: "d",
  unflag: "f",
};

export function normalize(bindings: Bindings): Bindings {
  return Object.fromEntries(
    ACTIONS.map(({ id }) => [id, bindings[id].trim().toLowerCase()]),
  ) as Bindings;
}

export function bindingError(bindings: Bindings): string | null {
  const used = new Map<string, string>();
  for (const { id, label } of ACTIONS) {
    const key = bindings[id].trim().toLowerCase();
    if (!/^\p{L}$/u.test(key)) return `${label}: choose a single letter.`;
    const previous = used.get(key);
    if (previous)
      return `${key.toUpperCase()} is assigned to both ${previous} and ${label}.`;
    used.set(key, label);
  }
  return null;
}

/** Translate a configured letter to the existing handlers' canonical key.
 * Unassigned letters do nothing; navigation keys pass through unchanged.
 * Call only after handling Ctrl/Command shortcuts such as Undo. */
export function resolveKey(key: string, bindings: Bindings): string {
  if (!/^\p{L}$/u.test(key)) return key;
  return (
    ACTIONS.find(({ id }) => bindings[id] === key.toLowerCase())?.key ?? ""
  );
}

export const STORAGE_KEY = "picturesque.keyboard-shortcuts.v1";

export function readBindings(storage: Pick<Storage, "getItem">): Bindings {
  try {
    const raw: unknown = JSON.parse(storage.getItem(STORAGE_KEY) ?? "{}");
    if (!raw || typeof raw !== "object" || Array.isArray(raw))
      return { ...DEFAULTS };
    const bindings = { ...DEFAULTS };
    for (const { id } of ACTIONS) {
      const value: unknown = (raw as Record<string, unknown>)[id];
      if (value !== undefined) {
        if (typeof value !== "string") return { ...DEFAULTS };
        bindings[id] = value;
      }
    }
    return bindingError(bindings) ? { ...DEFAULTS } : normalize(bindings);
  } catch {
    return { ...DEFAULTS };
  }
}
