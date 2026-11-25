import { expect, test } from "bun:test";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { browserMessages, MESSAGES, SERVER_ONLY } from "./dictionaries";
import { type MessageKey, UI_LANGS } from "./messages";

const src = join(import.meta.dir, "..");
const read = (file: string) => readFileSync(join(src, file), "utf8");
// the source files that can look up a message: no tests, no dictionaries
const sources = readdirSync(src, { recursive: true, encoding: "utf8" })
  .map((file) => file.replaceAll("\\", "/"))
  .filter(
    (file) =>
      /\.tsx?$/.test(file) &&
      !/\.test\.tsx?$/.test(file) &&
      !/\/messages-\w+\.ts$/.test(file),
  )
  .sort();

test("the browser's dictionary has every key but the server-only ones", () => {
  for (const ui of UI_LANGS) {
    const browser = browserMessages(ui);
    for (const [key, text] of Object.entries(MESSAGES[ui])) {
      const kept = !SERVER_ONLY.includes(key as MessageKey);
      expect(Object.hasOwn(browser, key), `${ui} ${key}`).toBe(kept);
      if (kept) expect(browser[key as MessageKey]).toBe(text);
    }
  }
});

const SERVER_RESULTS = [
  "components/catalog/results/result-notes.tsx",
  "components/catalog/results/results.tsx",
];

test("only results modules rendered on the server look up a server-only key", () => {
  expect(
    sources.filter((file) =>
      SERVER_ONLY.some((key) => read(file).includes(`"${key}"`)),
    ),
  ).toEqual([...SERVER_RESULTS, "lib/dictionaries.ts"]);
  for (const file of SERVER_RESULTS) {
    expect(read(file)).not.toContain('"use client"');
  }
  // pages hand the browser its dictionary through browserMessages only
  expect(sources.filter((file) => /\bMESSAGES\b/.test(read(file)))).toEqual([
    "lib/dictionaries.ts",
  ]);
});

test("every key that only server results modules look up is server-only", () => {
  const texts = sources
    .filter((file) => file !== "lib/dictionaries.ts")
    .map((file) => [file, read(file)] as const);
  const onlyResults = Object.keys(MESSAGES.en).filter((key) => {
    const named = texts.filter(([, text]) => text.includes(`"${key}"`));
    return (
      named.length > 0 && named.every(([file]) => SERVER_RESULTS.includes(file))
    );
  });
  expect(onlyResults.sort()).toEqual([...SERVER_ONLY].sort());
});
