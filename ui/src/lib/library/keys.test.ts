import { describe, expect, it } from "vitest";
import { flagForKey } from "./keys";

describe("flagForKey", () => {
  it("maps P, X and U everywhere, in either case", () => {
    for (const view of ["grid", "loupe", "compare"] as const) {
      expect(flagForKey("p", view)).toBe("pick");
      expect(flagForKey("X", view)).toBe("reject");
      expect(flagForKey("u", view)).toBe("unflagged");
    }
  });

  it("picks with ↑ and rejects with ↓ in the loupe and compare", () => {
    expect(flagForKey("ArrowUp", "loupe")).toBe("pick");
    expect(flagForKey("ArrowDown", "loupe")).toBe("reject");
    expect(flagForKey("ArrowUp", "compare")).toBe("pick");
    expect(flagForKey("ArrowDown", "compare")).toBe("reject");
  });

  it("leaves arrows to navigation in the grid", () => {
    expect(flagForKey("ArrowUp", "grid")).toBeUndefined();
    expect(flagForKey("ArrowDown", "grid")).toBeUndefined();
    expect(flagForKey("ArrowLeft", "loupe")).toBeUndefined();
  });
});
