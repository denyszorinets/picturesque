import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it } from "vitest";
import TasteFields from "./TasteFields.svelte";

afterEach(cleanup);

describe("TasteFields", () => {
  it("flags an empty or out-of-range percentage without clamping it", async () => {
    render(TasteFields, { percent: 50, taste: null });
    const input = screen.getByTestId("taste-weight") as HTMLInputElement;
    expect(screen.queryByTestId("taste-weight-error")).toBeNull();
    await fireEvent.input(input, { target: { value: "" } });
    expect(screen.getByTestId("taste-weight-error")).toHaveTextContent(
      "Enter a percentage from 0 to 100.",
    );
    await fireEvent.input(input, { target: { value: "150" } });
    expect(screen.getByTestId("taste-weight-error")).toBeVisible();
    expect(input.value).toBe("150");
    await fireEvent.input(input, { target: { value: "80" } });
    expect(screen.queryByTestId("taste-weight-error")).toBeNull();
  });
});
