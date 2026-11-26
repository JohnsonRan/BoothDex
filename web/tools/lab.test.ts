import { expect, test } from "bun:test";
import { createRequire } from "node:module";

type Box = { x: number; y: number; w: number; h: number };
type Geometry = Record<string, Box | number | boolean | null> & {
  perRow: number;
  notesOffset?: number;
};

const require = createRequire(import.meta.url);
const { compare } = require("./lab.cjs") as {
  compare: (
    a: Geometry,
    b: Geometry,
    tolerance?: number,
  ) => { worst: number; bad: string[] };
};

const base = (offset: number, notesOffset = 0): Geometry => ({
  topBar: { x: 0, y: 0, w: 1440, h: 48 },
  results: { x: 280, y: 372, w: 1136, h: 1200 + offset },
  notesOffset,
  grid: { x: 280, y: 372 + offset, w: 1136, h: 1200 },
  tile0: { x: 280, y: 372 + offset, w: 176, h: 284 },
  "tile0.cover": { x: 280, y: 372 + offset, w: 176, h: 176 },
  "tile0.title": { x: 280, y: 560 + offset, w: 176, h: 40 },
  perRow: 6,
});

test("compare accounts for ready notesOffset across grid and tiles while checking unshifted results", () => {
  const loading = base(0, 0);
  const ready = base(96, 96);
  expect(compare(ready, loading)).toEqual({ worst: 0, bad: [] });
});

test("compare fails when results container, relative tile geometry, or loading notesOffset shifts", () => {
  const loading = base(0, 0);
  const shiftedResults = base(96, 96);
  shiftedResults.results = { x: 280, y: 376, w: 1136, h: 1296 };
  expect(compare(shiftedResults, loading).bad).toContain(
    "results.y 376.0 -> 372.0",
  );

  const shiftedTitle = base(96, 96);
  shiftedTitle["tile0.title"] = { x: 280, y: 661, w: 176, h: 40 };
  expect(compare(shiftedTitle, loading).bad).toContain(
    "tile0.title.y 565.0 (661.0 - notes 96.0) -> 560.0",
  );

  const unexpectedLoadingNotes = base(0, 20);
  expect(compare(base(96, 96), unexpectedLoadingNotes).bad).toContain(
    "notesOffset (loading) 20.0",
  );
});
