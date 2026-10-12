import type { DecisionView, PhotoSummary } from "../api";
import { ApiError } from "../api";

type Flag = PhotoSummary["flag"];

export interface DecisionChange {
  photo_id: number;
  flag?: Flag;
  rating?: number;
}

/** Backend call culling needs (injectable for tests). */
export interface CullingApi {
  setDecisions(changes: DecisionChange[]): Promise<DecisionView[]>;
}

/** Where culling reads and updates photos (the timeline store). */
export interface PhotoList {
  photos: PhotoSummary[];
}

interface Step {
  /** What to send to undo the step: each photo's previous decision. */
  before: DecisionChange[];
  /** What to send to redo it. */
  after: DecisionChange[];
}

const HISTORY = 200;

/**
 * Applies pick/reject/rating changes to photos, keeps the loaded list in
 * sync, and keeps an undo/redo history.
 */
export class Culling {
  error = $state<string | null>(null);
  private undoStack: Step[] = [];
  private redoStack: Step[] = [];
  /** Whether there is something to undo / redo. */
  canUndo = $state(false);
  canRedo = $state(false);

  constructor(
    private readonly api: CullingApi,
    private readonly list: PhotoList,
  ) {}

  flag(indices: Iterable<number>, flag: Flag): Promise<void> {
    return this.change(indices, { flag });
  }

  rate(indices: Iterable<number>, rating: number): Promise<void> {
    return this.change(indices, { rating });
  }

  async undo(): Promise<void> {
    const step = this.undoStack.pop();
    if (!step) return;
    if (await this.send(step.before)) this.redoStack.push(step);
    else this.undoStack.push(step);
    this.sync();
  }

  async redo(): Promise<void> {
    const step = this.redoStack.pop();
    if (!step) return;
    if (await this.send(step.after)) this.undoStack.push(step);
    else this.redoStack.push(step);
    this.sync();
  }

  private async change(
    indices: Iterable<number>,
    change: { flag?: Flag; rating?: number },
  ): Promise<void> {
    const photos = [...indices]
      .map((i) => this.list.photos[i])
      .filter((p): p is PhotoSummary => !!p);
    // Skip photos the change would not alter, so undo only touches real changes.
    const affected = photos.filter(
      (p) =>
        (change.flag !== undefined && p.flag !== change.flag) ||
        (change.rating !== undefined && p.rating !== change.rating),
    );
    if (affected.length === 0) return;
    const before = affected.map((p) => ({
      photo_id: p.id,
      flag: p.flag,
      rating: p.rating,
    }));
    const after = affected.map((p) => ({ photo_id: p.id, ...change }));
    if (await this.send(after)) {
      this.undoStack.push({ before, after });
      if (this.undoStack.length > HISTORY) this.undoStack.shift();
      this.redoStack = [];
    }
    this.sync();
  }

  /** Sends changes and applies the results to the loaded photos. */
  private async send(changes: DecisionChange[]): Promise<boolean> {
    this.error = null;
    try {
      const results = await this.api.setDecisions(changes);
      const byId: Record<number, DecisionView> = {};
      for (const r of results) byId[r.photo_id] = r;
      this.list.photos = this.list.photos.map((p) => {
        const r = byId[p.id];
        return r ? { ...p, flag: r.flag, rating: r.rating } : p;
      });
      return true;
    } catch (e) {
      this.error = ApiError.from(e).message;
      return false;
    }
  }

  private sync() {
    this.canUndo = this.undoStack.length > 0;
    this.canRedo = this.redoStack.length > 0;
  }
}
