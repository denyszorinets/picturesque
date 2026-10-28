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
});
