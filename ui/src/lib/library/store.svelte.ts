import type {
  PhotoFacets,
  PhotoPage,
  PhotoQuery,
  PhotoSort,
  PhotoSummary,
} from "../api";
import { ApiError } from "../api";
import { buildSections, type Section } from "./layout";

/** Backend calls the library needs (injectable for tests). */
export interface LibraryApi {
  queryPhotos(query: PhotoQuery): Promise<PhotoPage>;
  photoFacets(filter?: PhotoQuery): Promise<PhotoFacets>;
}

const PAGE = 300;

/** Photos to show in a given order (search results), with a heading. */
export interface Ranking {
  ids: number[];
  label: string;
}

/**
 * The timeline's data: day counts (so the full layout is known up front)
 * and the photos, loaded page by page in newest-first order as they are
 * needed.
 */
export class LibraryStore {
  facets = $state<PhotoFacets | null>(null);
  /** Loaded photos; index i is the i-th photo of the timeline. */
  photos = $state<PhotoSummary[]>([]);
  error = $state<string | null>(null);
  loading = $state(false);

  private cursor: string | null = null;
  private complete = false;
  private queue: Promise<void> = Promise.resolve();
  private generation = 0;
  /** Filters (and sort) applied to the timeline. */
  private filter: PhotoQuery = {};
  /** The sort `facets` were loaded for (sections follow it). */
  private sort = $state<PhotoSort | undefined>();
  /** Search results: photo ids best first, shown in that order. */
  private ranking: Ranking | null = null;
  /** The heading of the ranked list `facets` were loaded for. */
  private rankedLabel = $state<string | null>(null);

  constructor(private readonly api: LibraryApi) {}

  get total(): number {
    return this.facets?.total ?? 0;
  }

  get sections(): Section[] {
    if (!this.facets) return [];
    if (this.rankedLabel !== null)
      return this.facets.total > 0
        ? [
            {
              day: null,
              label: this.rankedLabel,
              start: 0,
              count: this.facets.total,
            },
          ]
        : [];
    return buildSections(this.facets.days, this.facets.total, this.sort);
  }

  /**
   * Applies new filters and reloads. With `ranking`, shows only those
   * photos (that match the filters) in the ranking's order.
   */
  setFilter(filter: PhotoQuery, ranking: Ranking | null = null): Promise<void> {
    this.filter = filter;
    this.ranking = ranking;
    return this.load();
  }

  /** Loads the ranked photos that match the filters, in ranking order. */
  private async loadRanked(ranking: Ranking, generation: number) {
    const query = { ...this.filter, photo_ids: ranking.ids };
    const [facets, page] = await Promise.all([
      this.api.photoFacets(query),
      this.api.queryPhotos({ ...query, limit: 1000 }),
    ]);
    if (generation !== this.generation) return;
    const position: Record<number, number> = {};
    ranking.ids.forEach((id, i) => (position[id] = i));
    this.sort = this.filter.sort;
    this.rankedLabel = ranking.label;
    this.facets = facets;
    this.photos = [...page.photos].sort(
      (a, b) => (position[a.id] ?? 0) - (position[b.id] ?? 0),
    );
    this.cursor = null;
    this.complete = true;
    this.queue = Promise.resolve();
  }

  /** (Re)loads facets and the first page. */
  async load(): Promise<void> {
    const generation = ++this.generation;
    this.loading = true;
    this.error = null;
    try {
      if (this.ranking) {
        await this.loadRanked(this.ranking, generation);
        return;
      }
      const [facets, page] = await Promise.all([
        this.api.photoFacets(this.filter),
        this.api.queryPhotos({ ...this.filter, limit: PAGE }),
      ]);
      if (generation !== this.generation) return;
      this.sort = this.filter.sort;
      this.rankedLabel = null;
      this.facets = facets;
      this.photos = page.photos;
      this.cursor = page.next_cursor;
      this.complete = page.next_cursor === null;
      this.queue = Promise.resolve();
    } catch (e) {
      this.error = ApiError.from(e).message;
    } finally {
      this.loading = false;
    }
  }

  /** Reloads only the counts (after decisions changed). */
  async refreshFacets(): Promise<void> {
    const generation = this.generation;
    try {
      const query = this.ranking
        ? { ...this.filter, photo_ids: this.ranking.ids }
        : this.filter;
      const facets = await this.api.photoFacets(query);
      if (generation === this.generation) this.facets = facets;
    } catch {
      // Stale counts are harmless; the next load fixes them.
    }
  }

  /**
   * Reloads facets and the photos loaded so far, in place, so the view
   * keeps its scroll position (used when series or best photos change).
   */
  async refresh(): Promise<void> {
    const generation = ++this.generation;
    const want = Math.max(1, this.photos.length);
    try {
      if (this.ranking) {
        await this.loadRanked(this.ranking, generation);
        return;
      }
      const facets = await this.api.photoFacets(this.filter);
      const photos: PhotoSummary[] = [];
      let cursor: string | null = null;
      do {
        const page: PhotoPage = await this.api.queryPhotos({
          ...this.filter,
          limit: PAGE,
          cursor: cursor ?? undefined,
        });
        if (generation !== this.generation) return;
        photos.push(...page.photos);
        cursor = page.next_cursor;
      } while (cursor !== null && photos.length < want);
      this.sort = this.filter.sort;
      this.facets = facets;
      this.photos = photos;
      this.cursor = cursor;
      this.complete = cursor === null;
      this.queue = Promise.resolve();
    } catch (e) {
      if (generation === this.generation) this.error = ApiError.from(e).message;
    }
  }

  /** Loads pages until photo `index` is available (or there are no more). */
  ensureLoaded(index: number): Promise<void> {
    const generation = this.generation;
    this.queue = this.queue.then(async () => {
      while (
        generation === this.generation &&
        !this.complete &&
        this.photos.length <= index
      ) {
        try {
          const page = await this.api.queryPhotos({
            ...this.filter,
            limit: PAGE,
            cursor: this.cursor ?? undefined,
          });
          if (generation !== this.generation) return;
          this.photos = [...this.photos, ...page.photos];
          this.cursor = page.next_cursor;
          this.complete = page.next_cursor === null;
        } catch (e) {
          this.error = ApiError.from(e).message;
          return;
        }
      }
    });
    return this.queue;
  }

  /** Distinct cities among the loaded photos of a section. */
  places(section: Section): string[] {
    const seen: string[] = [];
    const end = Math.min(section.start + section.count, this.photos.length);
    for (let i = section.start; i < end; i++) {
      const city = this.photos[i]?.city;
      if (city && !seen.includes(city)) seen.push(city);
    }
    return seen;
  }
}
