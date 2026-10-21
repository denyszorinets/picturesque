import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it } from "vitest";
import DarktableNote from "./DarktableNote.svelte";

afterEach(() => {
  cleanup();
  localStorage.clear();
});

describe("DarktableNote", () => {
  it("explains darktable once, until dismissed", async () => {
    render(DarktableNote);
    expect(screen.getByTestId("darktable-note")).toHaveTextContent(
      "look for updated XMP files on startup",
    );

    await fireEvent.click(screen.getByTestId("darktable-note-dismiss"));
    expect(screen.queryByTestId("darktable-note")).toBeNull();

    cleanup();
    render(DarktableNote);
    expect(screen.queryByTestId("darktable-note")).toBeNull();
  });
});
