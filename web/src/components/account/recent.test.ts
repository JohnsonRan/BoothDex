import { afterEach, expect, test } from "bun:test";
import { itemsOf } from "./recent";

const realFetch = globalThis.fetch;
afterEach(() => {
  globalThis.fetch = realFetch;
});

test("one read of a list is shared while under way, and a later one asks again", async () => {
  let reads = 0;
  globalThis.fetch = (async () => {
    reads++;
    return Response.json({ items: [] });
  }) as unknown as typeof fetch;
  await Promise.all([itemsOf([1, 2]), itemsOf([1, 2])]);
  expect(reads).toBe(1);
  await itemsOf([1, 2]);
  expect(reads).toBe(2);
});
