import { isTauri } from "@tauri-apps/api/core";

/**
 * Opens a web page in the user's browser: the desktop app's web view
 * does not open links itself.
 */
export async function openExternal(url: string): Promise<void> {
  if (isTauri()) {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  } else {
    window.open(url, "_blank", "noopener,noreferrer");
  }
}
