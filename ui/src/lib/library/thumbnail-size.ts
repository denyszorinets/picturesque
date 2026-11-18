/** A view preference only; never changes photos or the library query. */
export const MIN_THUMBNAIL = 120;
export const MAX_THUMBNAIL = 360;
export const DEFAULT_THUMBNAIL = 180;
const KEY = "picturesque.library.thumbnailSize";

export function thumbnailSize(value: number): number {
  return Number.isFinite(value)
    ? Math.max(
        MIN_THUMBNAIL,
        Math.min(MAX_THUMBNAIL, Math.round(value / 10) * 10),
      )
    : DEFAULT_THUMBNAIL;
}

export function loadThumbnailSize(): number {
  try {
    const saved = localStorage.getItem(KEY);
    return saved === null ? DEFAULT_THUMBNAIL : thumbnailSize(Number(saved));
  } catch {
    return DEFAULT_THUMBNAIL;
  }
}

export function saveThumbnailSize(size: number): void {
  try {
    localStorage.setItem(KEY, String(thumbnailSize(size)));
  } catch {
    // The control still works when storage is unavailable.
  }
}
