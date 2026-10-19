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
    series_id: null,
    best_in_series: false,
    series_size: 0,
    quality: null,
    aesthetic: null,
    eyes_closed: null,
    faces: 0,
    keeper: false,
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
    auto_picked: 0,
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

  it("refreshes the loaded photos in place", async () => {
    const api = backend(1000);
    const store = new LibraryStore(api);
    await store.load();
    await store.ensureLoaded(400);

    vi.mocked(api.photoFacets).mockResolvedValue(facets(999));
    await store.refresh();

    expect(store.total).toBe(999);
    expect(store.photos).toHaveLength(600);
    await store.ensureLoaded(700);
    expect(store.photos.map((p) => p.id).slice(598, 602)).toEqual([
      599, 600, 601, 602,
    ]);
  });

  it("shows ranked photos in ranking order under one heading", async () => {
    const api = backend(10);
    vi.mocked(api.queryPhotos).mockImplementation(async (q) => ({
      // The backend returns matches in its own order.
      photos: (q.photo_ids ?? [])
        .filter((id) => id !== 7)
        .sort()
        .map((id) => photo(id)),
      next_cursor: null,
      total: 3,
    }));
    vi.mocked(api.photoFacets).mockResolvedValue(facets(3));
    const store = new LibraryStore(api);

    await store.setFilter(
      { min_rating: 2 },
      { ids: [9, 7, 2, 5], label: "Matching “dog”" },
    );

    expect(api.queryPhotos).toHaveBeenCalledWith({
      min_rating: 2,
      photo_ids: [9, 7, 2, 5],
      limit: 1000,
    });
    expect(store.photos.map((p) => p.id)).toEqual([9, 2, 5]);
    expect(store.sections).toEqual([
      { day: null, label: "Matching “dog”", start: 0, count: 3 },
    ]);
    await store.refresh();
    expect(store.photos.map((p) => p.id)).toEqual([9, 2, 5]);

    // Without a ranking, the timeline is back.
    vi.mocked(api.photoFacets).mockResolvedValue(facets(10));
    await store.setFilter({});
    expect(store.sections[0]!.day).toBe("2026-10-12");
  });

  it("drops the search heading when a refresh overtakes clearing the search", async () => {
    const api = backend(10);
    const store = new LibraryStore(api);
    await store.setFilter({}, { ids: [2, 5], label: "Matching “red”" });

    // Clearing the search reloads; a refresh (series changed) starts
    // meanwhile and wins.
    const cleared = store.setFilter({});
    const refreshed = store.refresh();
    await Promise.all([cleared, refreshed]);

    expect(store.sections[0]!.label).not.toMatch(/Matching/);
    expect(store.sections[0]!.day).toBe("2026-10-12");
  });

  it("refreshes only the counts", async () => {
    const api = backend(1000);
    const store = new LibraryStore(api);
    await store.load();
    vi.mocked(api.photoFacets).mockResolvedValue(facets(999));
    await store.refreshFacets();
    expect(store.total).toBe(999);
    expect(store.photos).toHaveLength(300);
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

describe("LibraryStore filters", () => {
  it("sends the filter with facets and every page", async () => {
    const api = backend(700);
    const store = new LibraryStore(api);

    await store.setFilter({ flags: ["pick"] });
    await store.ensureLoaded(400);

    expect(api.photoFacets).toHaveBeenCalledWith({ flags: ["pick"] });
    for (const [q] of (api.queryPhotos as ReturnType<typeof vi.fn>).mock
      .calls) {
      expect(q.flags).toEqual(["pick"]);
    }
  });
});
