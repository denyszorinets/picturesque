import { describe, expect, it, vi } from "vitest";
import type { DecisionView, PhotoSummary } from "../api";
import {
  Culling,
  type CullingApi,
  type DecisionChange,
} from "./culling.svelte";

function photo(
  id: number,
  flag: PhotoSummary["flag"] = "unflagged",
  rating = 0,
): PhotoSummary {
  return {
    id,
    captured_at: null,
    day: null,
    camera: null,
    lens: null,
    city: null,
    country: null,
    flag,
    rating,
    width: null,
    height: null,
    has_raw: false,
    file_name: `${id}.jpg`,
    series_id: null,
    best_in_series: false,
    series_size: 0,
    quality: null,
  };
}

/** A backend that applies changes to its own copy of decisions. */
function backend(list: {
  photos: PhotoSummary[];
}): CullingApi & { calls: DecisionChange[][] } {
  const state = new Map(
    list.photos.map((p) => [p.id, { flag: p.flag, rating: p.rating }]),
  );
  const calls: DecisionChange[][] = [];
  return {
    calls,
    setDecisions: vi.fn(
      async (changes: DecisionChange[]): Promise<DecisionView[]> => {
        calls.push(changes);
        return changes.map((c) => {
          const d = state.get(c.photo_id)!;
          if (c.flag) d.flag = c.flag;
          if (c.rating !== undefined) d.rating = c.rating;
          return { photo_id: c.photo_id, flag: d.flag, rating: d.rating };
        });
      },
    ),
  };
}

describe("Culling", () => {
  it("flags and rates the given photos and updates the list", async () => {
    const list = { photos: [photo(1), photo(2), photo(3)] };
    const culling = new Culling(backend(list), list);

    await culling.flag([0, 2], "pick");
    await culling.rate([2], 4);

    expect(list.photos.map((p) => [p.flag, p.rating])).toEqual([
      ["pick", 0],
      ["unflagged", 0],
      ["pick", 4],
    ]);
  });

  it("undoes and redoes, restoring each photo's previous decision", async () => {
    const list = { photos: [photo(1, "reject"), photo(2, "unflagged", 3)] };
    const culling = new Culling(backend(list), list);

    await culling.flag([0, 1], "pick");
    expect(culling.canUndo).toBe(true);
    await culling.undo();
    expect(list.photos.map((p) => [p.flag, p.rating])).toEqual([
      ["reject", 0],
      ["unflagged", 3],
    ]);
    expect(culling.canRedo).toBe(true);
    await culling.redo();
    expect(list.photos.map((p) => p.flag)).toEqual(["pick", "pick"]);
  });

  it("a new change clears the redo history", async () => {
    const list = { photos: [photo(1)] };
    const culling = new Culling(backend(list), list);
    await culling.flag([0], "pick");
    await culling.undo();

    await culling.rate([0], 2);

    expect(culling.canRedo).toBe(false);
  });

  it("only sends photos the change alters", async () => {
    const list = { photos: [photo(1, "pick"), photo(2)] };
    const api = backend(list);
    const culling = new Culling(api, list);

    await culling.flag([0, 1], "pick");
    await culling.flag([0], "pick");

    expect(api.calls).toEqual([[{ photo_id: 2, flag: "pick" }]]);
  });

  it("keeps history intact when the backend fails", async () => {
    const list = { photos: [photo(1)] };
    const culling = new Culling(
      { setDecisions: vi.fn().mockRejectedValue(new Error("offline")) },
      list,
    );

    await culling.flag([0], "reject");

    expect(culling.error).toBe("offline");
    expect(culling.canUndo).toBe(false);
    expect(list.photos[0]!.flag).toBe("unflagged");
  });

  it("culls photos outside the list and tells subscribers", async () => {
    const list = { photos: [photo(1)] };
    const api = backend({ photos: [photo(1), photo(7)] });
    const culling = new Culling(api, list);
    const seen: number[][] = [];
    const stop = culling.subscribe((d) => seen.push(d.map((v) => v.photo_id)));

    await culling.flagPhotos([photo(7), photo(1)], "reject");
    await culling.ratePhotos([photo(7)], 3);
    await culling.undo();
    stop();
    await culling.redo();

    expect(list.photos[0]!.flag).toBe("reject");
    expect(seen).toEqual([[7, 1], [7], [7]]);
  });
});
