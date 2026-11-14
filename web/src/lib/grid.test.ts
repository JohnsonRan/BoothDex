import { expect, test } from "bun:test";
import { columnCount } from "./grid";

test("columnCount counts the columns and leaves out the empty implicit tracks a wide span adds", () => {
  expect(columnCount("176px 176px 176px 176px 176px 176px")).toBe(6);
  // a 520 px grid of 160 px columns holding a `span 4` box, implicit tracks at 0px
  expect(columnCount("160px 160px 160px 0px")).toBe(3);
  expect(columnCount("186.656px 186.672px 186.672px 0px 0px")).toBe(3);
  expect(columnCount("177.594px")).toBe(1);
});
