import { expect, test } from "bun:test";
import {
  anchorAt,
  type Frame,
  firstBelow,
  framePages,
  pageToRead,
  restoreStep,
} from "./result-frame";
import type { Page } from "./result-pages";

type T = { id: number };
const tile = (id: number): T => ({ id });

test("firstBelow finds the first child reaching below the edge, the last when none does", () => {
  const bottoms = [100, 200, 300, 400];
  const at = (i: number) => bottoms[i];
  expect(firstBelow(0, 4, 250, at)).toBe(2);
  expect(firstBelow(0, 4, 200, at)).toBe(2);
  expect(firstBelow(0, 4, 50, at)).toBe(0);
  expect(firstBelow(0, 4, 999, at)).toBe(3);
  expect(firstBelow(1, 3, 50, at)).toBe(1);
});

test("anchorAt takes the first tile within the viewport, else the child it started from", () => {
  const tops = [-50, 100, 200, 1000];
  const top = (i: number) => tops[i];
  expect(anchorAt(0, 4, 900, top, (i) => i >= 2)).toBe(2);
  expect(anchorAt(0, 4, 900, top, (i) => i === 3)).toBe(0);
  expect(anchorAt(1, 4, 900, top, () => true)).toBe(1);
});

const frame = (over: Partial<Frame> = {}): Frame => ({
  tops: [-5000, 0, 3000],
  bottoms: [-1900, 3000, 6000],
  height: 900,
  edge: 100,
  inView: 1,
  up: false,
  scrolling: false,
  ...over,
});
const never = (): number[] => {
  throw new Error("not measured");
};

test("framePages keeps every page that stays as it is", () => {
  const p: Page<T>[] = [
    { n: 1, tiles: [tile(1)], heights: [300] },
    { n: 2, tiles: [tile(2)] },
    { n: 3, tiles: [], near: false },
  ];
  const next = framePages(p, frame(), never);
  expect(next.every((q, j) => q === p[j])).toBe(true);
});

test("framePages gives a far page's tiles up for their heights, unless focus holds it", () => {
  const p: Page<T>[] = [
    { n: 1, tiles: [tile(1), tile(2)] },
    { n: 2, tiles: [tile(3)] },
  ];
  const f = frame({ tops: [-5000, 0], bottoms: [-1900, 3000] });
  expect(framePages(p, f, () => [300, 310])[0]).toEqual({
    n: 1,
    tiles: [tile(1), tile(2)],
    heights: [300, 310],
    gradual: true,
  });
  expect(framePages(p, f, () => undefined)[0]).toBe(p[0]);
});

test("framePages draws a page back near view, gradually only from below the screen", () => {
  const p: Page<T>[] = [
    { n: 1, tiles: [tile(1)], heights: [300] },
    { n: 2, tiles: [tile(2)], heights: [300] },
  ];
  const next = framePages(
    p,
    frame({ tops: [-500, 1000], bottoms: [1000, 2000], inView: 0 }),
    never,
  );
  expect(next[0]).toEqual({
    n: 1,
    tiles: [tile(1)],
    heights: undefined,
    gradual: false,
  });
  expect(next[1].gradual).toBe(true);
});

test("framePages turns a box near view to silhouettes: below the page in view, or above it moving up", () => {
  const p: Page<T>[] = [
    { n: 1, tiles: [], block: 900 },
    { n: 2, tiles: [tile(2)] },
    { n: 3, tiles: [], block: 900 },
  ];
  const f = frame({ tops: [-800, 50, 1000], bottoms: [50, 1000, 1900] });
  const down = framePages(p, f, never);
  expect(down[0]).toBe(p[0]);
  expect(down[2].near).toBe(true);
  expect(framePages(p, { ...f, up: true }, never)[0].near).toBe(true);
  // one starting above the band waits for scrolling to stop
  expect(framePages(p, { ...f, up: true, scrolling: true }, never)[0]).toBe(
    p[0],
  );
  // a box gone far again stops being silhouettes
  const far = framePages(
    [{ n: 1, tiles: [], near: true }],
    frame({ tops: [5000], bottoms: [9000], inView: 0 }),
    never,
  );
  expect(far[0].near).toBe(false);
});

test("pageToRead picks the page in view, else the nearest unread near page", () => {
  const p: Page<T>[] = [
    { n: 1, tiles: [] },
    { n: 2, tiles: [] },
    { n: 3, tiles: [tile(3)] },
    { n: 4, tiles: [], failed: true },
    { n: 5, tiles: [] },
  ];
  const near = (...on: number[]) =>
    p.map((q, j) => ({ ...q, near: on.includes(j) }));
  expect(pageToRead(p, near(0, 1, 4), 1)).toBe(1);
  expect(pageToRead(p, near(0, 4), 2)).toBe(0);
  expect(pageToRead(p, near(3, 4), 3)).toBe(4);
  expect(pageToRead(p, near(2, 3), 2)).toBeUndefined();
});

test("restoreStep ends at the place, reads on while short of it, and stops when nothing is left", () => {
  const whole: Page<T> = { n: 3, tiles: [tile(1)] };
  expect(restoreStep(0.5, whole, 9)).toBe("done");
  expect(restoreStep(40, whole, 9)).toBe("read");
  expect(restoreStep(40, { ...whole, n: 9 }, 9)).toBe("done");
  expect(restoreStep(undefined, whole, 9)).toBe("done");
  expect(restoreStep(undefined, { ...whole, partial: true }, 9)).toBe("read");
});
