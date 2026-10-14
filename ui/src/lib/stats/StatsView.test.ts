import { cleanup, render, screen, within } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { GearStats, KeeperCounts, LibraryStats } from "../api";
import StatsView from "./StatsView.svelte";

const counts = (
  photos: number,
  picks: number,
  auto_picked: number,
  rejects = 0,
): KeeperCounts => ({ photos, picks, auto_picked, rejects });

function gear(
  label: string,
  c: KeeperCounts,
  focal: [number, number][] = [],
): GearStats {
  return {
    label,
    camera: null,
    lens: null,
    counts: c,
    keeper_rate: (c.picks + c.auto_picked) / c.photos,
    few_photos: c.photos < 20,
    focal_lengths: focal.map(([value, count]) => ({ value, count })),
  };
}

const STATS: LibraryStats = {
  all: counts(203, 50, 21, 30),
  keeper_rate: 71 / 203,
  cameras: [
    gear("Canon EOS R6", counts(100, 30, 10, 20), [
      [35, 50],
      [50, 30],
      [85, 10],
      [24, 5],
      [70, 3],
      [16, 2],
    ]),
    gear("Canon EOS R5", counts(100, 20, 10, 10), [[24, 100]]),
    gear("Apple iPhone 15 Pro", counts(3, 0, 1)),
  ],
  lenses: [gear("RF35mm F1.8", counts(60, 20, 10))],
  min_photos: 20,
};

afterEach(() => cleanup());

describe("StatsView", () => {
  it("ranks gear by keepers and lists focal lengths per camera", async () => {
    render(StatsView, {
      props: { load: () => Promise.resolve(STATS), visible: true },
    });

    expect(await screen.findByTestId("best-camera")).toHaveTextContent(
      "Canon EOS R6",
    );
    expect(screen.getByTestId("best-lens")).toHaveTextContent("RF35mm F1.8");
    expect(screen.getByTestId("overall-rate")).toHaveTextContent("35%");

    const cameras = within(screen.getByTestId("camera-stats"));
    const rows = cameras.getAllByTestId("gear-row");
    expect(rows.map((r) => r.querySelector("th")?.textContent)).toEqual([
      "Canon EOS R6",
      "Canon EOS R5",
      "Apple iPhone 15 Pro",
    ]);
    expect(within(rows[0]!).getByTestId("keeper-rate")).toHaveTextContent(
      "40%",
    );
    // Gear with few photos is set apart.
    expect(cameras.getByText(/Fewer than 20 photos/)).toBeInTheDocument();

    const focal = screen.getAllByTestId("focal-lengths");
    expect(focal).toHaveLength(2);
    expect(focal[0]).toHaveTextContent("35 mm");
    expect(focal[0]).toHaveTextContent("50%");
    // Five shown, the rest summarized.
    expect(focal[0]!.querySelectorAll("li")).toHaveLength(5);
    expect(focal[0]).toHaveTextContent("and 1 other");
  });

  it("has no leader when every camera has few photos", async () => {
    render(StatsView, {
      props: {
        load: () =>
          Promise.resolve({
            ...STATS,
            cameras: [gear("Apple iPhone 15 Pro", counts(3, 0, 1))],
          }),
        visible: true,
      },
    });
    expect(
      await screen.findByText("Not enough photos with camera information."),
    ).toBeInTheDocument();
  });

  it("explains an empty library and loads only when visible", async () => {
    const load = vi.fn().mockResolvedValue({
      ...STATS,
      all: counts(0, 0, 0),
      cameras: [],
      lenses: [],
    });
    const view = render(StatsView, { props: { load, visible: false } });
    expect(load).not.toHaveBeenCalled();
    await view.rerender({ load, visible: true });
    expect(await screen.findByTestId("stats-empty")).toBeInTheDocument();
    expect(load).toHaveBeenCalledTimes(1);
  });

  it("shows an error instead of loading forever when drawing fails", async () => {
    // Two rows with one name cannot be drawn (the backend merges them).
    const twice = gear("Canon EOS R5", counts(30, 3, 0));
    render(StatsView, {
      props: {
        load: () => Promise.resolve({ ...STATS, cameras: [twice, twice] }),
        visible: true,
      },
    });
    expect(await screen.findByTestId("stats-failed")).toHaveTextContent(
      "Statistics could not be shown",
    );
  });
});
