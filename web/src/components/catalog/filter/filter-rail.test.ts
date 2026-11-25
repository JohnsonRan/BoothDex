import { expect, test } from "bun:test";
import { visibleRailHeader } from "./filter-rail";

test("visibleRailHeader shrinks from the full header height to zero as the page scrolls past it", () => {
  expect(visibleRailHeader(56, 0)).toBe(56);
  expect(visibleRailHeader(56, 20)).toBe(36);
  expect(visibleRailHeader(56, 56)).toBe(0);
  expect(visibleRailHeader(56, 200)).toBe(0);
});

test("visibleRailHeader clamps negative overscroll and empty headers", () => {
  expect(visibleRailHeader(64, -18)).toBe(64);
  expect(visibleRailHeader(0, 0)).toBe(0);
});
