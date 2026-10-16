import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import HelpDialog from "./HelpDialog.svelte";
import { GUIDES, SHORTCUTS } from "./shortcuts";

const openExternal = vi.fn<(url: string) => Promise<void>>(async () => {});
vi.mock("../api/links", () => ({
  openExternal: (url: string) => openExternal(url),
}));

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("SHORTCUTS", () => {
  it("lists culling, navigation, loupe and compare keys", () => {
    const keys = SHORTCUTS.flatMap((g) => g.items.map((i) => i.keys.join("+")));
    for (const key of [
      "P",
      "X",
      "U",
      "1–5",
      "C",
      "B",
      "Enter",
      "/",
      "Ctrl+Z",
      "?",
    ])
      expect(keys).toContain(key);
    expect(SHORTCUTS.map((g) => g.title)).toEqual([
      "Culling",
      "Moving around",
      "Series",
      "Photo (loupe)",
      "Compare",
      "Anywhere",
    ]);
  });
});

describe("HelpDialog", () => {
  it("shows every shortcut group", () => {
    render(HelpDialog, { onclose: vi.fn() });

    for (const group of SHORTCUTS)
      expect(
        screen.getByRole("heading", { name: group.title }),
      ).toBeInTheDocument();
    const culling = screen.getByTestId("shortcuts-Culling");
    expect(within(culling).getByText("Pick")).toBeInTheDocument();
  });

  it("opens the guides in the browser", async () => {
    render(HelpDialog, { onclose: vi.fn() });

    await fireEvent.click(screen.getByRole("link", { name: GUIDES[0]!.label }));
    expect(openExternal).toHaveBeenCalledWith(GUIDES[0]!.url);
    expect(GUIDES.every((g) => g.url.startsWith("https://"))).toBe(true);
  });

  it("closes with Escape or Close", async () => {
    const onclose = vi.fn();
    render(HelpDialog, { onclose });

    await fireEvent.keyDown(window, { key: "Escape" });
    await fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(onclose).toHaveBeenCalledTimes(2);
  });
});
