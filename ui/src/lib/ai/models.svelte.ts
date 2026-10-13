import { ApiError, type AppEvent, type ModelView } from "../api";

/** Backend calls the model list needs (injectable for tests). */
export interface ModelsApi {
  listModels(): Promise<ModelView[]>;
  installModel(id: string): Promise<ModelView>;
  cancelModel(id: string): Promise<ModelView>;
  removeModel(id: string): Promise<ModelView>;
}

/** The downloadable AI models, kept current from `model_progress`. */
export class Models {
  list = $state<ModelView[]>([]);
  error = $state<string | null>(null);

  /** Events received per model, to spot responses that events overtook. */
  private events: Record<string, number> = {};

  constructor(private readonly api: ModelsApi) {}

  async load(): Promise<void> {
    try {
      this.list = await this.api.listModels();
    } catch (e) {
      this.error = ApiError.from(e).message;
    }
  }

  handle(event: AppEvent): void {
    if (event.type !== "model_progress") return;
    this.events[event.id] = (this.events[event.id] ?? 0) + 1;
    this.update(event);
  }

  install(id: string): Promise<void> {
    return this.run(id, () => this.api.installModel(id));
  }

  cancel(id: string): Promise<void> {
    return this.run(id, () => this.api.cancelModel(id));
  }

  remove(id: string): Promise<void> {
    return this.run(id, () => this.api.removeModel(id));
  }

  private async run(id: string, call: () => Promise<ModelView>): Promise<void> {
    this.error = null;
    const seen = this.events[id] ?? 0;
    try {
      const view = await call();
      // A download can finish before its command's response arrives; the
      // event is newer then.
      if ((this.events[id] ?? 0) === seen) this.update(view);
    } catch (e) {
      this.error = ApiError.from(e).message;
    }
  }

  private update(view: ModelView): void {
    this.list = this.list.map((m) => (m.id === view.id ? { ...view } : m));
  }
}

/** `1234567` → `1.2 MB`. */
export function formatSize(bytes: number): string {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(1)} MB`;
  return `${Math.max(1, Math.round(bytes / 1e3))} KB`;
}
