import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import FinishDialog from "./FinishDialog.svelte";

const finishStatus = vi.fn();
const finishFolder = vi.fn();
vi.mock("../api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../api")>()),
  finishStatus: () => finishStatus(),
  finishFolder: () => finishFolder(),
}));

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("FinishDialog", () => {
  it("says what finishing does, finishes, and offers next steps", async () => {
    finishStatus.mockResolvedValue({
      auto_picked: 3,
      labels_to_write: 3,
      picks: 5,
      rejects: 2,
    });
    finishFolder.mockResolvedValue({
      auto_labeled: 3,
      written: 8,
      failed: 0,
      error: null,
    });
    const onnextfolder = vi.fn();
    const onclose = vi.fn();
    render(FinishDialog, { autoLabel: "yellow", onclose, onnextfolder });

    const summary = await screen.findByTestId("finish-summary");
    expect(summary).toHaveTextContent("5 picks, 2 rejects");
    expect(summary).toHaveTextContent(
      "3 auto-picked photos get the yellow colour label",
    );

    await fireEvent.click(screen.getByTestId("finish-go"));
    expect(await screen.findByTestId("finish-result")).toHaveTextContent(
      "3 auto-picked photos have the yellow label",
    );
    await fireEvent.click(screen.getByTestId("finish-next-folder"));
    expect(onclose).toHaveBeenCalled();
    expect(onnextfolder).toHaveBeenCalled();
  });

  it("reports sidecars that could not be written", async () => {
    finishStatus.mockResolvedValue({
      auto_picked: 0,
      labels_to_write: 0,
      picks: 1,
      rejects: 0,
    });
    finishFolder.mockResolvedValue({
      auto_labeled: 0,
      written: 0,
      failed: 1,
      error: "Read-only file system",
    });
    render(FinishDialog, { autoLabel: null, onclose: vi.fn() });

    await fireEvent.click(await screen.findByTestId("finish-go"));

    expect(await screen.findByTestId("finish-result")).toHaveTextContent(
      "1 sidecar could not be written: Read-only file system",
    );
  });
});
