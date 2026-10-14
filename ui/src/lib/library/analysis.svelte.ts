import type { AnalysisStatus, AppEvent } from "../api";

/** Backend calls analysis progress needs (injectable for tests). */
export interface AnalysisApi {
  analysisStatus(): Promise<AnalysisStatus>;
  prioritizeAnalysis(photoIds: number[]): Promise<AnalysisStatus>;
}

/** Wait this long after scrolling stops before asking for priority. */
export const PRIORITY_DELAY_MS = 300;

/**
 * Background analysis progress, and asking the backend to analyze the
 * photos on screen first.
 */
export class AnalysisProgress {
  status = $state<AnalysisStatus | null>(null);

  private timer: ReturnType<typeof setTimeout> | undefined;
  private lastKey = "";

  constructor(private readonly api: AnalysisApi) {}

  /** Photos still waiting for analysis, or 0 when unknown or done. */
  get remaining(): number {
    return this.status ? this.status.total - this.status.analyzed : 0;
  }

  async load(): Promise<void> {
    try {
      this.status = await this.api.analysisStatus();
    } catch {
      // Progress is informational; keep the last known status.
    }
  }

  handle(event: AppEvent): void {
    if (event.type === "analysis_progress") {
      const { type: _, ...status } = event;
      this.status = status;
    }
  }

  /** Called with the photos on screen; sends them once scrolling settles. */
  visible(photoIds: number[]): void {
    clearTimeout(this.timer);
    if (photoIds.length === 0 || this.remaining === 0) return;
    const key = photoIds.join(",");
    if (key === this.lastKey) return;
    this.timer = setTimeout(() => {
      this.lastKey = key;
      this.api.prioritizeAnalysis(photoIds).then(
        (status) => (this.status = status),
        () => (this.lastKey = ""),
      );
    }, PRIORITY_DELAY_MS);
  }

  dispose(): void {
    clearTimeout(this.timer);
  }
}
