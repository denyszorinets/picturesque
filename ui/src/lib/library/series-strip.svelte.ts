import {
  photoSeries,
  queryPhotos,
  type PhotoPage,
  type PhotoQuery,
  type PhotoSummary,
  type SeriesView,
} from "../api";

/** What the strip reads from the backend. */
export interface StripApi {
  photoSeries(photoId: number): Promise<SeriesView>;
  queryPhotos(query: PhotoQuery): Promise<PhotoPage>;
}

/** Loupe choices that outlive one photo (and one loupe). */
export const stripPrefs = $state({ compareBest: false });

/**
 * The series of the photo in the loupe: its members in capture order, for
 * the ribbon, and the photo to compare with (the series' best, or the
 * runner-up when the shown photo is the best).
 */
export class SeriesStrip {
  /** Members in capture order; empty when the photo is in no series. */
  members = $state<PhotoSummary[]>([]);
  /** The series' photos best first. */
  ranking = $state<number[]>([]);
  bestId = $state<number | null>(null);
  error = $state<string | null>(null);
  private generation = 0;

  constructor(private readonly api: StripApi = { photoSeries, queryPhotos }) {}

  /** Loads the series of `photo`; the newest call wins. */
  async load(photo: PhotoSummary | undefined): Promise<void> {
    const generation = ++this.generation;
    if (!photo || photo.series_id === null) {
      this.clear();
      return;
    }
    try {
      const series = await this.api.photoSeries(photo.id);
      if (series.photo_ids.length < 2) {
        if (generation === this.generation) this.clear();
        return;
      }
      const page = await this.api.queryPhotos({
        photo_ids: series.photo_ids,
        limit: series.photo_ids.length,
      });
      if (generation !== this.generation) return;
      this.members = series.photo_ids
        .map((id) => page.photos.find((p) => p.id === id))
        .filter((p): p is PhotoSummary => p !== undefined);
      this.ranking = series.ranking.map((r) => r.photo_id);
      this.bestId = series.best_photo_id;
      this.error = null;
    } catch (e) {
      if (generation !== this.generation) return;
      this.clear();
      this.error = e instanceof Error ? e.message : String(e);
    }
  }

  private clear() {
    this.members = [];
    this.ranking = [];
    this.bestId = null;
    this.error = null;
  }

  /** 1-based place of a photo in the series, or `null`. */
  position(id: number): number | null {
    const at = this.members.findIndex((p) => p.id === id);
    return at < 0 ? null : at + 1;
  }

  /**
   * The photo to show beside `id`: the best of the series, or the runner-up
   * when `id` is the best. `null` when there is none.
   */
  rival(id: number): { photo: PhotoSummary; runnerUp: boolean } | null {
    const runnerUp = id === this.bestId;
    const rivalId = runnerUp
      ? this.ranking.find((other) => other !== id)
      : (this.bestId ?? this.ranking[0]);
    const photo = this.members.find((p) => p.id === rivalId);
    return photo ? { photo, runnerUp } : null;
  }
}
