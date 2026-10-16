import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ReanalyzeResult } from "../api";
import Reanalyze from "./Reanalyze.svelte";
import { waitingNote } from "./models";

afterEach(cleanup);

function setup(result: ReanalyzeResult) {
  const reanalyze = vi.fn(async () => result);
  render(Reanalyze, { reanalyze, label: "Re-analyze all photos" });
  return reanalyze;
}

describe("waitingNote", () => {
  it("is empty when no analysis waits for a model", () => {
    expect(waitingNote([])).toBeNull();
  });

  it("names what waits and where to download the models", () => {
    expect(
      waitingNote(["AI scores and similar photos", "faces and closed eyes"]),
    ).toBe(
      "AI analysis needs models (AI scores and similar photos; faces and closed eyes): download them in Settings → AI models.",
    );
  });
});

describe("Reanalyze", () => {
  it("says AI analysis needs models when they are not downloaded", async () => {
    setup({ photos: 2, waiting_for_models: ["faces and closed eyes"] });
    await fireEvent.click(screen.getByTestId("reanalyze"));

    expect(await screen.findByTestId("reanalyze-message")).toHaveTextContent(
      "Analyzing 2 photos again.",
    );
    expect(screen.getByTestId("reanalyze-models")).toHaveTextContent(
      "AI analysis needs models (faces and closed eyes): download them in Settings → AI models.",
    );
  });

  it("says nothing about models once they are downloaded", async () => {
    setup({ photos: 2, waiting_for_models: [] });
    await fireEvent.click(screen.getByTestId("reanalyze"));

    await screen.findByTestId("reanalyze-message");
    expect(screen.queryByTestId("reanalyze-models")).toBeNull();
  });
});
