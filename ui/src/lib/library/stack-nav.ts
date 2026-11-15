/**
 * Moving through photos in the loupe while series are stacked: each stack
 * is one entry of the list, and the loupe steps through the stack's
 * members (in capture order) before moving to the next entry.
 */

/** Where a stack is entered: its first or last member, or the photo the
 * list shows for it (its best). */
export type Entry = "first" | "last" | "shown";

/** The result of a step. */
export type Step =
  /** Show another member of the current stack. */
  | { kind: "member"; id: number }
  /** Open another entry of the list, starting at `entry`. */
  | { kind: "entry"; index: number; entry: Entry }
  /** Nowhere to go. */
  | null;

export interface StepInput {
  /** The list entry the loupe is on. */
  index: number;
  /** Entries in the list. */
  total: number;
  /** Members of the entry's stack in capture order; empty for a single
   * photo, or while not stacked. */
  members: number[];
  /** The photo shown. */
  currentId: number | null;
  /** +1 forward, -1 back. */
  delta: 1 | -1;
  /** Skip the rest of the stack and go to the next or previous entry. */
  wholeStack?: boolean;
}

/** Where a step forward or back goes. */
export function step({
  index,
  total,
  members,
  currentId,
  delta,
  wholeStack = false,
}: StepInput): Step {
  const at = currentId === null ? -1 : members.indexOf(currentId);
  if (!wholeStack && at >= 0) {
    const next = at + delta;
    if (next >= 0 && next < members.length) {
      const id = members[next];
      if (id !== undefined) return { kind: "member", id };
    }
  }
  const nextIndex = index + delta;
  if (nextIndex < 0 || nextIndex >= total) return null;
  // Going forward starts the next stack at its first photo; going back
  // ends the previous one at its last, so every frame is seen in order.
  // Jumping whole stacks shows each stack's best.
  const entry: Entry = wholeStack ? "shown" : delta > 0 ? "first" : "last";
  return { kind: "entry", index: nextIndex, entry };
}

/** The member to show when entering a stack, or `null` for the photo the
 * list shows. */
export function entryMember(members: number[], entry: Entry): number | null {
  if (members.length < 2 || entry === "shown") return null;
  return (entry === "first" ? members[0] : members[members.length - 1]) ?? null;
}
