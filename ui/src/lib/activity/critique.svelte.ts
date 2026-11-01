import type { AppEvent, BackgroundCritiqueStatus } from "../api";

/** Backend calls critique progress needs (injectable for tests). */
export interface CritiqueApi {
  backgroundCritiqueStatus(): Promise<BackgroundCritiqueStatus>;
}

/** Background critique progress: asked once, then pushed by events. */
export class CritiqueProgress {
  status = $state<BackgroundCritiqueStatus | null>(null);

  constructor(private readonly api: CritiqueApi) {}

  async load(): Promise<void> {
    try {
      this.status = await this.api.backgroundCritiqueStatus();
    } catch {
      // Progress is informational; keep the last known status.
    }
  }

  handle(event: AppEvent): void {
    if (event.type === "background_critique_progress") {
      const { type: _, ...status } = event;
      this.status = status;
    }
  }
}
