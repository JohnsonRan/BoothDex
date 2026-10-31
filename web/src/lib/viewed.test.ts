import { expect, test } from "bun:test";
import { addViewed } from "./viewed";

test("addViewed keeps newest first, moves a repeat to the top and caps the list", () => {
  let v: number[] = [];
  v = addViewed(v, 1);
  v = addViewed(v, 2);
  v = addViewed(v, 1);
  expect(v).toEqual([1, 2]);
  const many = [1, 2, 3, 4].reduce(
    (l, id) => addViewed(l, id, 3),
    [] as number[],
  );
  expect(many).toEqual([4, 3, 2]);
});
