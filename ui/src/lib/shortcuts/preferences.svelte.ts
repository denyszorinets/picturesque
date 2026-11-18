import {
  ACTIONS,
  DEFAULTS,
  STORAGE_KEY,
  bindingError,
  normalize,
  readBindings,
  resolveKey,
  type Bindings,
} from "./bindings";

function load(): Bindings {
  try {
    return readBindings(localStorage);
  } catch {
    return { ...DEFAULTS };
  }
}

export const shortcuts = $state<{ bindings: Bindings }>({ bindings: load() });

/** A failed write must not silently activate preferences that cannot persist. */
export function saveBindings(bindings: Bindings): string | null {
  const error = bindingError(bindings);
  if (error) return error;
  const next = normalize(bindings);
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  } catch {
    return "Could not save shortcuts on this device. Your previous shortcuts are still active.";
  }
  shortcuts.bindings = next;
  return null;
}

export function shortcutKey(key: string): string {
  return resolveKey(key, shortcuts.bindings);
}

/** The displayed letter for an existing default shortcut. */
export function shortcutLabel(defaultKey: string): string {
  const action = ACTIONS.find(({ key }) => key === defaultKey.toLowerCase());
  return action ? shortcuts.bindings[action.id].toUpperCase() : defaultKey;
}

/** Do not run photo shortcuts while typing, composing, or behind a modal. */
export function ignorePhotoKey(event: KeyboardEvent): boolean {
  const target = event.target instanceof Element ? event.target : null;
  return (
    event.defaultPrevented ||
    event.isComposing ||
    !!target?.closest("input, select, textarea, [contenteditable]") ||
    !!document.querySelector(
      '[role="dialog"]:not([data-testid="loupe"]):not([data-testid="compare"])',
    )
  );
}
