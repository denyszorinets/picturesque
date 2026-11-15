import { describe, expect, it } from "vitest";
import { entryMember, step } from "./stack-nav";

describe("step", () => {
  const members = [11, 12, 13];

  it.each([
    // [currentId, delta, expected]
    [11, 1, { kind: "member", id: 12 }],
    [12, 1, { kind: "member", id: 13 }],
    [13, -1, { kind: "member", id: 12 }],
    [12, -1, { kind: "member", id: 11 }],
  ] as const)(
    "moves within the stack from %i by %i",
    (currentId, delta, expected) => {
      expect(step({ index: 4, total: 10, members, currentId, delta })).toEqual(
        expected,
      );
    },
  );

  it("goes to the next entry's first photo after the last member", () => {
    expect(
      step({ index: 4, total: 10, members, currentId: 13, delta: 1 }),
    ).toEqual({
      kind: "entry",
      index: 5,
      entry: "first",
    });
  });

  it("goes to the previous entry's last photo before the first member", () => {
    expect(
      step({ index: 4, total: 10, members, currentId: 11, delta: -1 }),
    ).toEqual({
      kind: "entry",
      index: 3,
      entry: "last",
    });
  });

  it("jumps whole stacks to each stack's shown photo", () => {
    expect(
      step({
        index: 4,
        total: 10,
        members,
        currentId: 12,
        delta: 1,
        wholeStack: true,
      }),
    ).toEqual({ kind: "entry", index: 5, entry: "shown" });
    expect(
      step({
        index: 4,
        total: 10,
        members,
        currentId: 12,
        delta: -1,
        wholeStack: true,
      }),
    ).toEqual({ kind: "entry", index: 3, entry: "shown" });
  });

  it("moves between entries when there is no stack", () => {
    expect(
      step({ index: 0, total: 3, members: [], currentId: 7, delta: 1 }),
    ).toEqual({
      kind: "entry",
      index: 1,
      entry: "first",
    });
  });

  it("stops at the ends of the list", () => {
    expect(
      step({ index: 9, total: 10, members, currentId: 13, delta: 1 }),
    ).toBeNull();
    expect(
      step({ index: 0, total: 10, members, currentId: 11, delta: -1 }),
    ).toBeNull();
    expect(
      step({ index: 0, total: 0, members: [], currentId: null, delta: 1 }),
    ).toBeNull();
  });

  it("treats a photo outside the stack as the entry itself", () => {
    expect(
      step({ index: 2, total: 5, members, currentId: 99, delta: 1 }),
    ).toEqual({
      kind: "entry",
      index: 3,
      entry: "first",
    });
  });
});

describe("entryMember", () => {
  it.each([
    [[1, 2, 3], "first", 1],
    [[1, 2, 3], "last", 3],
    [[1, 2, 3], "shown", null],
    [[1], "first", null],
    [[], "last", null],
  ] as const)("in %j entering at %s shows %s", (members, entry, expected) => {
    expect(entryMember([...members], entry)).toBe(expected);
  });
});
