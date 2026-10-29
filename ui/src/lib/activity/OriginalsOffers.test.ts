import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { OriginalsPlan, OriginalsStatus } from "../api";
import OriginalsOffers from "./OriginalsOffers.svelte";

const originalsOffers = vi.fn();
const originalsPlan = vi.fn();
const deleteOriginals = vi.fn();
const originalsStatus = vi.fn();
const cancelDeleteOriginals = vi.fn();
vi.mock("../api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../api")>()),
  originalsOffers: () => originalsOffers(),
  originalsPlan: (id: number) => originalsPlan(id),
  deleteOriginals: (id: number, through: number) =>
    deleteOriginals(id, through),
  originalsStatus: () => originalsStatus(),
  cancelDeleteOriginals: () => cancelDeleteOriginals(),
}));

const PLAN: OriginalsPlan = {
  source_id: 3,
  through_job_id: 41,
  photos: 12,
  files: 20,
  bytes: 480_000_000,
  duplicates: 2,
  changed: 1,
};

const status = (over: Partial<OriginalsStatus>): OriginalsStatus => ({
  state: "completed",
  source_id: 3,
  done: 12,
  total: 12,
  deleted: 12,
  gone: 0,
  files: 20,
  bytes: 480_000_000,
  kept: [],
  ...over,
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("OriginalsOffers", () => {
  it("shows nothing when no folder has originals to delete", async () => {
    originalsOffers.mockResolvedValue([]);
    render(OriginalsOffers, { visible: true });
    await vi.waitFor(() => expect(originalsOffers).toHaveBeenCalled());
    expect(screen.queryByTestId("originals-offers")).toBeNull();
  });

  it("does not load while hidden", () => {
    render(OriginalsOffers, { visible: false });
    expect(originalsOffers).not.toHaveBeenCalled();
  });

  it("offers a folder, shows the plan and deletes only after the click", async () => {
    originalsOffers.mockResolvedValue([
      { source_id: 3, path: "/media/card", photos: 12 },
    ]);
    originalsPlan.mockResolvedValue(PLAN);
    deleteOriginals.mockResolvedValue(status({ state: "running", done: 0 }));
    originalsStatus.mockResolvedValue(status({}));
    render(OriginalsOffers, { visible: true });

    const offer = await screen.findByTestId("originals-offer-3");
    expect(screen.getByTestId("originals-offers")).toHaveTextContent(
      "12 photos imported",
    );
    await fireEvent.click(offer);

    const summary = await screen.findByTestId("originals-summary");
    expect(summary).toHaveTextContent("12 photos (20 files");
    expect(screen.getByTestId("originals-dialog")).toHaveTextContent(
      "Deleting is permanent",
    );
    expect(screen.getByTestId("originals-dialog")).toHaveTextContent(
      "2 photos were already in your library",
    );
    expect(screen.getByTestId("originals-dialog")).toHaveTextContent(
      "1 photo changed since they were imported and stay",
    );
    // Nothing is deleted until the confirmation.
    expect(deleteOriginals).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByTestId("originals-go"));
    expect(deleteOriginals).toHaveBeenCalledWith(3, 41);

    const result = await screen.findByTestId("originals-result", undefined, {
      timeout: 3000,
    });
    expect(result).toHaveTextContent("Deleted the originals of 12 photos");
    expect(result).toHaveTextContent("Your library was not changed");
  });

  it("says what was kept and when it stopped", async () => {
    originalsOffers.mockResolvedValue([
      { source_id: 3, path: "/media/card", photos: 12 },
    ]);
    originalsPlan.mockResolvedValue(PLAN);
    deleteOriginals.mockResolvedValue(
      status({
        state: "cancelled",
        deleted: 5,
        kept: [
          { photo_id: 9, reason: "IMG_9.CR3 is not in the library" },
          { photo_id: 10, reason: "other" },
        ],
      }),
    );
    render(OriginalsOffers, { visible: true });
    await fireEvent.click(await screen.findByTestId("originals-offer-3"));
    await fireEvent.click(await screen.findByTestId("originals-go"));

    expect(await screen.findByTestId("originals-result")).toHaveTextContent(
      "Stopped.",
    );
    expect(screen.getByTestId("originals-kept")).toHaveTextContent(
      "2 photos kept:",
    );
    expect(screen.getByTestId("originals-kept")).toHaveTextContent(
      "IMG_9.CR3 is not in the library",
    );
    expect(
      screen.getByTestId("originals-kept").querySelectorAll("li"),
    ).toHaveLength(2);
  });

  it("offers no deleting when nothing is eligible", async () => {
    originalsOffers.mockResolvedValue([
      { source_id: 3, path: "/media/card", photos: 1 },
    ]);
    originalsPlan.mockResolvedValue({ ...PLAN, photos: 0, files: 0, bytes: 0 });
    render(OriginalsOffers, { visible: true });
    await fireEvent.click(await screen.findByTestId("originals-offer-3"));

    expect(await screen.findByTestId("originals-nothing")).toHaveTextContent(
      "Nothing to delete: 1 photo changed",
    );
    expect(screen.queryByTestId("originals-go")).toBeNull();
  });

  it("shows why deleting is not possible", async () => {
    originalsOffers.mockResolvedValue([
      { source_id: 3, path: "/media/card", photos: 1 },
    ]);
    originalsPlan.mockRejectedValue(new Error("it is not writable"));
    render(OriginalsOffers, { visible: true });
    await fireEvent.click(await screen.findByTestId("originals-offer-3"));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "it is not writable",
    );
    expect(screen.queryByTestId("originals-go")).toBeNull();
  });
});
