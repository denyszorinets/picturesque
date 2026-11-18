import { shortcutLabel } from "../shortcuts/preferences.svelte";

/** One step of a tour: what it points at and what it says. */
export interface TourStep {
  /** CSS selector of the control described; the step is centred when it
   * is not on screen. */
  target: string;
  title: string;
  text: string;
}

/** The culling tour, shown once the library has photos. */
export const CULLING_TOUR: TourStep[] = [
  {
    target: "[data-testid=timeline]",
    title: "Your photos",
    text: "Every photo, newest day first. Photos taken in a burst are framed together as a series. These are the copies in your library folder; the originals are never touched.",
  },
  {
    target: "button.tile",
    title: "Pick, reject, rate",
    get text() {
      return `Click a photo, then press ${shortcutLabel("p")} to pick, ${shortcutLabel("x")} to reject or 1–5 to give stars. Hold Shift to move on to the next photo. Ctrl+Z undoes.`;
    },
  },
  {
    target: "button.tile",
    title: "Look closer",
    get text() {
      return `Double-click a photo or press Enter to open it large. ${shortcutLabel("z")} zooms to 100%, ${shortcutLabel("i")} shows its details, ← → go through the photos.`;
    },
  },
  {
    target: "[data-testid=open-compare]",
    title: "Compare a series",
    get text() {
      return `Press ${shortcutLabel("c")} to see the selected photos, or a photo's series, side by side. Keep the best with ${shortcutLabel("p")} or ${shortcutLabel("b")}, reject the rest with ${shortcutLabel("x")}.`;
    },
  },
  {
    target: "[data-testid=toggle-filters]",
    title: "Filter",
    text: "Show only unflagged photos, picks, a rating, sharp photos or one camera, so you can cull in passes.",
  },
  {
    target: "[data-testid=open-export]",
    title: "Export your picks",
    text: "Copy the picks to a folder, as originals or resized JPEGs, ready to share or edit.",
  },
  {
    target: "[data-testid=open-help]",
    title: "Need a hand?",
    text: "Press ? any time for every keyboard shortcut and the guides. You can replay this tour from Settings.",
  },
];
