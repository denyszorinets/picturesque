import { describe, expect, it } from "vitest";
import { Filters } from "./filters.svelte";

function memoryStorage() {
  const data = new Map<string, string>();
  return {
    getItem: (k: string) => data.get(k) ?? null,
    setItem: (k: string, v: string) => void data.set(k, v),
  };
}

describe("Filters", () => {
  it("has no query and no chips by default", () => {
    const f = new Filters(memoryStorage());

    expect(f.toQuery()).toEqual({});
    expect(f.active).toBe(false);
  });

  it("builds the query from every filter", () => {
    const f = new Filters(memoryStorage());
    f.from = "2026-10-01";
    f.to = "2026-10-31";
    f.toggleFlag("pick");
    f.toggleFlag("unflagged");
    f.minRating = 3;
    f.camera = { make: "Canon", model: "Canon EOS R5" };
    f.lens = "RF50";
    f.place = { country_code: "PT", city: "Lisbon", label: "Lisbon, Portugal" };

    expect(f.toQuery()).toEqual({
      from: "2026-10-01",
      to: "2026-10-31",
      flags: ["pick", "unflagged"],
      min_rating: 3,
      camera: { make: "Canon", model: "Canon EOS R5" },
      lens: "RF50",
      country_code: "PT",
      city: "Lisbon",
    });
  });

  it("shows a chip per filter that clears it", () => {
    const f = new Filters(memoryStorage());
    f.from = "2026-10-01";
    f.toggleFlag("reject");
    f.minRating = 2;
    f.place = { country_code: "PT", label: "Portugal" };

    expect(f.chips.map((c) => c.label)).toEqual([
      "From 2026-10-01",
      "Rejects",
      "★★ or more",
      "Portugal",
    ]);
    f.chips[1]!.clear();
    expect(f.flags).toEqual([]);
    f.chips[0]!.clear();
    expect(f.toQuery()).toEqual({ min_rating: 2, country_code: "PT" });
  });

  it("toggles flags and clears everything", () => {
    const f = new Filters(memoryStorage());
    f.toggleFlag("pick");
    f.toggleFlag("pick");
    expect(f.flags).toEqual([]);
    f.toggleFlag("reject");
    f.lens = "x";
    f.clear();
    expect(f.active).toBe(false);
  });

  it("keeps filters for the session", () => {
    const storage = memoryStorage();
    const first = new Filters(storage);
    first.toggleFlag("pick");
    first.minRating = 4;
    first.save();

    const second = new Filters(storage);

    expect(second.toQuery()).toEqual({ flags: ["pick"], min_rating: 4 });
  });

  it("ignores unreadable saved filters", () => {
    const storage = memoryStorage();
    storage.setItem("picturesque.library.filters", "{not json");

    expect(new Filters(storage).active).toBe(false);
  });

  it("stacks series, expands some, and keeps stacking on clear", () => {
    const storage = memoryStorage();
    const f = new Filters(storage);
    f.stacked = true;
    f.toggleExpanded(9);
    f.toggleExpanded(3);
    f.toggleExpanded(5);
    f.toggleExpanded(5);

    expect(f.toQuery()).toEqual({
      collapse_series: true,
      expanded_series: [3, 9],
    });
    expect(f.active).toBe(false);
    f.clear();
    expect(f.stacked).toBe(true);

    f.save();
    const reopened = new Filters(storage);
    // Stacking is kept; which series were expanded is not.
    expect(reopened.toQuery()).toEqual({ collapse_series: true });
  });

  it("filters on scores and best photos, and sorts", () => {
    const storage = memoryStorage();
    const f = new Filters(storage);
    f.minScores = { sharpness: 0.5, exposure: 0, noise: 0.25 };
    f.bestOnly = true;
    f.sort = "quality_desc";

    expect(f.toQuery()).toEqual({
      min_sharpness: 0.5,
      min_noise: 0.25,
      best_only: true,
      sort: "quality_desc",
    });
    expect(f.chips.map((c) => c.label)).toEqual([
      "Sharpness ≥ 50%",
      "Low noise ≥ 25%",
      "Best only",
    ]);
    f.save();
    expect(new Filters(storage).toQuery()).toEqual(f.toQuery());

    f.chips[0]!.clear();
    expect(f.minScores.sharpness).toBe(0);
    f.clear();
    // Sorting is not a filter.
    expect(f.toQuery()).toEqual({ sort: "quality_desc" });
  });
});
