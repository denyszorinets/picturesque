import type { Settings } from "../api";

/** The last part of a folder path, for labels. */
export function folderName(path: string): string {
  const parts = path.split(/[\\/]+/).filter((p) => p !== "");
  return parts[parts.length - 1] ?? path;
}

/** What the header says is open: a folder culled in place, or the library. */
export function sessionLabel(settings: Pick<Settings, "open_folder">): string {
  return settings.open_folder ? folderName(settings.open_folder) : "Library";
}
