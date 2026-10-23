import {
  imageUrl,
  type ImageSize,
  type PairView,
  type PhotoSummary,
} from "../api";

/**
 * Which file of a RAW+JPEG pair the grid, loupe and compare show: the
 * `pair_view` setting, kept current by the app.
 */
export const pairSetting = $state<{ view: PairView }>({ view: "image" });

/** Whether the photo has both a RAW and a rendered image (a JPEG). */
export function isPair(
  photo: Pick<PhotoSummary, "has_raw" | "has_image">,
): boolean {
  return photo.has_raw && photo.has_image;
}

/**
 * URL of the photo's image showing `view` (default: the setting). Only
 * pairs name a view, so other photos keep one URL whatever is chosen.
 */
export function photoUrl(
  photo: PhotoSummary,
  size: ImageSize,
  view: PairView = pairSetting.view,
): string {
  return imageUrl(photo.id, size, isPair(photo) ? view : undefined);
}

/** `JPEG` for `a.jpg`, `HEIC` for `a.heic`: the format of the photo's rendered image. */
export function imageFormat(fileName: string): string {
  const ext = fileName.includes(".")
    ? fileName.slice(fileName.lastIndexOf(".") + 1).toUpperCase()
    : "";
  return ext === "JPG" || ext === "JPEG" || ext === "" ? "JPEG" : ext;
}

/** Short name of the file `view` shows, for labels: `RAW` or `JPEG`, `HEIC`, … */
export function viewLabel(photo: PhotoSummary, view: PairView): string {
  const raw = isPair(photo) ? view === "raw" : !photo.has_image;
  return raw ? "RAW" : imageFormat(photo.file_name);
}
