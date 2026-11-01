import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError, type CriticStatus, type CritiqueView } from "../api";
import Critique from "./Critique.svelte";

afterEach(cleanup);

const STATUS: CriticStatus = {
  enabled: true,
  backend: "server",
  server: "http://127.0.0.1:11434/v1",
  local: true,
  reachable: true,
  models: ["qwen3-vl:4b"],
  model: "qwen3-vl:4b",
  model_installed: true,
  problem: null,
  suggested: [],
  default_rubric: "- Composition",
};

const CRITIQUE: CritiqueView = {
  model: "qwen3-vl:4b",
  rating: 7.5,
  score: 0.72,
  summary: "Warm light on a calm lake.",
  strengths: ["golden light"],
  weaknesses: ["kayaks too small"],
  issues: [],
  potential: null,
  fixable: null,
  fix_steps: [],
  improvements: [],
  camera_tips: [],
  outdated: false,
};

describe("Critique", () => {
  it("shows nothing while critiques are off", () => {
    const load = vi.fn(async () => null);
    render(Critique, {
      photoId: 1,
      status: { ...STATUS, enabled: false },
      load,
      run: vi.fn(),
    });
    expect(screen.queryByTestId("critique")).toBeNull();
    expect(load).not.toHaveBeenCalled();
  });

  it("shows a stored critique and can ask again", async () => {
    const run = vi.fn(async () => ({ ...CRITIQUE, rating: 6 }));
    render(Critique, {
      photoId: 1,
      status: STATUS,
      load: async () => CRITIQUE,
      run,
    });
    expect(await screen.findByTestId("critique-summary")).toHaveTextContent(
      "7.5/10 Warm light on a calm lake.",
    );
    expect(screen.getByTestId("critique-strengths")).toHaveTextContent(
      "golden light",
    );
    expect(screen.getByTestId("critique-weaknesses")).toHaveTextContent(
      "kayaks too small",
    );
    await fireEvent.click(screen.getByTestId("critique-again"));
    expect(run).toHaveBeenCalledWith(1, true);
    expect(await screen.findByText("6.0/10")).toBeInTheDocument();
  });

  it("critiques on request and says why it could not", async () => {
    const run = vi
      .fn()
      .mockRejectedValueOnce(
        new ApiError(
          "internal",
          "Cannot critique the photo: no model server answered.",
        ),
      )
      .mockResolvedValueOnce(CRITIQUE);
    render(Critique, {
      photoId: 3,
      status: { ...STATUS, problem: "Run `ollama pull qwen3-vl:4b`." },
      load: async () => null,
      run,
    });
    expect(await screen.findByTestId("critique-problem")).toHaveTextContent(
      "ollama pull",
    );
    await fireEvent.click(screen.getByTestId("critique-run"));
    expect(await screen.findByTestId("critique-error")).toHaveTextContent(
      "no model server answered",
    );
    await fireEvent.click(screen.getByTestId("critique-run"));
    expect(run).toHaveBeenLastCalledWith(3, false);
    expect(await screen.findByTestId("critique-summary")).toBeInTheDocument();
    expect(screen.queryByTestId("critique-error")).toBeNull();
  });

  it("shows an outdated critique with a note", async () => {
    render(Critique, {
      photoId: 1,
      status: STATUS,
      load: async () => ({ ...CRITIQUE, outdated: true }),
      run: vi.fn(),
    });
    expect(await screen.findByTestId("critique-summary")).toBeInTheDocument();
    expect(screen.getByTestId("critique-outdated")).toHaveTextContent(
      "Written by an earlier model or prompt",
    );
    expect(screen.getByTestId("critique-again")).toHaveTextContent(
      "Critique again",
    );
  });

  it("says when the stored critique could not be loaded", async () => {
    render(Critique, {
      photoId: 1,
      status: STATUS,
      load: async () => {
        throw new ApiError("internal", "catalog is busy");
      },
      run: vi.fn(),
    });
    expect(await screen.findByTestId("critique-error")).toHaveTextContent(
      "Could not load the critique: catalog is busy",
    );
    expect(screen.getByTestId("critique-run")).toBeInTheDocument();
  });

  describe("v2 fields", () => {
    const V2: CritiqueView = {
      ...CRITIQUE,
      issues: ["exposure", "horizon"],
      potential: 8.5,
      fixable: "easy",
      fix_steps: ["Lift the shadows"],
      improvements: ["Step closer to the kayaks"],
      camera_tips: ["Use f/8 at 1/250"],
    };

    async function show(c: CritiqueView) {
      render(Critique, {
        photoId: 1,
        status: STATUS,
        load: async () => c,
        run: vi.fn(),
      });
      await screen.findByTestId("critique-summary");
    }

    it("shows issue tags and the advice lists", async () => {
      await show(V2);
      const tags = screen.getByTestId("critique-issues");
      expect(tags).toHaveTextContent("Exposure");
      expect(tags).toHaveTextContent("Horizon");
      const advice = screen.getByTestId("critique-advice");
      expect(advice).toHaveTextContent("How to improve");
      expect(advice).toHaveTextContent("Step closer to the kayaks");
      expect(advice).toHaveTextContent("Camera settings");
      expect(advice).toHaveTextContent("Use f/8 at 1/250");
      expect(screen.getByTestId("critique-raw")).toHaveTextContent(
        "Lift the shadows",
      );
    });

    it.each([
      ["not_needed", "Nothing to fix in editing"],
      ["easy", "Easy to improve with editing"],
      ["needs_skill", "Can be improved with editing skill"],
      ["not_fixable", "Not fixable in editing"],
    ] as const)("words fixable %s", async (fixable, text) => {
      await show({ ...V2, fixable });
      expect(screen.getByTestId("critique-raw")).toHaveTextContent(text);
    });

    it("shows the potential only when it is clearly higher", async () => {
      await show({ ...V2, potential: 7.9 });
      expect(screen.queryByTestId("critique-potential")).toBeNull();
      cleanup();
      await show({ ...V2, potential: 8.0 });
      expect(screen.getByTestId("critique-potential")).toHaveTextContent(
        "could reach 8.0/10",
      );
    });

    it("shows none of it for an old critique", async () => {
      await show(CRITIQUE);
      expect(screen.queryByTestId("critique-issues")).toBeNull();
      expect(screen.queryByTestId("critique-advice")).toBeNull();
      expect(screen.queryByTestId("critique-raw")).toBeNull();
    });
  });
});
