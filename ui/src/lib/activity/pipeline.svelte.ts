import {
  ApiError,
  type AppEvent,
  type ImportSource,
  type JobFilter,
  type JobView,
  type LogEntry,
  type LogLevel,
  type OriginalsChoice,
  type PipelineStatus,
} from "../api";

/** Backend calls the import pipeline view needs (injectable for tests). */
export interface PipelineApi {
  pipelineStatus(): Promise<PipelineStatus>;
  listImportSources(): Promise<ImportSource[]>;
  addImportSource(
    path: string,
    watch?: boolean,
    deleteOriginals?: OriginalsChoice,
  ): Promise<ImportSource>;
  setImportSourceDeleteOriginals(
    id: number,
    choice: OriginalsChoice,
  ): Promise<ImportSource>;
  removeImportSource(id: number): Promise<ImportSource[]>;
  setImportSourceWatch(id: number, watch: boolean): Promise<ImportSource>;
  scanImportSources(id?: number): Promise<PipelineStatus>;
  pipelineJobs(args: {
    filter?: JobFilter;
    limit?: number;
    before?: number;
  }): Promise<JobView[]>;
  pipelineLog(args: {
    min_level?: LogLevel;
    limit?: number;
    before?: number;
  }): Promise<LogEntry[]>;
  retryFailedImports(ids?: number[]): Promise<PipelineStatus>;
}

/** Rows fetched per page of photos or log entries. */
export const PAGE = 100;

/** Whether the pipeline has work in progress. */
export function isBusy(status: PipelineStatus | null): boolean {
  return (
    !!status && (status.queued > 0 || status.active !== null || status.scanning)
  );
}

export type StepState = "done" | "active" | "pending" | "skipped" | "failed";

export interface Step {
  label: string;
  state: StepState;
  /** Extra detail, for example analysis progress. */
  detail?: string;
}

export const STEP_LABELS = [
  "Found",
  "Read",
  "Copied",
  "Verified",
  "Catalogued",
  "Thumbnail",
  "Analyzed",
] as const;

/** Where a photo is, step by step, as the activity view shows it. */
export function steps(job: JobView): Step[] {
  const reached: Record<JobView["stage"], number> = {
    queued: 1,
    reading: 1,
    copying: 2,
    imported: 5,
    duplicate: 2,
    failed: 0,
  };
  const done = reached[job.stage];
  const result: Step[] = STEP_LABELS.map((label, i) => ({
    label,
    state: i < done ? "done" : "pending",
  }));
  const set = (i: number, state: StepState, detail?: string) => {
    result[i] = { label: STEP_LABELS[i] ?? "", state, detail };
  };
  switch (job.stage) {
    case "reading":
      set(1, "active");
      break;
    case "copying":
      set(2, "active");
      set(3, "active");
      break;
    case "duplicate":
      for (let i = 2; i < result.length; i++) set(i, "skipped");
      break;
    case "failed": {
      // Reading covers Read; copying covers Copied and Verified.
      const at = job.failed_stage === "copying" ? 2 : 1;
      for (let i = 0; i < result.length; i++)
        set(i, i < at ? "done" : i === at ? "failed" : "skipped");
      break;
    }
    case "imported": {
      set(5, job.thumbnail ? "done" : "active");
      const [analyzed, total] = job.analysis ?? [0, 0];
      if (total === 0) set(6, "skipped", "no analyzers");
      else if (analyzed >= total) set(6, "done");
      else set(6, "active", `${analyzed}/${total}`);
      break;
    }
    case "queued":
      break;
  }
  return result;
}

/**
 * The import pipeline: its status (for the header indicator, always
 * live), and the sources, photos and log shown in the activity view
 * (loaded while it is visible).
 */
export class Pipeline {
  status = $state<PipelineStatus | null>(null);
  sources = $state<ImportSource[]>([]);
  jobs = $state<JobView[]>([]);
  jobsComplete = $state(false);
  log = $state<LogEntry[]>([]);
  logComplete = $state(false);
  filter = $state<JobFilter>("all");
  logLevel = $state<LogLevel>("info");
  error = $state<string | null>(null);

  private generation = 0;

  constructor(private readonly api: PipelineApi) {}

  get busy(): boolean {
    return isBusy(this.status);
  }

  /** Follows a backend event; returns whether it was pipeline progress. */
  handle(event: AppEvent): boolean {
    if (event.type !== "pipeline_progress") return false;
    const { type: _, ...status } = event;
    this.status = status;
    return true;
  }

  async loadStatus(): Promise<void> {
    try {
      this.status = await this.api.pipelineStatus();
    } catch {
      // Informational; keep the last known status.
    }
  }

  /**
   * Sources equal to the ones shown stay the same objects, so a refresh
   * while a dropdown is open does not rebuild it (and close it).
   */
  private keepUnchanged(next: ImportSource[]): ImportSource[] {
    return next.map((s) => {
      const old = this.sources.find((o) => o.id === s.id);
      return old && JSON.stringify(old) === JSON.stringify(s) ? old : s;
    });
  }

  /** Reloads sources, status, and the photos and log loaded so far. */
  async refresh(): Promise<void> {
    const generation = ++this.generation;
    try {
      const [status, sources, jobs, log] = await Promise.all([
        this.api.pipelineStatus(),
        this.api.listImportSources(),
        this.api.pipelineJobs({
          filter: this.filter,
          limit: Math.max(PAGE, this.jobs.length),
        }),
        this.api.pipelineLog({
          min_level: this.logLevel,
          limit: Math.max(PAGE, this.log.length),
        }),
      ]);
      if (generation !== this.generation) return;
      this.status = status;
      this.sources = this.keepUnchanged(sources);
      this.jobsComplete = jobs.length < Math.max(PAGE, this.jobs.length);
      this.jobs = jobs;
      this.logComplete = log.length < Math.max(PAGE, this.log.length);
      this.log = log;
    } catch (e) {
      if (generation === this.generation) this.error = ApiError.from(e).message;
    }
  }

  async setFilter(filter: JobFilter): Promise<void> {
    this.filter = filter;
    this.jobs = [];
    await this.refresh();
  }

  async setLogLevel(level: LogLevel): Promise<void> {
    this.logLevel = level;
    this.log = [];
    await this.refresh();
  }

  async moreJobs(): Promise<void> {
    const last = this.jobs.at(-1);
    if (!last) return;
    const page = await this.api.pipelineJobs({
      filter: this.filter,
      limit: PAGE,
      before: last.id,
    });
    this.jobs = [...this.jobs, ...page];
    this.jobsComplete = page.length < PAGE;
  }

  async moreLog(): Promise<void> {
    const last = this.log.at(-1);
    if (!last) return;
    const page = await this.api.pipelineLog({
      min_level: this.logLevel,
      limit: PAGE,
      before: last.id,
    });
    this.log = [...this.log, ...page];
    this.logComplete = page.length < PAGE;
  }

  /** Runs a change, shows its error if any, then reloads. */
  private async act(change: () => Promise<unknown>): Promise<boolean> {
    this.error = null;
    try {
      await change();
      return true;
    } catch (e) {
      this.error = ApiError.from(e).message;
      return false;
    } finally {
      await this.refresh();
    }
  }

  addSource(
    path: string,
    watch = true,
    deleteOriginals: OriginalsChoice = "default",
  ): Promise<boolean> {
    const trimmed = path.trim();
    if (!trimmed) return Promise.resolve(false);
    return this.act(() =>
      this.api.addImportSource(trimmed, watch, deleteOriginals),
    );
  }

  setDeleteOriginals(id: number, choice: OriginalsChoice): Promise<boolean> {
    return this.act(() => this.api.setImportSourceDeleteOriginals(id, choice));
  }

  removeSource(id: number): Promise<boolean> {
    return this.act(() => this.api.removeImportSource(id));
  }

  setWatch(id: number, watch: boolean): Promise<boolean> {
    return this.act(() => this.api.setImportSourceWatch(id, watch));
  }

  scan(id?: number): Promise<boolean> {
    return this.act(() => this.api.scanImportSources(id));
  }

  retry(ids?: number[]): Promise<boolean> {
    return this.act(() => this.api.retryFailedImports(ids));
  }
}
