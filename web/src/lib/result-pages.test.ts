import { expect, test } from "bun:test";
import {
  boxesBefore,
  boxParts,
  isFar,
  type Kept,
  kidsOf,
  leadOffset,
  type Page,
  pageAt,
  pageQuery,
  pageStarts,
  readSnapshot,
  resized,
  restorePlan,
  resultsQuery,
  SNAPSHOT,
  type Snapshot,
  withAbove,
  withFailed,
  withInto,
  withNext,
  withPage,
  withRest,
} from "./result-pages";

test("pageQuery leaves page 1 bare and keeps sem=0 only on later pages", () => {
  expect(pageQuery("", 1, true)).toBe("");
  expect(pageQuery("q=dress", 1, true)).toBe("q=dress");
  expect(pageQuery("q=dress", 3, false)).toBe("q=dress&page=3");
  expect(pageQuery("q=dress", 3, true)).toBe("q=dress&page=3&sem=0");
  expect(pageQuery("", 2, false)).toBe("page=2");
});

test("withPage rewrites page and sem, keeping the other pairs byte for byte and the item last", () => {
  expect(withPage("", 2, false)).toBe("?page=2");
  expect(withPage("?page=4&sem=0", 1, true)).toBe("");
  expect(withPage("?tags=a+b&page=2&sem=0", 5, true)).toBe(
    "?tags=a+b&page=5&sem=0",
  );
  expect(withPage("?q=x&page=2&item=7", 3, false)).toBe("?q=x&page=3&item=7");
  expect(withPage("?item=7&q=x", 1, false)).toBe("?q=x&item=7");
});

test("resultsQuery keeps only the pairs that choose the results", () => {
  expect(resultsQuery("?tags=a+b&page=2&sem=0&item=7")).toBe("tags=a+b");
  expect(resultsQuery("")).toBe("");
});

test("pageAt picks the last page starting above the line, the first when none does", () => {
  expect(pageAt([-3000, -200, 900], 450)).toBe(1);
  expect(pageAt([-3000, -200, 400], 450)).toBe(2);
  expect(pageAt([300], 450)).toBe(0);
  expect(pageAt([600], 450)).toBe(0);
});

test("isFar is true only beyond two screens of the viewport", () => {
  const h = 900;
  expect(isFar(-5000, -1801, h)).toBe(true);
  expect(isFar(-5000, -1799, h)).toBe(false);
  expect(isFar(2701, 9000, h)).toBe(true);
  expect(isFar(2699, 9000, h)).toBe(false);
});

const snap: Snapshot = { key: "q=x", page: 3, id: 42, top: 120 };

test("readSnapshot takes only this search's entry", () => {
  expect(readSnapshot({ [SNAPSHOT]: snap, __NA: true }, "q=x")).toEqual(snap);
  expect(readSnapshot({ [SNAPSHOT]: snap }, "q=y")).toBeNull();
  expect(readSnapshot(null, "q=x")).toBeNull();
  expect(readSnapshot({ __NA: true }, "q=x")).toBeNull();
  expect(readSnapshot({ [SNAPSHOT]: { ...snap, id: 0 } }, "q=x")).toBeNull();
});

test("restorePlan reuses recent pages laid out at this width, else reads the anchor's page", () => {
  const kept: Kept<string> = {
    key: "q=x",
    pages: ["p2", "p3"],
    width: 1200,
    at: 1_000,
  };
  expect(restorePlan(snap, kept, 1200, 2_000)).toEqual({
    pages: ["p2", "p3"],
  });
  expect(restorePlan(snap, kept, 800, 2_000)).toEqual({ page: 3 });
  expect(restorePlan(snap, kept, 1200, 1_000 + 6 * 60_000)).toEqual({
    page: 3,
  });
  expect(restorePlan(snap, { ...kept, key: "q=y" }, 1200, 2_000)).toEqual({
    page: 3,
  });
  expect(restorePlan(snap, undefined, 1200, 2_000)).toEqual({ page: 3 });
});

test("boxParts fills exactly a page's cells, so the next page starts in its own column", () => {
  // 6 columns: page 1 ends 4 into a row, page 2 closes it with 2, page 3 with 4
  expect(boxParts(1, 100, 6)).toEqual({ lead: 0, rows: 16, tail: 4 });
  expect(boxParts(2, 100, 6)).toEqual({ lead: 2, rows: 16, tail: 2 });
  expect(boxParts(3, 100, 6)).toEqual({ lead: 4, rows: 16, tail: 0 });
  // 5 columns: whole rows only
  expect(boxParts(1, 100, 5)).toEqual({ lead: 0, rows: 20, tail: 0 });
  expect(boxParts(4, 100, 5)).toEqual({ lead: 0, rows: 20, tail: 0 });
  // 7 columns: page 2 starts at index 100, 2 into a row
  expect(boxParts(2, 100, 7)).toEqual({ lead: 5, rows: 13, tail: 4 });
  expect(boxParts(2, 100, 2)).toEqual({ lead: 0, rows: 50, tail: 0 });
  // a page smaller than the rest of its row is all lead
  expect(boxParts(2, 3, 8)).toEqual({ lead: 3, rows: 0, tail: 0 });
  for (const [n, cols] of [
    [2, 6],
    [3, 7],
    [9, 8],
  ]) {
    const { lead, rows, tail } = boxParts(n, 100, cols);
    expect(lead + rows * cols + tail).toBe(100);
    expect((n * 100) % cols).toBe(tail);
  }
});

test("leadOffset is the first page's column of the whole set, none from page 1", () => {
  expect(leadOffset(1, 100, 6)).toBe(0);
  expect(leadOffset(3, 100, 6)).toBe(2);
  expect(leadOffset(2, 100, 7)).toBe(2);
  expect(leadOffset(9, 100, 5)).toBe(0);
});

type T = { id: number };
const tiles = (...ids: number[]): T[] => ids.map((id) => ({ id }));

test("kidsOf counts the tiles, a notice, a page of silhouettes near view, else its boxes", () => {
  expect(kidsOf({ n: 1, tiles: tiles(1, 2) }, 100, 6)).toBe(2);
  expect(kidsOf({ n: 1, tiles: [], near: true }, 100, 6)).toBe(100);
  expect(kidsOf({ n: 1, tiles: [], near: true, failed: true }, 100, 6)).toBe(1);
  expect(kidsOf({ n: 1, tiles: [] }, 100, 6)).toBe(2);
  expect(kidsOf({ n: 2, tiles: [] }, 100, 6)).toBe(3);
  expect(kidsOf({ n: 3, tiles: [] }, 100, 6)).toBe(2);
  expect(kidsOf({ n: 3, tiles: [] }, 100, 5)).toBe(1);
});

test("pageStarts runs each page's children on from the last, after the first page's offset, to their end", () => {
  const p: Page<T>[] = [
    { n: 1, tiles: [] },
    { n: 2, tiles: [], near: true },
    { n: 3, tiles: tiles(1, 2, 3) },
    { n: 4, tiles: [] },
  ];
  // page 1 at 4 columns of 10: 2 rows and a tail of 2
  expect(pageStarts(p, 10, 4)).toEqual([0, 2, 12, 15, 17]);
  expect(pageStarts(p, 10, 5)).toEqual([0, 1, 11, 14, 15]);
  // page 3 first: 20 % 6 = 2 cells before it, one child
  expect(pageStarts(p.slice(2), 10, 6)).toEqual([1, 4, 6]);
  expect(pageStarts(p.slice(2), 10, 5)).toEqual([0, 3, 4]);
});

test("boxesBefore puts at most ABOVE pages before n, from page 1", () => {
  expect(boxesBefore(1)).toEqual([]);
  expect(boxesBefore(3)).toEqual([
    { n: 1, tiles: [] },
    { n: 2, tiles: [] },
  ]);
  const deep = boxesBefore(60);
  expect(deep.length).toBe(40);
  expect([deep[0].n, deep[39].n]).toEqual([20, 59]);
});

test("withAbove puts the pages before the first one up once, however often it runs", () => {
  const p: Page<T>[] = [{ n: 3, tiles: tiles(1) }];
  const once = withAbove(p, 3);
  expect(once.map((q) => q.n)).toEqual([1, 2, 3]);
  expect(withAbove(once, 3)).toBe(once);
  expect(withAbove(p, 1)).toBe(p);
});

test("withRest adds the unseen tiles to the partial page only, drawn gradually", () => {
  const p: Page<T>[] = [
    { n: 2, tiles: tiles(1), partial: true },
    { n: 3, tiles: tiles(9), partial: true },
  ];
  const next = withRest(p, 2, tiles(1, 2, 3), new Set([1]));
  expect(next[0]).toEqual({ n: 2, tiles: tiles(1, 2, 3), gradual: true });
  expect(next[1]).toBe(p[1]);
  const whole: Page<T>[] = [{ n: 2, tiles: tiles(1) }];
  expect(withRest(whole, 2, tiles(2), new Set())[0]).toBe(whole[0]);
});

test("withNext appends only while the page read after is still the last", () => {
  const p: Page<T>[] = [{ n: 4, tiles: tiles(1) }];
  expect(withNext(p, 4, tiles(2))).toEqual([
    p[0],
    { n: 5, tiles: tiles(2), gradual: true },
  ]);
  expect(withNext(p, 3, tiles(2))).toBe(p);
});

test("withInto fills only an unread box, gradually when it is below the reader", () => {
  const p: Page<T>[] = [
    { n: 1, tiles: [], near: true },
    { n: 2, tiles: tiles(5) },
  ];
  expect(withInto(p, 1, tiles(7), "next")[0]).toEqual({
    n: 1,
    tiles: tiles(7),
    gradual: true,
  });
  expect(withInto(p, 1, tiles(7), "prev")[0]).toEqual({
    n: 1,
    tiles: tiles(7),
    gradual: false,
  });
  expect(withInto(p, 2, tiles(7), "next")[1]).toBe(p[1]);
});

test("withFailed sets and clears one page's failure", () => {
  const p: Page<T>[] = [
    { n: 1, tiles: [] },
    { n: 2, tiles: [] },
  ];
  const failed = withFailed(p, 2, true);
  expect(failed[1]).toEqual({ n: 2, tiles: [], failed: true });
  expect(failed[0]).toBe(p[0]);
  expect(withFailed(failed, 2, false)[1]).toEqual({
    n: 2,
    tiles: [],
    failed: false,
  });
});

test("resized gives boxes a tile's new height and leaves the other pages", () => {
  const p: Page<T>[] = [
    { n: 1, tiles: tiles(1, 2), heights: [300, 310] },
    { n: 2, tiles: tiles(3) },
    { n: 3, tiles: [] },
  ];
  const next = resized(p, 280);
  expect(next[0].heights).toEqual([280, 280]);
  expect(next[1]).toBe(p[1]);
  expect(next[2]).toBe(p[2]);
});
