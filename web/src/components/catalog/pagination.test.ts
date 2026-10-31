import { expect, test } from "bun:test";
import { pageList } from "./pagination";

test("page list keeps the ends and the current neighbourhood", () => {
  expect(pageList(1, 1)).toEqual([1]);
  expect(pageList(1, 5)).toEqual([1, 2, 3, 4, 5]);
  expect(pageList(6, 72)).toEqual([1, null, 4, 5, 6, 7, 8, null, 72]);
  expect(pageList(1, 72)).toEqual([1, 2, 3, null, 72]);
  expect(pageList(72, 72)).toEqual([1, null, 70, 71, 72]);
  // a gap of one page shows the page; longer gaps get one marker
  expect(pageList(4, 10)).toEqual([1, 2, 3, 4, 5, 6, null, 10]);
  expect(pageList(5, 9)).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9]);
});
