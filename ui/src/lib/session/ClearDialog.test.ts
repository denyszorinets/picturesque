import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import ClearDialog from "./ClearDialog.svelte";

const clearPlan = vi.fn();
const clearFolder = vi.fn();
vi.mock("../api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../api")>()),
  clearPlan: () => clearPlan(),
  clearFolder: () => clearFolder(),
}));

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("ClearDialog", () => {
  it("needs its own yes before deleting rejects without a copy", async () => {
    clearPlan.mockResolvedValue({
      mode: "verified_and_rejects",
      photos: 3,
      files: 5,
      bytes: 50_000_000,
      rejects: 1,
      partly_copied: 0,
    });
    clearFolder.mockResolvedValue({
      deleted: 3,
      files: 5,
      bytes: 50_000_000,
      kept: [],
    });
    const oncleared = vi.fn();
    render(ClearDialog, { folder: "/card", onclose: vi.fn(), oncleared });

    expect(await screen.findByTestId("clear-summary")).toHaveTextContent(
      "Deletes 3 photos (5 files",
    );
    const go = screen.getByTestId("clear-go");
    expect(go).toBeDisabled();

    await fireEvent.click(screen.getByTestId("clear-rejects-confirm"));
    expect(go).toBeEnabled();
    await fireEvent.click(go);

    expect(await screen.findByTestId("clear-result")).toHaveTextContent(
      "Deleted 3 photos",
    );
    expect(oncleared).toHaveBeenCalled();
  });

  it("says when there is nothing to delete", async () => {
    clearPlan.mockResolvedValue({
      mode: "verified",
      photos: 0,
      files: 0,
      bytes: 0,
      rejects: 0,
      partly_copied: 0,
    });
    render(ClearDialog, { folder: "/card", onclose: vi.fn() });

    expect(await screen.findByTestId("clear-nothing")).toBeInTheDocument();
    expect(screen.queryByTestId("clear-go")).toBeNull();
  });
});
