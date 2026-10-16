import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import Tour from "./Tour.svelte";
import type { TourStep } from "./steps";

const STEPS: TourStep[] = [
  { target: "#first", title: "First", text: "The first control." },
  { target: "#missing", title: "Second", text: "Not on screen." },
  { target: "#third", title: "Third", text: "The last one." },
];

afterEach(() => {
  cleanup();
  document.body.innerHTML = "";
});

function setup() {
  for (const id of ["first", "third"]) {
    const el = document.createElement("button");
    el.id = id;
    // jsdom lays nothing out: give the controls a place on screen.
    el.getBoundingClientRect = () =>
      ({
        left: 10,
        top: 10,
        right: 60,
        bottom: 30,
        width: 50,
        height: 20,
      }) as DOMRect;
    document.body.append(el);
  }
  const onend = vi.fn();
  render(Tour, { steps: STEPS, onend });
  return onend;
}

describe("Tour", () => {
  it("goes through the steps and ends", async () => {
    const onend = setup();

    expect(screen.getByRole("dialog", { name: "First" })).toHaveTextContent(
      "1 of 3",
    );
    expect(screen.getByTestId("tour-highlight")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Next" }));

    // A control that is not on screen: the step is shown without a highlight.
    expect(screen.getByRole("dialog", { name: "Second" })).toBeInTheDocument();
    expect(screen.queryByTestId("tour-highlight")).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(screen.getByRole("dialog", { name: "First" })).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Next" }));
    await fireEvent.click(screen.getByRole("button", { name: "Next" }));
    expect(screen.getByRole("dialog", { name: "Third" })).toHaveTextContent(
      "3 of 3",
    );
    await fireEvent.click(screen.getByRole("button", { name: "Done" }));
    expect(onend).toHaveBeenCalledOnce();
  });

  it("can be skipped with the button or Escape", async () => {
    const onend = setup();

    await fireEvent.keyDown(window, { key: "Escape" });
    expect(onend).toHaveBeenCalledOnce();
    await fireEvent.click(screen.getByRole("button", { name: "Skip tour" }));
    expect(onend).toHaveBeenCalledTimes(2);
  });

  it("moves with the arrow keys", async () => {
    setup();

    await fireEvent.keyDown(window, { key: "ArrowRight" });
    expect(screen.getByRole("dialog", { name: "Second" })).toBeInTheDocument();
    await fireEvent.keyDown(window, { key: "ArrowLeft" });
    expect(screen.getByRole("dialog", { name: "First" })).toBeInTheDocument();
  });
});
