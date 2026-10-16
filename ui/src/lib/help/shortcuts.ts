/** One keyboard shortcut: keys pressed together, and what they do. */
export interface Shortcut {
  keys: string[];
  action: string;
}

export interface ShortcutGroup {
  title: string;
  /** Where the keys work, when not everywhere. */
  note?: string;
  items: Shortcut[];
}

/**
 * Every keyboard shortcut, as handled by LibraryView, Loupe and Compare.
 * Keep in step with those handlers.
 */
export const SHORTCUTS: ShortcutGroup[] = [
  {
    title: "Culling",
    note: "On the selected photos, or the photo in the loupe.",
    items: [
      { keys: ["P"], action: "Pick" },
      { keys: ["X"], action: "Reject" },
      { keys: ["U"], action: "Unflag" },
      { keys: ["1–5"], action: "Rate 1 to 5 stars" },
      { keys: ["0"], action: "Clear the rating" },
      { keys: ["Shift", "key"], action: "Decide, then go to the next photo" },
      { keys: ["Ctrl", "Z"], action: "Undo" },
      { keys: ["Ctrl", "Shift", "Z"], action: "Redo (also Ctrl+Y)" },
    ],
  },
  {
    title: "Moving around",
    items: [
      { keys: ["←", "→", "↑", "↓"], action: "Move through the photos" },
      { keys: ["Shift", "arrow"], action: "Extend the selection" },
      { keys: ["Home"], action: "First photo (End: last)" },
      { keys: ["Enter"], action: "Open the photo in the loupe" },
      { keys: ["Esc"], action: "Clear the selection" },
      { keys: ["/"], action: "Search photos" },
    ],
  },
  {
    title: "Series",
    items: [
      { keys: ["C"], action: "Compare the selection, or the photo's series" },
      { keys: ["B"], action: "Make this the best photo of its series" },
      { keys: ["S"], action: "Split the series before this photo" },
      { keys: ["J"], action: "Join this photo to the one before it" },
    ],
  },
  {
    title: "Photo (loupe)",
    items: [
      { keys: ["←", "→"], action: "Previous or next photo" },
      { keys: ["Z"], action: "Zoom to 100%" },
      { keys: ["I"], action: "Show or hide the photo's details" },
      { keys: ["Esc"], action: "Back to the timeline" },
    ],
  },
  {
    title: "Compare",
    items: [
      { keys: ["←", "→"], action: "Previous or next photo" },
      { keys: ["L"], action: "Show 2 or 4 photos" },
      { keys: ["Z"], action: "Zoom all photos together" },
      { keys: ["Shift", "arrow"], action: "Pan while zoomed" },
      { keys: ["P", "X", "B"], action: "Pick, reject, or choose the best" },
      { keys: ["Esc"], action: "Back to the timeline" },
    ],
  },
  {
    title: "Anywhere",
    items: [{ keys: ["?"], action: "Show these shortcuts and help" }],
  },
];

const DOCS = "https://denyszorinets.github.io/picturesque/docs";

/** Guides opened in the browser from Help. */
export const GUIDES: { label: string; url: string }[] = [
  {
    label: "Your first import",
    url: `${DOCS}/getting-started/first-import.html`,
  },
  { label: "Your first cull", url: `${DOCS}/getting-started/first-cull.html` },
  { label: "Documentation", url: `${DOCS}/` },
  {
    label: "Report a problem",
    url: "https://github.com/denyszorinets/picturesque/issues",
  },
];
