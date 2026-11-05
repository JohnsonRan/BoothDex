import { expect, test } from "bun:test";
import {
  FONTS_CSS,
  FONTS_EARLY,
  FONTS_USED,
  fontSlices,
  isFontSheet,
  isFontSlice,
} from "./fonts";

const ORIGIN = "https://boothdex.example";
Object.assign(globalThis, { location: { origin: ORIGIN } });

const SHEET = `${ORIGIN}/_next/static/chunks/0eb6ulhvsu1on.css`;
const SLICE = `${ORIGIN}/_next/static/media/MiSans-VF.10.1kh03p1nl-7va.woff2`;

test("only same-origin MiSans addresses are accepted", () => {
  expect(isFontSheet(SHEET)).toBe(true);
  expect(isFontSlice(SLICE)).toBe(true);
  for (const bad of [
    "https://evil.example/_next/static/chunks/a.css",
    `${ORIGIN}/_next/static/chunks/a.js`,
    `${ORIGIN}/api/x.css`,
    "javascript:alert(1)",
    "/_next/static/chunks/a.css",
    42,
    null,
  ])
    expect(isFontSheet(bad)).toBe(false);
  expect(isFontSlice(`${ORIGIN}/_next/static/media/Figtree.1.woff2`)).toBe(
    false,
  );
  expect(isFontSlice(SLICE.replace(ORIGIN, "https://evil.example"))).toBe(
    false,
  );
});

test("remembered slices must be an array of valid addresses", () => {
  expect(
    fontSlices(JSON.stringify([SLICE, 1, "https://evil.example/x"])),
  ).toEqual([SLICE]);
  expect(fontSlices('{"0":"x"}')).toEqual([]);
  expect(fontSlices("not json")).toEqual([]);
  expect(fontSlices(null)).toEqual([]);
});

/** Runs the head script against a fake document; returns the links it added. */
function runEarly(store: Record<string, string>) {
  const added: Record<string, string>[] = [];
  const el = () => {
    const link: Record<string, string> = {};
    return Object.assign(link, {
      setAttribute: (k: string, v: string) => {
        link[k] = v;
      },
    });
  };
  const document = {
    head: { appendChild: (l: Record<string, string>) => added.push(l) },
    createElement: el,
  };
  const localStorage = { getItem: (k: string) => store[k] ?? null };
  new Function("document", "localStorage", "location", FONTS_EARLY)(
    document,
    localStorage,
    { origin: ORIGIN },
  );
  return added.map((l) => `${l.rel} ${l.href}`);
}

test("the head script loads only valid remembered addresses", () => {
  expect(
    runEarly({
      [FONTS_CSS]: SHEET,
      [FONTS_USED]: JSON.stringify([SLICE, "https://evil.example/f.woff2", 3]),
    }),
  ).toEqual([`preload ${SLICE}`, `stylesheet ${SHEET}`]);
  expect(runEarly({ [FONTS_CSS]: "https://evil.example/a.css" })).toEqual([]);
  expect(
    runEarly({ [FONTS_CSS]: SHEET, [FONTS_USED]: '"just a string"' }),
  ).toEqual([`stylesheet ${SHEET}`]);
});
