import { describe, expect, it, vi } from "vitest";
import type { PhotoSummary, SeriesView } from "../api";
import { SeriesStrip, type StripApi } from "./series-strip.svelte";

function photo(id: number, over: Partial<PhotoSummary> = {}): PhotoSummary {
  return {
    id,
    file_name: `IMG_${id}.JPG`,
    series_id: 1,
    flag: "unflagged",
    ...over,
  } as PhotoSummary;
}

function series(over: Partial<SeriesView> = {}): SeriesView {
  return {
    photo_id: 2,
    series_id: 1,
    photo_ids: [1, 2, 3],
    edit: null,
    best_photo_id: 3,
    best_chosen: false,
    ranking: [
      { photo_id: 3, score: 0.9 },
      { photo_id: 1, score: 0.6 },
      { photo_id: 2, score: 0.4 },
    ],
    ...over,
  };
}

function api(view: SeriesView = series()) {
  return {
    photoSeries: vi.fn<StripApi["photoSeries"]>(async () => view),
    // Returned in a different order than the series: the strip must sort.
    queryPhotos: vi.fn<StripApi["queryPhotos"]>(async () => ({
      photos: [photo(3), photo(1), photo(2)],
      total: 3,
      next_cursor: null,
    })) as unknown as StripApi["queryPhotos"],
  };
}

describe("SeriesStrip", () => {
  it("lists the members in capture order and knows the best", async () => {
    const strip = new SeriesStrip(api());
    await strip.load(photo(2));

    expect(strip.members.map((p) => p.id)).toEqual([1, 2, 3]);
    expect(strip.bestId).toBe(3);
    expect(strip.position(2)).toBe(2);
    expect(strip.position(99)).toBeNull();
  });

  it("is empty for a photo in no series or in a series of one", async () => {
    const backend = api();
    const strip = new SeriesStrip(backend);
    await strip.load(photo(2));
    await strip.load(photo(2, { series_id: null }));
    expect(strip.members).toEqual([]);
    expect(backend.photoSeries).toHaveBeenCalledTimes(1);

    await strip.load(photo(2));
    const alone = api(series({ photo_ids: [2] }));
    const lone = new SeriesStrip(alone);
    await lone.load(photo(2));
    expect(lone.members).toEqual([]);
    expect(alone.queryPhotos).not.toHaveBeenCalled();
  });

  it("compares with the best, or with the runner-up when it is the best", async () => {
    const strip = new SeriesStrip(api());
    await strip.load(photo(2));

    expect(strip.rival(2)).toMatchObject({
      photo: { id: 3 },
      runnerUp: false,
    });
    expect(strip.rival(1)).toMatchObject({ photo: { id: 3 } });
    expect(strip.rival(3)).toMatchObject({
      photo: { id: 1 },
      runnerUp: true,
    });
  });

  it("has nothing to compare with before the series is known", () => {
    expect(new SeriesStrip(api()).rival(1)).toBeNull();
  });

  it("keeps the newest load when an older one finishes last", async () => {
    let finishOld!: (v: SeriesView) => void;
    const old = new Promise<SeriesView>((r) => (finishOld = r));
    const backend = api();
    backend.photoSeries
      .mockReturnValueOnce(old)
      .mockResolvedValueOnce(series({ best_photo_id: 1 }));
    const strip = new SeriesStrip(backend);

    const first = strip.load(photo(2));
    await strip.load(photo(2));
    finishOld(series({ best_photo_id: 2 }));
    await first;

    expect(strip.bestId).toBe(1);
  });

  it("clears and reports a failed load", async () => {
    const backend = api();
    const strip = new SeriesStrip(backend);
    await strip.load(photo(2));
    backend.photoSeries.mockRejectedValueOnce(new Error("boom"));

    await strip.load(photo(2));

    expect(strip.members).toEqual([]);
    expect(strip.error).toBe("boom");
  });
});
