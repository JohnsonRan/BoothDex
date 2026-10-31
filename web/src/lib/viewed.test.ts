import { expect, test } from "bun:test";
import { addViewed, parseViewed } from "./viewed";

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

test("parseViewed accepts valid [id, at] pairs and drops invalid or old number-only data", () => {
  const now = Date.now();
  const raw = [
    [101, now],
    102, // old number-only format
    "invalid",
    [103, "not-a-timestamp"],
    [101, now - 1000], // duplicate id
    [-5, now],
    [104, now - 500],
  ];
  expect(parseViewed(raw)).toEqual([
    [101, now],
    [104, now - 500],
  ]);
  expect(parseViewed(null)).toEqual([]);
  expect(parseViewed({})).toEqual([]);
  const pairs = [1, 2, 3, 4].map((id) => [id, now]);
  expect(parseViewed(pairs, 2)).toEqual([
    [1, now],
    [2, now],
  ]);
});
