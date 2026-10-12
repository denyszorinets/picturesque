import { describe, expect, it, vi } from "vitest";
import type { PhotoFacets, PhotoPage, PhotoSummary } from "../api";
import { LibraryStore, type LibraryApi } from "./store.svelte";

function photo(id: number, city: string | null = null): PhotoSummary {
  return {
    id,
    captured_at: null,
    day: null,
    camera: null,
    lens: null,
    city,
    country: null,
    flag: "unflagged",
    rating: 0,
    width: null,
    height: null,
    has_raw: false,
    file_name: `${id}.jpg`,
  };
}

function facets(total: number): PhotoFacets {
  return {
    total,
    first_day: null,
    last_day: null,
    days: [{ value: "2026-10-12", count: total }],
    cameras: [],
    lenses: [],
    places: [],
    flags: [],
    ratings: [0, 0, 0, 0, 0, 0],
  };
}

/** A backend with `total` photos served in pages. */
function backend(
  total: number,
  cityOf = (_: number): string | null => null,
): LibraryApi {
  return {
    photoFacets: vi.fn().mockResolvedValue(facets(total)),
    queryPhotos: vi.fn(async (q): Promise<PhotoPage> => {
      const start = q.cursor ? Number(q.cursor) : 0;
      const end = Math.min(total, start + (q.limit ?? 200));
      const photos = Array.from({ length: end - start }, (_, i) =>
        photo(start + i + 1, cityOf(start + i)),
      );
      return { photos, next_cursor: end < total ? String(end) : null, total };
    }),
  };
}

describe("LibraryStore", () => {
  it("loads facets and the first page", async () => {
    const store = new LibraryStore(backend(1000));

    await store.load();

    expect(store.total).toBe(1000);
    expect(store.photos).toHaveLength(300);
    expect(store.sections).toEqual([
      { day: "2026-10-12", start: 0, count: 1000 },
    ]);
  });

  it("loads further pages only as needed", async () => {
    const api = backend(1000);
    const store = new LibraryStore(api);
    await store.load();

    await store.ensureLoaded(650);
    expect(store.photos).toHaveLength(900);
    await store.ensureLoaded(100);
    expect(api.queryPhotos).toHaveBeenCalledTimes(3);
    await store.ensureLoaded(5000);
    expect(store.photos).toHaveLength(1000);
  });

  it("serializes overlapping requests", async () => {
    const api = backend(1000);
    const store = new LibraryStore(api);
    await store.load();

    await Promise.all([
      store.ensureLoaded(400),
      store.ensureLoaded(500),
      store.ensureLoaded(320),
    ]);

    expect(store.photos).toHaveLength(600);
    expect(new Set(store.photos.map((p) => p.id)).size).toBe(600);
  });

  it("lists the cities of a section's loaded photos", async () => {
    const store = new LibraryStore(
      backend(5, (i) => (i < 3 ? "Lisbon" : "Porto")),
    );
    await store.load();

    expect(store.places({ day: "2026-10-12", start: 0, count: 5 })).toEqual([
      "Lisbon",
      "Porto",
    ]);
  });

  it("reports errors", async () => {
    const store = new LibraryStore({
      photoFacets: vi.fn().mockRejectedValue(new Error("catalog unavailable")),
      queryPhotos: vi.fn(),
    });

    await store.load();

    expect(store.error).toBe("catalog unavailable");
  });
});
