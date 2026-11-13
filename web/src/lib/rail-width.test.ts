import { expect, test } from "bun:test";
import { parseRailWidth, RAIL_REM, railCookie, railToSave } from "./rail-width";

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

test("going back to the loaded width after a saved drag writes it again", () => {
  let saved: number = RAIL_REM.default;
  const settle = (size: number) => {
    const next = railToSave(saved, size);
    if (next !== null) saved = next;
  };
  settle(18.75);
  expect(saved).toBe(18.75);
  settle(RAIL_REM.default);
  expect(saved).toBe(RAIL_REM.default);
});

test("a width that differs only by rounding writes nothing", () => {
  expect(railToSave(14.5, (14.5 * 18.4) / 18.4)).toBeNull();
});
