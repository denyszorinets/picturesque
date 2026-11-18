import { render, screen, fireEvent, waitFor } from "@testing-library/svelte";
import { beforeEach, expect, it, vi } from "vitest";
import ReviewDialog from "./ReviewDialog.svelte";
import * as api from "../api";
vi.mock("../api", async (original) => ({
  ...(await original<typeof api>()),
  reviewPlan: vi.fn(),
  applyReview: vi.fn(),
}));

beforeEach(() => {
  vi.mocked(api.reviewPlan)
    .mockReset()
    .mockImplementation(async (q) => ({
      token: q.flags ? "picks" : "dates",
      count: q.flags ? 7 : 100,
      first_day: "2026-11-17",
      last_day: "2026-11-17",
      undated: 0,
    }));
  vi.mocked(api.applyReview).mockReset().mockResolvedValue(7);
});

it("defaults to exactly the filtered view; date expansion requires an explicit choice", async () => {
  const query: api.PhotoQuery = {
    from: "2026-11-17",
    to: "2026-11-17",
    flags: ["pick"],
    auto_picked: true,
    reviewed: false,
    collapse_series: true,
  };
  const changed = vi.fn();
  render(ReviewDialog, {
    query,
    description: "Picks · Auto-picked · Not reviewed · stacked",
    folder: "/synthetic",
    onclose: vi.fn(),
    onchanged: changed,
  });
  await waitFor(() =>
    expect(screen.getByTestId("review-count")).toHaveTextContent("7 photos"),
  );
  expect(api.reviewPlan).toHaveBeenLastCalledWith(query);
  expect(api.applyReview).not.toHaveBeenCalled();
  await fireEvent.click(screen.getByRole("radio", { name: /All photos from/ }));
  await waitFor(() =>
    expect(screen.getByTestId("review-count")).toHaveTextContent("100 photos"),
  );
  expect(api.reviewPlan).toHaveBeenLastCalledWith({
    from: "2026-11-17",
    to: "2026-11-17",
  });
  await fireEvent.click(screen.getByRole("button", { name: "Mark reviewed" }));
  await waitFor(() => expect(changed).toHaveBeenCalledOnce());
  expect(api.applyReview).toHaveBeenCalledWith("dates", true);
});

it("can reverse a filtered mark and never offers unbounded date expansion", async () => {
  render(ReviewDialog, {
    query: { reviewed: true, from: "2026-11-17" },
    description: "Reviewed",
    folder: "/synthetic",
    onclose: vi.fn(),
    onchanged: vi.fn(),
  });
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Mark not reviewed" }),
    ).toBeEnabled(),
  );
  expect(screen.queryByRole("radio", { name: /All photos from/ })).toBeNull();
  await fireEvent.click(
    screen.getByRole("button", { name: "Mark not reviewed" }),
  );
  await waitFor(() =>
    expect(api.applyReview).toHaveBeenCalledWith("dates", false),
  );
});

it("shows a storage error without reporting success", async () => {
  vi.mocked(api.applyReview).mockRejectedValue(new Error("disk full"));
  const changed = vi.fn();
  render(ReviewDialog, {
    query: {},
    description: "All photos",
    folder: "/synthetic",
    onclose: vi.fn(),
    onchanged: changed,
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Mark reviewed" })).toBeEnabled(),
  );
  await fireEvent.click(screen.getByRole("button", { name: "Mark reviewed" }));
  await waitFor(() =>
    expect(screen.getByRole("alert")).toHaveTextContent("disk full"),
  );
  expect(changed).not.toHaveBeenCalled();
});
