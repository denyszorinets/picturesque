import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { CriticStatus } from "../api";
import CriticFields from "./CriticFields.svelte";

afterEach(cleanup);

const STATUS: CriticStatus = {
  enabled: true,
  server: "http://127.0.0.1:11434/v1",
  local: true,
  reachable: true,
  models: ["llava:7b"],
  model: "llava:7b",
  model_installed: true,
  problem: null,
  suggested: [
    {
      name: "qwen3-vl:4b",
      size: 3_300_000_000,
      license: "Apache-2.0",
      license_url: "https://huggingface.co/Qwen/Qwen3-VL-4B-Instruct",
      note: "Recommended.",
    },
  ],
  default_rubric: "- Composition",
};

function setup(props: Partial<Record<string, unknown>> = {}) {
  const oncheck = vi.fn();
  render(CriticFields, {
    enabled: true,
    server: "http://127.0.0.1:11434/v1",
    model: "llava:7b",
    rubric: "",
    status: STATUS,
    oncheck,
    unsaved: false,
    ...props,
  });
  return oncheck;
}

describe("CriticFields", () => {
  it("hides the details while off", () => {
    setup({ enabled: false });
    expect(screen.queryByTestId("critic-server")).toBeNull();
  });

  it("offers suggested models with their license", async () => {
    setup();
    expect(screen.getByTestId("critic-suggested")).toHaveTextContent(
      "qwen3-vl:4b · 3.3 GB · Apache-2.0 · Recommended.",
    );
    await fireEvent.click(screen.getByRole("button", { name: "qwen3-vl:4b" }));
    expect(screen.getByTestId("critic-model")).toHaveValue("qwen3-vl:4b");
    expect(screen.getByTestId("critic-status")).toHaveTextContent(
      "Ready: llava:7b is installed.",
    );
  });

  it("warns when photos would leave this computer", () => {
    setup({ server: "https://api.example.com/v1" });
    expect(screen.getByTestId("critic-remote")).toBeVisible();
    cleanup();
    setup({ server: "http://[::1]:8080/v1" });
    expect(screen.queryByTestId("critic-remote")).toBeNull();
  });

  it("shows the default rubric and restores it", async () => {
    setup({ rubric: "Only the light." });
    expect(screen.getByTestId("critic-rubric")).toHaveAttribute(
      "placeholder",
      "- Composition",
    );
    await fireEvent.click(screen.getByTestId("critic-rubric-default"));
    expect(screen.getByTestId("critic-rubric")).toHaveValue("");
  });

  it("says what is wrong and checks again", async () => {
    const oncheck = setup({
      status: { ...STATUS, reachable: false, problem: "Install Ollama." },
    });
    expect(screen.getByTestId("critic-status")).toHaveTextContent(
      "Install Ollama.",
    );
    await fireEvent.click(screen.getByTestId("critic-check"));
    expect(oncheck).toHaveBeenCalled();
    cleanup();
    setup({ unsaved: true });
    expect(screen.getByTestId("critic-status")).toHaveTextContent(
      "Save to use these settings.",
    );
    expect(screen.queryByTestId("critic-check")).toBeNull();
  });
});
