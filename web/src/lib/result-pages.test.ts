import { expect, test } from "bun:test";
import {
  isFar,
  type Kept,
  pageAt,
  pageQuery,
  readSnapshot,
  restorePlan,
  resultsQuery,
  SNAPSHOT,
  type Snapshot,
  withPage,
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
