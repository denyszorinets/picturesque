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

/** Whether files can be shown in the system file manager. */
export function canReveal(): boolean {
  return isTauri();
}

/** Opens the folder holding `path` in the system file manager, selecting it. */
export async function revealInFolder(path: string): Promise<void> {
  if (!isTauri()) return;
  const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
  await revealItemInDir(path);
}
