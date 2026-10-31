import { expect, test } from "bun:test";
import { addQuery } from "./history";

test("addQuery keeps newest first, dedupes case-insensitively and caps the list", () => {
  let h: string[] = [];
  h = addQuery(h, "  short hair ");
  h = addQuery(h, "dress");
  h = addQuery(h, "Short Hair");
  expect(h).toEqual(["Short Hair", "dress"]);
  expect(addQuery(h, "   ")).toBe(h);
  const many = ["a", "b", "c", "d"].reduce(
    (l, q) => addQuery(l, q, 3),
    [] as string[],
  );
  expect(many).toEqual(["d", "c", "b"]);
});
