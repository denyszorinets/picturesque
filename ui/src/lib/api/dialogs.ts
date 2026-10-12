import { isTauri } from "@tauri-apps/api/core";

/**
 * Asks the user for folders with the native dialog. Returns `null` when no
 * native dialog is available (the UI running in a plain browser), in which
 * case the caller offers a path field instead.
 */
export async function pickFolders(options: {
  multiple: boolean;
  title: string;
}): Promise<string[] | null> {
  if (!isTauri()) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({
    directory: true,
    multiple: options.multiple,
    title: options.title,
  });
  if (picked === null) return [];
  return Array.isArray(picked) ? picked : [picked];
}

/** Whether a native folder dialog is available. */
export function canPickFolders(): boolean {
  return isTauri();
}
