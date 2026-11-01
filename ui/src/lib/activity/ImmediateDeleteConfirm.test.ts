import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import ImmediateDeleteConfirm from "./ImmediateDeleteConfirm.svelte";

afterEach(() => cleanup());

describe("ImmediateDeleteConfirm", () => {
  it("says deleting is permanent and names the folder", () => {
    render(ImmediateDeleteConfirm, {
      scope: "/media/card",
      onconfirm: vi.fn(),
      oncancel: vi.fn(),
    });
    const dialog = screen.getByTestId("immediate-dialog");
    expect(dialog).toHaveTextContent("permanent");
    expect(dialog).toHaveTextContent("no trash and no undo");
    expect(dialog).toHaveTextContent("/media/card");
  });

  it("confirms only with the explicit button", async () => {
    const onconfirm = vi.fn();
    const oncancel = vi.fn();
    render(ImmediateDeleteConfirm, { onconfirm, oncancel });

    await fireEvent.click(screen.getByTestId("immediate-confirm"));

    expect(onconfirm).toHaveBeenCalledOnce();
    expect(oncancel).not.toHaveBeenCalled();
  });

  it("cancels with the button and with Escape, never confirming", async () => {
    const onconfirm = vi.fn();
    const oncancel = vi.fn();
    render(ImmediateDeleteConfirm, { onconfirm, oncancel });

    await fireEvent.click(screen.getByTestId("immediate-cancel"));
    await fireEvent.keyDown(window, { key: "Escape" });

    expect(oncancel).toHaveBeenCalledTimes(2);
    expect(onconfirm).not.toHaveBeenCalled();
  });
});
