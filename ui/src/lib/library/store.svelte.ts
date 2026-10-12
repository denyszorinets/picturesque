import type { PhotoFacets, PhotoPage, PhotoQuery, PhotoSummary } from "../api";
import { ApiError } from "../api";
import { buildSections, type Section } from "./layout";

/** Backend calls the library needs (injectable for tests). */
export interface LibraryApi {
  queryPhotos(query: PhotoQuery): Promise<PhotoPage>;
  photoFacets(): Promise<PhotoFacets>;
}

const PAGE = 300;

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

  constructor(private readonly api: LibraryApi) {}

  get total(): number {
    return this.facets?.total ?? 0;
  }

  get sections(): Section[] {
    return this.facets
      ? buildSections(this.facets.days, this.facets.total)
      : [];
  }

  /** (Re)loads facets and the first page. */
  async load(): Promise<void> {
    const generation = ++this.generation;
    this.loading = true;
    this.error = null;
    try {
      const [facets, page] = await Promise.all([
        this.api.photoFacets(),
        this.api.queryPhotos({ limit: PAGE }),
      ]);
      if (generation !== this.generation) return;
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
