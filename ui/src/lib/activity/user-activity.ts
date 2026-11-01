/** At most one report per this long. */
export const ACTIVITY_INTERVAL_MS = 30_000;

/**
 * Calls `report` on pointer and key activity, at most once per interval.
 * Failures are ignored. Returns a function that stops listening.
 */
export function trackActivity(
  report: () => Promise<unknown>,
  target: Pick<Window, "addEventListener" | "removeEventListener"> = window,
  now: () => number = Date.now,
): () => void {
  let last = -Infinity;
  const onActivity = () => {
    const t = now();
    if (t - last < ACTIVITY_INTERVAL_MS) return;
    last = t;
    try {
      void report().catch(() => {});
    } catch {
      // Nothing to do: activity is a hint.
    }
  };
  const events = ["pointermove", "pointerdown", "keydown", "wheel"] as const;
  for (const name of events)
    target.addEventListener(name, onActivity, { passive: true });
  return () => {
    for (const name of events) target.removeEventListener(name, onActivity);
  };
}
