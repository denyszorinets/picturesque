import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { BackgroundCritiqueStatus, CriticStatus } from "../api";
import { validPercent } from "../settings/percent";
import CriticFields from "./CriticFields.svelte";

afterEach(cleanup);

const STATUS: CriticStatus = {
  enabled: true,
  backend: "server",
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

const PROGRESS: BackgroundCritiqueStatus = {
  enabled: true,
  running: false,
  paused: null,
  critiqued: 120,
  stale: 0,
  missing: 880,
  failed: 0,
  total: 1000,
  seconds_per_photo: 15,
  seconds_left: 13_200,
};

function setup(props: Partial<Record<string, unknown>> = {}) {
  const oncheck = vi.fn();
  render(CriticFields, {
    enabled: true,
    backend: "server",
    server: "http://127.0.0.1:11434/v1",
    model: "llava:7b",
    preset: "amateur",
    rubric: "",
    background: true,
    intensity: "gentle",
    scorePercent: 25,
    progress: PROGRESS,
    status: STATUS,
    oncheck,
    unsaved: false,
    ...props,
  });
  return oncheck;
}

describe("CriticFields background critique", () => {
  it("shows its options only while critiques are on", () => {
    setup({ enabled: false });
    expect(screen.queryByTestId("critic-background")).toBeNull();
    cleanup();
    setup();
    expect(screen.getByTestId("critic-background")).toBeChecked();
    expect(screen.getByTestId("critic-intensity")).toHaveValue("gentle");
  });

  it("flags an empty or out-of-range percentage", async () => {
    setup();
    const input = screen.getByTestId("critic-score-weight");
    expect(screen.queryByTestId("critic-score-weight-error")).toBeNull();
    await fireEvent.input(input, { target: { value: "" } });
    expect(screen.getByTestId("critic-score-weight-error")).toHaveTextContent(
      "Enter a percentage from 0 to 100.",
    );
    await fireEvent.input(input, { target: { value: "40" } });
    expect(screen.queryByTestId("critic-score-weight-error")).toBeNull();
    await fireEvent.input(input, { target: { value: "101" } });
    expect(screen.getByTestId("critic-score-weight-error")).toBeVisible();
    await fireEvent.input(input, { target: { value: "12.5" } });
    expect(screen.getByTestId("critic-score-weight-error")).toBeVisible();
  });

  it("validates whole percentages from 0 to 100", () => {
    expect(validPercent(0)).toBe(true);
    expect(validPercent(100)).toBe(true);
    expect(validPercent(null)).toBe(false);
    expect(validPercent(Number.NaN)).toBe(false);
    expect(validPercent(-1)).toBe(false);
    expect(validPercent(101)).toBe(false);
  });

  it("hides the intensity while the background critic is off", () => {
    setup({ background: false });
    expect(screen.queryByTestId("critic-intensity")).toBeNull();
    expect(screen.queryByTestId("critic-progress")).toBeNull();
  });

  it("shows progress with the time left", () => {
    setup();
    expect(screen.getByTestId("critic-progress")).toHaveTextContent(
      "120 of 1000 photos critiqued. about 4 h left.",
    );
  });

  it("shows stale photos and that it is running", () => {
    setup({ progress: { ...PROGRESS, stale: 30, running: true } });
    const text = screen.getByTestId("critic-progress");
    expect(text).toHaveTextContent("30 to redo with the new model or rubric.");
    expect(text).toHaveTextContent("Critiquing now");
  });

  it("shows why it waits", () => {
    setup({ progress: { ...PROGRESS, paused: "on battery" } });
    expect(screen.getByTestId("critic-progress")).toHaveTextContent(
      "Paused: on battery",
    );
  });

  it("says when done", () => {
    setup({
      progress: {
        ...PROGRESS,
        missing: 0,
        critiqued: 1000,
        seconds_left: 0,
      },
    });
    expect(screen.getByTestId("critic-progress")).toHaveTextContent("Done");
  });

  it("asks to save instead of showing progress", () => {
    setup({ unsaved: true });
    expect(screen.getByTestId("critic-progress")).toHaveTextContent(
      "Save to use these settings.",
    );
  });

  it("edits the score weight as a percent", async () => {
    setup({ scorePercent: 25 });
    const input = screen.getByTestId("critic-score-weight");
    expect(input).toHaveValue(25);
    await fireEvent.input(input, { target: { value: "40" } });
    expect(input).toHaveValue(40);
  });
});

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

  it("needs no server for the built-in critic", () => {
    setup({
      backend: "builtin",
      status: {
        ...STATUS,
        backend: "builtin",
        model: "Qwen3-VL 4B (built-in)",
        models: [],
      },
    });
    expect(screen.getByTestId("critic-builtin")).toBeChecked();
    expect(screen.queryByTestId("critic-server")).toBeNull();
    expect(screen.queryByTestId("critic-suggested")).toBeNull();
    expect(screen.getByTestId("critic-rubric")).toBeVisible();
    expect(screen.getByTestId("critic-status")).toHaveTextContent(
      "Ready: the built-in critic runs on this computer.",
    );
  });

  it("switches to a server", async () => {
    setup({ backend: "builtin" });
    await fireEvent.click(screen.getByTestId("critic-use-server"));
    expect(screen.getByTestId("critic-server")).toBeVisible();
  });
});

describe("CriticFields critique style", () => {
  it("shows the saved style and lets another be picked", async () => {
    setup({ preset: "professional" });
    expect(screen.getByTestId("critic-preset-professional")).toBeChecked();
    expect(screen.getByTestId("critic-preset-amateur")).not.toBeChecked();
    expect(screen.getByTestId("critic-preset-hint")).toHaveTextContent(
      "outdated",
    );
    await fireEvent.click(screen.getByTestId("critic-preset-beginner"));
    expect(screen.getByTestId("critic-preset-beginner")).toBeChecked();
    expect(screen.getByTestId("critic-preset-professional")).not.toBeChecked();
  });

  it("hides the style while critiques are off", () => {
    setup({ enabled: false });
    expect(screen.queryByTestId("critic-preset-amateur")).toBeNull();
  });
});
