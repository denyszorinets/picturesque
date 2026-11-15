import { describe, expect, it, vi } from "vitest";
import type { SpeciesView } from "../api";
import { BirdNames, names } from "./birds.svelte";

const robin: SpeciesView = {
  common: "American Robin",
  scientific: "Turdus migratorius",
  code: "amerob",
};
const jay: SpeciesView = {
  common: "Blue Jay",
  scientific: "Cyanocitta cristata",
  code: "blujay",
};

function api(stored: Record<number, SpeciesView[]> = {}) {
  return {
    photoSpecies: vi.fn(async (ids: number[]) =>
      ids
        .filter((id) => stored[id])
        .map((id) => ({ photo_id: id, species: stored[id]! })),
    ),
    setPhotoSpecies: vi.fn(async (id: number, species: SpeciesView[]) => {
      stored[id] = species;
    }),
  };
}

describe("BirdNames", () => {
  it("loads each photo's species once", async () => {
    const backend = api({ 1: [robin] });
    const names = new BirdNames(backend);

    await names.load([1, 2, 2]);
    await names.load([1, 2]);

    expect(backend.photoSpecies).toHaveBeenCalledTimes(1);
    expect(backend.photoSpecies).toHaveBeenCalledWith([1, 2]);
    expect(names.of(1)).toEqual([robin]);
    expect(names.of(2)).toEqual([]);
  });

  it("asks again after a failed load", async () => {
    const backend = {
      photoSpecies: vi
        .fn()
        .mockRejectedValueOnce(new Error("offline"))
        .mockResolvedValue([{ photo_id: 3, species: [robin] }]),
      setPhotoSpecies: vi.fn(async () => {}),
    };
    const names = new BirdNames(backend);

    await names.load([3]);
    expect(names.of(3)).toEqual([]);
    await names.load([3]);
    expect(names.of(3)).toEqual([robin]);
  });

  it("adds species once each, to every photo given, and removes them", async () => {
    const backend = api({ 4: [robin] });
    const names = new BirdNames(backend);

    await names.add([4, 5], jay);
    await names.add([4], jay);
    expect(names.of(4)).toEqual([robin, jay]);
    expect(names.of(5)).toEqual([jay]);
    expect(backend.setPhotoSpecies).toHaveBeenCalledTimes(2);

    await names.remove([4, 5], "jay-not-there");
    await names.remove([4], "amerob");
    expect(names.of(4)).toEqual([jay]);
    expect(backend.setPhotoSpecies).toHaveBeenLastCalledWith(4, [jay]);
  });

  it("joins names for labels", () => {
    expect(names([])).toBeNull();
    expect(names([robin, jay])).toBe("American Robin, Blue Jay");
  });
});
