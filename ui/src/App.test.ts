import { cleanup, render, screen } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App.svelte";

const appInfo = vi.fn();
vi.mock("./lib/api", () => ({ appInfo: () => appInfo() }));

describe("App", () => {
  // Hooks use block bodies: a function returned from beforeEach runs as teardown.
  beforeEach(() => {
    appInfo.mockReset();
  });
  afterEach(() => {
    cleanup();
  });

  it("shows the backend name and version", async () => {
    appInfo.mockResolvedValue({ name: "Picturesque", version: "0.1.0" });

    render(App);

    expect(await screen.findByTestId("app-version")).toHaveTextContent(
      "Picturesque 0.1.0",
    );
  });

  it("shows an error when the backend is unavailable", async () => {
    appInfo.mockRejectedValue(new Error("no backend"));

    render(App);

    expect(await screen.findByRole("alert")).toHaveTextContent("no backend");
  });
});
