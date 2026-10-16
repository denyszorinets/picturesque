import "@testing-library/jest-dom/vitest";

// jsdom has no ResizeObserver (used by Svelte's size bindings); nothing is
// laid out anyway.
globalThis.ResizeObserver ??= class {
  observe() {}
  unobserve() {}
  disconnect() {}
} as unknown as typeof ResizeObserver;
