import { expect, test } from "bun:test";
import { parseRailWidth, RAIL_REM, railCookie } from "./rail-width";

test("a stored rail width is held to the range and to quarter rems", () => {
  expect(parseRailWidth("18.6")).toBe(18.5);
  expect(parseRailWidth("2")).toBe(RAIL_REM.min);
  expect(parseRailWidth("99")).toBe(RAIL_REM.max);
});

test("a missing or malformed rail width is the default", () => {
  for (const raw of [undefined, null, "", "wide", "NaN", "Infinity"])
    expect(parseRailWidth(raw)).toBe(RAIL_REM.default);
});

test("the cookie stores the width as parsed back", () => {
  expect(railCookie(20.1)).toStartWith("rail=20; ");
});
