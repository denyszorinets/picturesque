import type { AppEvent, SidecarStatus } from "../api";

/** Backend calls sidecar status needs (injectable for tests). */
export interface SidecarApi {
  sidecarStatus(): Promise<SidecarStatus>;
}

/** What the toolbar says about sidecars. */
export interface SidecarLabel {
  text: string;
  tone: "ok" | "busy" | "error";
  title: string;
}

/** Joins a library-relative `/`-separated path onto the library folder. */
export function joinPath(folder: string, relative: string): string {
  const sep = folder.includes("\\") && !folder.includes("/") ? "\\" : "/";
  const base = folder.endsWith(sep) ? folder.slice(0, -1) : folder;
  return `${base}${sep}${relative.split("/").join(sep)}`;
}

/** Whether decisions have reached their XMP sidecars. */
export class SidecarProgress {
  status = $state<SidecarStatus | null>(null);

  constructor(private readonly api: SidecarApi) {}

  async load(): Promise<void> {
    try {
      this.status = await this.api.sidecarStatus();
    } catch {
      // Informational; keep the last known status.
    }
  }

  handle(event: AppEvent): void {
    if (event.type === "sidecar_progress") {
      const { type: _, ...status } = event;
      this.status = status;
    }
  }

  get label(): SidecarLabel | null {
    const s = this.status;
    if (!s?.folder) return null;
    const where = `Ratings and flags are saved in XMP sidecar files next to each photo in ${s.folder}`;
    if (s.failed > 0) {
      return {
        text: `XMP: ${s.failed} not saved`,
        tone: "error",
        title: `${s.error ?? "Some sidecars could not be written"}. Picturesque tries again after the next change and at startup.`,
      };
    }
    if (s.pending > 0) {
      return { text: "Saving to XMP…", tone: "busy", title: where };
    }
    return { text: "Saved to XMP ✓", tone: "ok", title: where };
  }
}
