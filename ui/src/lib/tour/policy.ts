/** What decides whether the culling tour shows (it also needs photos on
 * screen, which the library checks). */
export interface TourState {
  /** Whether the tour was seen; null until settings are loaded. */
  tourDone: boolean | null;
  /** Asked for again from Settings. */
  replay: boolean;
  hasLibrary: boolean;
  setupOpen: boolean;
  helpOpen: boolean;
  onLibraryTab: boolean;
}

export function tourWanted(s: TourState): boolean {
  if (s.tourDone === null || s.setupOpen || s.helpOpen || !s.onLibraryTab)
    return false;
  return s.replay || (!s.tourDone && s.hasLibrary);
}
