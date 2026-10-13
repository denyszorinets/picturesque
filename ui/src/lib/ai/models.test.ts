import { describe, expect, it, vi } from "vitest";
import type { ModelView } from "../api";
import { formatSize, Models, type ModelsApi } from "./models.svelte";

function view(state: ModelView["state"], extra: Partial<ModelView> = {}) {
  return {
    id: "clip",
    name: "CLIP",
    purpose: "Search",
    version: 1,
    license: "MIT",
    license_url: "",
    homepage: "",
    size: 1000,
    state,
    downloaded: 0,
    error: null,
    ...extra,
  } satisfies ModelView;
}

function api(): ModelsApi {
  return {
    listModels: vi.fn().mockResolvedValue([view("not_installed")]),
    installModel: vi.fn().mockResolvedValue(view("downloading")),
    cancelModel: vi.fn().mockResolvedValue(view("downloading")),
    removeModel: vi.fn().mockRejectedValue({
      code: "invalid_arguments",
      message: "CLIP is downloading; cancel the download first.",
    }),
  };
}

describe("Models", () => {
  it("installs, follows progress events and reports errors", async () => {
    const models = new Models(api());
    await models.load();
    expect(models.list[0]!.state).toBe("not_installed");

    await models.install("clip");
    expect(models.list[0]!.state).toBe("downloading");
    models.handle({
      type: "model_progress",
      ...view("downloading", { downloaded: 500 }),
    });
    expect(models.list[0]!.downloaded).toBe(500);
    models.handle({ type: "model_progress", ...view("installed") });
    expect(models.list[0]!.state).toBe("installed");
    // Other events are ignored.
    models.handle({ type: "series_changed", series: 1, photos: 2 });

    await models.remove("clip");
    expect(models.error).toContain("cancel the download");
  });

  it("keeps an event that overtook its command's response", async () => {
    let respond: (v: ModelView) => void = () => {};
    const slow = api();
    slow.installModel = () => new Promise((resolve) => (respond = resolve));
    const models = new Models(slow);
    await models.load();

    const installing = models.install("clip");
    models.handle({
      type: "model_progress",
      ...view("failed", { error: "corrupt" }),
    });
    respond(view("downloading"));
    await installing;

    expect(models.list[0]!.state).toBe("failed");
  });

  it("formats sizes", () => {
    expect(formatSize(351_613_724)).toBe("351.6 MB");
    expect(formatSize(2_500_000_000)).toBe("2.5 GB");
    expect(formatSize(158)).toBe("1 KB");
  });
});
