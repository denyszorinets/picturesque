import type { ImportPlanView, ImportStatus } from "../api";
import { ApiError } from "../api";

/** The backend calls the import flow needs (injectable for tests). */
export interface ImportApi {
  planImport(sources: string[]): Promise<ImportStatus>;
  getImport(runId: number): Promise<ImportPlanView>;
  importStatus(runId: number): Promise<ImportStatus>;
  startImport(runId: number): Promise<ImportStatus>;
  cancelImport(runId: number): Promise<ImportStatus>;
}

/**
 * - `choose`: picking source folders
 * - `planning`: scanning and reading metadata
 * - `review`: plan ready, nothing copied
 * - `copying`: copying in progress
 * - `done`: copying finished, was cancelled, or failed
 */
export type FlowStep = "choose" | "planning" | "review" | "copying" | "done";

const RUNNING = new Set(["planning", "copying"]);

/** State machine for one import, driven by API calls and progress events. */
export class ImportFlow {
  step = $state<FlowStep>("choose");
  sources = $state<string[]>([]);
  status = $state<ImportStatus | null>(null);
  plan = $state<ImportPlanView | null>(null);
  error = $state<string | null>(null);
  busy = $state(false);

  constructor(private readonly api: ImportApi) {}

  get runId(): number | null {
    return this.status?.run_id ?? null;
  }

  /** Progress within the running phase, 0–1, or null if unknown. */
  get fraction(): number | null {
    const s = this.status;
    if (!s || s.total === 0) return null;
    return Math.min(1, s.done / s.total);
  }

  addSource(path: string): void {
    const p = path.trim();
    if (p && !this.sources.includes(p)) this.sources = [...this.sources, p];
  }

  removeSource(path: string): void {
    this.sources = this.sources.filter((s) => s !== path);
  }

  async startPlanning(): Promise<void> {
    await this.run(async () => {
      this.plan = null;
      this.status = await this.api.planImport(this.sources);
      this.step = "planning";
    });
  }

  async startCopying(): Promise<void> {
    const id = this.runId;
    if (id === null) return;
    await this.run(async () => {
      this.status = await this.api.startImport(id);
      this.step = "copying";
    });
  }

  async cancel(): Promise<void> {
    const id = this.runId;
    if (id === null) return;
    await this.run(async () => {
      this.status = await this.api.cancelImport(id);
    });
  }

  /** Applies a progress event (ignored if it is for another run). */
  async onStatus(status: ImportStatus): Promise<void> {
    if (status.run_id !== this.runId) return;
    this.status = status;
    await this.settle();
  }

  /** Polls the status; used as a fallback while a phase is running. */
  async refresh(): Promise<void> {
    const id = this.runId;
    if (id === null || !RUNNING.has(this.step)) return;
    await this.run(async () => {
      this.status = await this.api.importStatus(id);
      await this.settle();
    });
  }

  reset(): void {
    this.step = "choose";
    this.status = null;
    this.plan = null;
    this.error = null;
  }

  /** Moves to the next step when the backend reports a phase has ended. */
  private async settle(): Promise<void> {
    const s = this.status;
    if (!s || RUNNING.has(s.state)) return;
    const id = s.run_id;
    if (this.step === "planning") {
      this.plan = await this.api.getImport(id);
      if (s.state === "planned") {
        this.step = "review";
      } else {
        // Planning was cancelled or failed.
        this.error =
          s.error ?? (s.state === "cancelled" ? null : "Planning failed.");
        this.step = "choose";
      }
    } else if (this.step === "copying") {
      this.plan = await this.api.getImport(id);
      this.step = "done";
    }
  }

  private async run(action: () => Promise<void>): Promise<void> {
    this.busy = true;
    this.error = null;
    try {
      await action();
    } catch (e) {
      this.error = ApiError.from(e).message;
    } finally {
      this.busy = false;
    }
  }
}
