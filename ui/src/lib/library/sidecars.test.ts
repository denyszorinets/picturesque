import { describe, expect, it, vi } from "vitest";
import type { SidecarStatus } from "../api";
import { SidecarProgress, joinPath } from "./sidecars.svelte";

function status(over: Partial<SidecarStatus> = {}): SidecarStatus {
  return { pending: 0, failed: 0, error: null, folder: "/photos", ...over };
}

describe("SidecarProgress", () => {
  it("says nothing until a library is set", async () => {
    const progress = new SidecarProgress({
      sidecarStatus: vi.fn(async () => status({ folder: null })),
    });
    await progress.load();
    expect(progress.label).toBeNull();
  });

  it("follows saving, saved and failed states from events", async () => {
    const progress = new SidecarProgress({
      sidecarStatus: vi.fn(async () => status()),
    });
    await progress.load();
    expect(progress.label?.text).toBe("Saved to XMP ✓");
    expect(progress.label?.title).toContain("/photos");

    progress.handle({ type: "sidecar_progress", ...status({ pending: 3 }) });
    expect(progress.label).toMatchObject({
      text: "Saving to XMP…",
      tone: "busy",
    });

    progress.handle({
      type: "sidecar_progress",
      ...status({ pending: 1, failed: 1, error: "disk full" }),
    });
    expect(progress.label).toMatchObject({
      text: "XMP: 1 not saved",
      tone: "error",
    });
    expect(progress.label?.title).toContain("disk full");
  });

  it("keeps the last status when loading fails", async () => {
    const progress = new SidecarProgress({
      sidecarStatus: vi.fn(async () => {
        throw new Error("offline");
      }),
    });
    await progress.load();
    expect(progress.status).toBeNull();
  });
});

describe("joinPath", () => {
  it("uses the folder's separator", () => {
    expect(joinPath("/photos/", "2026/a.CR3.xmp")).toBe(
      "/photos/2026/a.CR3.xmp",
    );
    expect(joinPath("C:\\Photos", "2026/a.CR3.xmp")).toBe(
      "C:\\Photos\\2026\\a.CR3.xmp",
    );
  });
});
