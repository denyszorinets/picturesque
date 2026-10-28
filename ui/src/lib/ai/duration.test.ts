import { describe, expect, it } from "vitest";
import { formatTimeLeft } from "./duration";

describe("formatTimeLeft", () => {
  it("uses minutes under 2 hours", () => {
    expect(formatTimeLeft(10)).toBe("about 1 min");
    expect(formatTimeLeft(2700)).toBe("about 45 min");
    expect(formatTimeLeft(7199)).toBe("about 120 min");
  });
  it("uses hours under 3 days", () => {
    expect(formatTimeLeft(7200)).toBe("about 2 h");
    expect(formatTimeLeft(86400)).toBe("about 24 h");
  });
  it("uses days beyond", () => {
    expect(formatTimeLeft(3 * 86400)).toBe("about 3 days");
    expect(formatTimeLeft(10 * 86400)).toBe("about 10 days");
  });
});
