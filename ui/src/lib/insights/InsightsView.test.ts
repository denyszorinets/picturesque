import { cleanup, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent } from "@testing-library/svelte";
import type { LibraryInsights } from "../api";
import InsightsView from "./InsightsView.svelte";

const EMPTY: LibraryInsights = {
  critiques: 0,
  with_issues: 0,
  min_critiques: 10,
  half_life_days: 365,
  as_of: null,
  mean_rating: null,
  weighted_rating: null,
  issues: [],
  trend: [],
  trend_slope: null,
  habits: [],
  advice: [],
};

const FULL: LibraryInsights = {
  ...EMPTY,
  critiques: 40,
  with_issues: 38,
  as_of: "2026-10-01",
  mean_rating: 5.2,
  weighted_rating: 5.8,
  issues: [
    {
      issue: "noise",
      count: 20,
      share: 0.5,
      weighted_share: 0.62,
      examples: [7, 9],
    },
    {
      issue: "focus",
      count: 8,
      share: 0.2,
      weighted_share: 0.1,
      examples: [3],
    },
  ],
  trend: [
    { quarter: "2026-Q1", critiques: 10, mean_rating: 4.5 },
    { quarter: "2026-Q2", critiques: 15, mean_rating: 5.5 },
    { quarter: "2026-Q3", critiques: 15, mean_rating: 6.5 },
  ],
  trend_slope: 2.0,
  habits: [
    {
      issue: "noise",
      setting: "iso",
      label: "ISO above 3200",
      critiques: 12,
      share: 0.75,
      baseline: 0.4,
      lift: 1.875,
    },
  ],
  advice: [
    {
      issue: "overexposed",
      title: "You tend to overexpose",
      text: "Dial in about -2/3 EV of exposure compensation.",
      evidence: "Named in 31% of your recent critiques (12 photos)",
      examples: [4, 5],
    },
  ],
};

const show = (insights: LibraryInsights, extra = {}) =>
  render(InsightsView, {
    props: {
      load: () => Promise.resolve(insights),
      visible: true,
      imageSrc: (id: number) => `/photo/${id}.jpg`,
      ...extra,
    },
  });

afterEach(() => cleanup());

describe("InsightsView", () => {
  it("explains that there are no critiques yet and opens the settings", async () => {
    const onsettings = vi.fn();
    show(EMPTY, { onsettings });
    expect(await screen.findByTestId("insights-empty")).toHaveTextContent(
      "No critiques yet",
    );
    await fireEvent.click(
      screen.getByRole("button", { name: "Open settings" }),
    );
    expect(onsettings).toHaveBeenCalled();
  });

  it("does not load while hidden", () => {
    const load = vi.fn(() => Promise.resolve(EMPTY));
    render(InsightsView, { props: { load, visible: false } });
    expect(load).not.toHaveBeenCalled();
  });

  it("shows problems, examples, habits and the trend", async () => {
    show(FULL);
    const noise = await screen.findByTestId("insights-issue-noise");
    expect(noise).toHaveTextContent("Noise");
    expect(noise).toHaveTextContent("62%");
    expect(
      noise.querySelectorAll("[data-testid=insights-example]"),
    ).toHaveLength(2);
    expect(noise.querySelector("img")).toHaveAttribute("src", "/photo/7.jpg");
    expect(screen.getByTestId("insights-issue-focus")).toBeVisible();
    expect(screen.getByTestId("insights-habit")).toHaveTextContent(
      "Noise shows up 1.9× as often at ISO above 3200",
    );
    expect(screen.getByTestId("insights-rating")).toHaveTextContent("5.8");
    expect(screen.getByTestId("insights-slope")).toHaveTextContent(
      "rising by about 2.0 points a year",
    );
    expect(screen.queryByTestId("insights-few")).toBeNull();
  });

  it("leads with advice: what to do, the evidence and examples", async () => {
    show(FULL);
    const advice = await screen.findByTestId("insights-advice-overexposed");
    expect(advice).toHaveTextContent("You tend to overexpose");
    expect(advice).toHaveTextContent("-2/3 EV");
    expect(advice).toHaveTextContent("31% of your recent critiques");
    expect(advice.querySelectorAll("img")).toHaveLength(2);
    // Before the list of recurring problems.
    const sections = screen.getByTestId("insights").querySelectorAll("section");
    expect(sections[0]).toHaveAttribute("data-testid", "insights-advice");
  });

  it("shows no advice section when there is none", async () => {
    show({ ...FULL, advice: [] });
    await screen.findByTestId("insights-issue-noise");
    expect(screen.queryByTestId("insights-advice")).toBeNull();
  });

  it("says when there are few critiques or no problem categories", async () => {
    show({ ...FULL, critiques: 4, with_issues: 0, issues: [], habits: [] });
    expect(await screen.findByTestId("insights-few")).toHaveTextContent(
      "Based on 4 critiques",
    );
    expect(screen.getByTestId("insights-no-issues")).toBeVisible();
  });

  it("says so when no critique names a problem", async () => {
    show({ ...FULL, with_issues: 38, issues: [], habits: [] });
    expect(await screen.findByTestId("insights-none")).toBeVisible();
  });

  it("keeps the newest load when an older one finishes last", async () => {
    let resolveOld!: (i: LibraryInsights) => void;
    const old = new Promise<LibraryInsights>((r) => (resolveOld = r));
    const load = vi
      .fn()
      .mockReturnValueOnce(old)
      .mockReturnValueOnce(Promise.resolve(FULL));
    const view = render(InsightsView, { props: { load, visible: true } });
    await view.rerender({ load, visible: false });
    await view.rerender({ load, visible: true });
    expect(await screen.findByTestId("insights-rating")).toHaveTextContent(
      "5.8",
    );
    resolveOld(EMPTY);
    await Promise.resolve();
    expect(screen.getByTestId("insights-rating")).toHaveTextContent("5.8");
    expect(screen.queryByTestId("insights-empty")).toBeNull();
  });

  it("shows a load failure", async () => {
    render(InsightsView, {
      props: { load: () => Promise.reject(new Error("boom")), visible: true },
    });
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent("boom"),
    );
  });
});
