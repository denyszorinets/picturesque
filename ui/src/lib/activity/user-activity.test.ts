import { afterEach, describe, expect, it, vi } from "vitest";
import { trackActivity } from "./user-activity";

afterEach(() => vi.useRealTimers());

describe("trackActivity", () => {
  it("reports at most once per 30 seconds", () => {
    vi.useFakeTimers();
    const report = vi.fn(async () => null);
    const stop = trackActivity(report);
    window.dispatchEvent(new Event("pointermove"));
    window.dispatchEvent(new Event("keydown"));
    expect(report).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(29_000);
    window.dispatchEvent(new Event("pointerdown"));
    expect(report).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(2_000);
    window.dispatchEvent(new Event("pointerdown"));
    expect(report).toHaveBeenCalledTimes(2);
    stop();
    vi.advanceTimersByTime(60_000);
    window.dispatchEvent(new Event("keydown"));
    expect(report).toHaveBeenCalledTimes(2);
  });

  it("ignores failures", async () => {
    const stop = trackActivity(() => Promise.reject(new Error("no")));
    expect(() => window.dispatchEvent(new Event("keydown"))).not.toThrow();
    await Promise.resolve();
    stop();
  });
});
